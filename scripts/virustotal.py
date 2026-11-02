#!/usr/bin/env python3
"""
Minimal VirusTotal API v3 client used by the Aurelia release pipeline.

API contract verified against the official documentation (November 2026):

  POST /files                     multipart upload of one file (<= 32 MB); returns
                                  {"data": {"type": "analysis", "id": "<analysis id>"}}
  POST /files/{sha256}/analyse     re-analyse a sample VT already holds; same shape
  GET  /analyses/{id}             status is queued | in-progress | completed; stats keys
                                  malicious, suspicious, harmless, undetected, timeout,
                                  confirmed-timeout, failure, type-unsupported
  GET  /files/{sha256}            file report; `last_analysis_stats` is all-zero when the
                                  record exists but was never analysed

Errors are {"error": {"code": "...", "message": "..."}} with HTTP 404 NotFoundError,
409 AlreadyExistsError, 429 QuotaExceededError / TooManyRequestsError,
503 TransientError, 504 DeadlineExceededError.

Public API quota (docs: "Public vs Premium API"): 4 requests per minute, 500 per day,
daily reset at 00:00 UTC. Every request therefore goes through one pacer so the pipeline
cannot exceed the per-minute budget.

The API key is supplied by the caller from the environment (VIRUSTOTAL_API_KEY); this
module never reads files or hard-codes credentials.
"""

import hashlib
import json
import mimetypes
import time
import urllib.error
import urllib.request
from pathlib import Path

API = "https://www.virustotal.com/api/v3"
PUBLIC_MIN_INTERVAL = 15.0        # 4 requests/minute
MAX_UPLOAD_BYTES = 32 * 1024 * 1024
DEFAULT_TIMEOUT = 900.0           # 15 minutes of polling
USER_AGENT = "aurelia-local-pipeline/1.0"


class VirusTotalError(RuntimeError):
    def __init__(self, message, status=None, code=None):
        super().__init__(message)
        self.status = status
        self.code = code


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while chunk := handle.read(1 << 20):
            digest.update(chunk)
    return digest.hexdigest()


def stats_total(stats) -> int:
    """Verdicts returned by engines; 0 means 'submitted but never analysed'."""
    try:
        return sum(int(v) for v in (stats or {}).values())
    except (TypeError, ValueError):
        return 0


