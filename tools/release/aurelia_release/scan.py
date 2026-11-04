"""VirusTotal scanning with an explicit, configurable release policy."""

from __future__ import annotations

import hashlib
import json
import time
import urllib.error
import urllib.request
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .config import ScanConfig
from .errors import ReleaseError
from .github import HttpResponse

VT_API = "https://www.virustotal.com/api/v3"


class ScanError(ReleaseError):
    """The scan could not produce a trustworthy verdict."""


@dataclass(frozen=True)
class ScanDecision:
    allowed: bool
    mode: str
    reason: str
    detections: int
    engines: tuple[str, ...]


def evaluate(stats: dict[str, bool], cfg: ScanConfig, sha256: str) -> ScanDecision:
    """Decide whether a report blocks the release.

    Undetected engines are represented as False and are kept in the mapping so
    the full report can be recorded, but they never count toward a threshold.
    """
    detected = tuple(sorted(name for name, hit in stats.items() if hit))
    total = len(detected)
    mode = cfg.mode

    if sha256.lower() in {item.lower() for item in cfg.allowlist}:
        return ScanDecision(True, mode, "allowlisted hash", total, detected)
    if mode == "off":
        return ScanDecision(True, mode, "scanning disabled", total, detected)
    if mode == "advisory":
        return ScanDecision(True, mode, f"advisory only: {total} detection(s)", total, detected)

    blocking_names = {name.lower() for name in cfg.blocking_engines}
    blocking = {
        name for name, hit in stats.items() if hit and name.lower() in blocking_names
    }
    if blocking:
        return ScanDecision(
            False,
            mode,
            f"blocking engine(s) detected: {', '.join(sorted(blocking))}",
            total,
            detected,
        )
    if mode == "strict":
        allowed = total == 0
        return ScanDecision(
            allowed,
            mode,
            "no detections" if allowed else f"strict mode: {total} detection(s)",
            total,
            detected,
        )
    allowed = total < cfg.threshold
    return ScanDecision(
        allowed,
        mode,
        f"{total} detection(s), threshold {cfg.threshold}"
        if not allowed
        else f"{total} detection(s) below threshold {cfg.threshold}",
        total,
        detected,
    )


def _multipart(path: Path) -> tuple[bytes, str]:
    boundary = f"----aurelia{uuid.uuid4().hex}"
    filename = path.name
    head = (
        f"--{boundary}\r\n"
        f'Content-Disposition: form-data; name="file"; filename="{filename}"\r\n'
        "Content-Type: application/octet-stream\r\n\r\n"
    ).encode("utf-8")
    tail = f"\r\n--{boundary}--\r\n".encode("utf-8")
    return head + path.read_bytes() + tail, f"multipart/form-data; boundary={boundary}"


class VirusTotalClient:
    def __init__(
        self,
        api_key: str,
        *,
        base: str = VT_API,
        transport: Callable[[str, str, dict[str, str], bytes | None], HttpResponse] | None = None,
        sleep: Callable[[float], None] = time.sleep,
        poll_attempts: int = 30,
        poll_interval: float = 15.0,
    ) -> None:
        self.api_key = api_key
        self.base = base.rstrip("/")
        self.transport = transport or self._urllib
        self.sleep = sleep
        self.poll_attempts = poll_attempts
        self.poll_interval = poll_interval

    @staticmethod
    def _urllib(
        method: str, url: str, headers: dict[str, str], body: bytes | None
    ) -> HttpResponse:
        request = urllib.request.Request(url, data=body, headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                return HttpResponse(response.status, dict(response.headers), response.read())
        except urllib.error.HTTPError as exc:
            return HttpResponse(exc.code, dict(exc.headers or {}), exc.read())

    def _call(
        self,
        method: str,
        path: str,
        *,
        body: bytes | None = None,
        content_type: str | None = None,
    ) -> HttpResponse:
        headers = {
            "x-apikey": self.api_key,
            "Accept": "application/json",
            "User-Agent": "aurelia-release/0.1",
        }
        if content_type:
            headers["Content-Type"] = content_type
        response = self.transport(method, f"{self.base}{path}", headers, body)
        if response.status == 404:
            return response
        if response.status >= 400:
            detail = response.body.decode("utf-8", "replace")[:300]
            raise ScanError(f"VirusTotal {method} {path}: HTTP {response.status}: {detail}")
        return response

    def get_report(self, sha256: str) -> dict[str, bool] | None:
        response = self._call("GET", f"/files/{sha256}")
        if response.status == 404:
            return None
        data = json.loads(response.body)
        results = data.get("data", {}).get("attributes", {}).get("last_analysis_results", {})
        return {
            name: info.get("category") in {"malicious", "suspicious"}
            for name, info in results.items()
        }

    def scan_file(self, path: Path) -> tuple[str, dict[str, bool]]:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        existing = self.get_report(digest)
        if existing is not None:
            return digest, existing

        body, content_type = _multipart(path)
        response = self._call("POST", "/files", body=body, content_type=content_type)
        analysis_id = json.loads(response.body).get("data", {}).get("id")
        if not analysis_id:
            raise ScanError(f"VirusTotal did not return an analysis id for {path.name}")

        for attempt in range(self.poll_attempts):
            state = json.loads(self._call("GET", f"/analyses/{analysis_id}").body)
            status = state.get("data", {}).get("attributes", {}).get("status")
            if status == "completed":
                report = self.get_report(digest)
                if report is not None:
                    return digest, report
                break
            if attempt + 1 < self.poll_attempts:
                self.sleep(self.poll_interval)
        raise ScanError(f"VirusTotal analysis for {path.name} did not complete")