class VirusTotalClient:
    def __init__(self, api_key, min_interval=PUBLIC_MIN_INTERVAL, timeout=DEFAULT_TIMEOUT,
                 log=print):
        if not api_key or not api_key.strip():
            raise VirusTotalError("empty VirusTotal API key")
        self.api_key = api_key.strip()
        self.min_interval = float(min_interval)
        self.timeout = float(timeout)
        self.log = log
        self._last_call = 0.0
        self.requests_made = 0

    # ── transport ───────────────────────────────────────────────────────
    def _pace(self):
        wait = self.min_interval - (time.monotonic() - self._last_call)
        if wait > 0:
            time.sleep(wait)
        self._last_call = time.monotonic()

    def _call(self, method, path, body=None, content_type="application/json"):
        url = path if path.startswith("http") else f"{API}{path}"
        for attempt in range(1, 4):
            self._pace()
            req = urllib.request.Request(url, data=body, method=method)
            req.add_header("x-apikey", self.api_key)
            req.add_header("Accept", "application/json")
            req.add_header("User-Agent", USER_AGENT)
            if body is not None:
                req.add_header("Content-Type", content_type)
            try:
                with urllib.request.urlopen(req, timeout=120) as resp:
                    self.requests_made += 1
                    payload = resp.read()
                    return resp.status, (json.loads(payload) if payload else {})
            except urllib.error.HTTPError as exc:
                self.requests_made += 1
                raw = exc.read()
                try:
                    parsed = json.loads(raw) if raw else {}
                except ValueError:
                    parsed = {}
                error = parsed.get("error", {}) if isinstance(parsed, dict) else {}
                code = error.get("code")
                message = error.get("message") or exc.reason

                if exc.code in (429, 503, 504) and attempt < 3:
                    # Documented quota / transient errors: back off, then retry.
                    backoff = 60 if exc.code == 429 else 20
                    self.log(f"  VirusTotal {exc.code} {code or ''}: {message}; "
                             f"retrying in {backoff}s ({attempt}/3)")
                    time.sleep(backoff)
                    continue
                if exc.code in (404, 409):
                    return exc.code, parsed
                raise VirusTotalError(
                    f"VirusTotal API error {exc.code} {code or ''}: {message}".strip(),
                    status=exc.code, code=code,
                ) from None
            except urllib.error.URLError as exc:
                if attempt < 3:
                    self.log(f"  VirusTotal network error ({exc.reason}); retrying in 20s")

    # ── endpoints ───────────────────────────────────────────────────────
    def file_report(self, sha256):
        """File report, or None when VirusTotal has never seen this hash (404)."""
        status, body = self._call("GET", f"/files/{sha256}")
        if status == 404:
            return None
        return body.get("data", {}).get("attributes", {}) if isinstance(body, dict) else None

    def upload(self, path: Path):
        size = path.stat().st_size
        if size > MAX_UPLOAD_BYTES:
            raise VirusTotalError(
                f"{path.name} is {size} bytes; /files accepts up to 32 MB "
                f"(use /files/upload_url for larger samples)"
            )
        boundary = "----aurelia" + hashlib.sha256(
            f"{path.name}{time.time()}".encode()).hexdigest()[:24]
        ctype = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
        head = (
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="file"; filename="{path.name}"\r\n'
            f"Content-Type: {ctype}\r\n\r\n"
        ).encode()
        tail = f"\r\n--{boundary}--\r\n".encode()
        status, body = self._call(
            "POST", "/files", body=head + path.read_bytes() + tail,
            content_type=f"multipart/form-data; boundary={boundary}",
        )
        if status == 409:
            return None  # already exists: caller falls back to a re-analysis
        return body.get("data", {}).get("id") if isinstance(body, dict) else None

    def reanalyse(self, sha256):
        status, body = self._call(
            "POST", f"/files/{sha256}/analyse", body=b"", content_type="application/json",
        )
        if status != 200:
            return None
        return body.get("data", {}).get("id") if isinstance(body, dict) else None

    def start_analysis(self, path: Path, sha256):
        """Return (analysis_id, how). Uploads new samples, re-analyses known ones."""
        report = self.file_report(sha256)
        if report is not None:
            if stats_total(report.get("last_analysis_stats", {})) > 0:
                return None, "existing"
            self.log("  sample known to VirusTotal but never analysed; requesting re-analysis")
            return self.reanalyse(sha256), "reanalyse"
        analysis_id = self.upload(path)
        if analysis_id is None:  # 409 race: it appeared between the two calls
            return self.reanalyse(sha256), "reanalyse"
        return analysis_id, "upload"

    def wait_for_analysis(self, analysis_id):
        """Poll until completed *and* engines actually reported, respecting the quota."""
        deadline = time.monotonic() + self.timeout
        last_status = None
        while time.monotonic() < deadline:
            status, body = self._call("GET", f"/analyses/{analysis_id}")
            if status == 200:
                attributes = body.get("data", {}).get("attributes", {})
                state = attributes.get("status")
                stats = attributes.get("stats", {})
                if state != last_status:
                    self.log(f"  analysis status: {state}")
                    last_status = state
                if state == "completed" and stats_total(stats) > 0:
                    return attributes
            elif status == 404:
                # Freshly queued analyses can 404 for a few seconds.
                self.log("  analysis not visible yet (404); retrying")
            if deadline - time.monotonic() <= 0:
                break
        raise VirusTotalError(
            f"analysis {analysis_id} did not complete within {int(self.timeout)}s"
        )

    def scan(self, path: Path):
        """Full scan of one file; returns a plain dict for the release record."""
        sha256 = sha256_of(path)
        self.log(f"  {path.name}: sha256 {sha256[:16]}...")
        analysis_id, how = self.start_analysis(path, sha256)
        if how == "existing":
            report = self.file_report(sha256) or {}
            stats = {k: int(v) for k, v in (report.get("last_analysis_stats") or {}).items()}
            self.log(f"  reused existing report: {stats.get('malicious', 0)} malicious / "
                     f"{stats.get('suspicious', 0)} suspicious "
                     f"({stats_total(stats)} engines)")
            return {
                "file": path.name, "sha256": sha256, "via": "existing",
                "analysis_date": report.get("last_analysis_date"),
                "stats": stats, "engines": stats_total(stats),
                "permalink": f"https://www.virustotal.com/gui/file/{sha256}",
            }

        self.log(f"  submitted via {how}; analysis id {analysis_id}")
        attributes = self.wait_for_analysis(analysis_id)
        stats = {k: int(v) for k, v in (attributes.get("stats") or {}).items()}
        self.log(f"  scan complete: {stats.get('malicious', 0)} malicious / "
                 f"{stats.get('suspicious', 0)} suspicious "
                 f"({stats_total(stats)} engines reporting)")
        return {
            "file": path.name, "sha256": sha256, "via": how,
            "analysis_id": analysis_id, "analysis_date": attributes.get("date"),
            "stats": stats, "engines": stats_total(stats),
            "permalink": f"https://www.virustotal.com/gui/file/{sha256}",
        }
