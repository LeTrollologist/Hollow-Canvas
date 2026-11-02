#!/usr/bin/env python3
"""
Attach the locally built Aurelia artifacts to the GitHub release page for a tag.

This script builds nothing. The bytes it uploads were already produced, verified
and recorded by scripts/pipeline.py on this machine; GitHub only stores them so
humans have somewhere to download from. No Actions workflow, no CI runner and no
forge CLI is involved -- it speaks the REST API directly with the same credential
`git push` already uses.

Usage:
  python scripts/release_page.py v2.1.0
  python scripts/release_page.py --all-missing
  python scripts/release_page.py v2.1.0 --dry-run
"""

import argparse
import hashlib
import json
import mimetypes
import os
import re
import subprocess
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
API = "https://api.github.com"
PRODUCT = "Aurelia"


def log(msg):
    print(f"[RELEASE] {msg}")


def get_token():
    for var in ("AURELIA_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"):
        val = os.environ.get(var)
        if val and val.strip():
            return val.strip()
    # Fall back to the credential git already uses for pushing.
    try:
        res = subprocess.run(
            ["git", "credential", "fill"], cwd=ROOT, text=True, capture_output=True,
            input="protocol=https\nhost=github.com\n\n", check=True,
        )
    except Exception:
        return None
    for line in res.stdout.splitlines():
        if line.startswith("password="):
            return line.split("=", 1)[1].strip()
    return None


def repo_slug():
    out = subprocess.run(["git", "remote", "get-url", "origin"], cwd=ROOT,
                         text=True, capture_output=True, check=True).stdout.strip()
    m = re.search(r"github\.com[:/]+(.+?)(?:\.git)?/?$", out)
    if not m:
        raise SystemExit(f"origin is not a GitHub remote: {out}")
    return m.group(1)


def call(tok, method, url, payload=None, raw=None, content_type="application/json"):
    data = raw if raw is not None else (json.dumps(payload).encode() if payload is not None else None)
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"token {tok}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("User-Agent", "aurelia-local-pipeline")
    if data is not None:
        req.add_header("Content-Type", content_type)
    try:
        with urllib.request.urlopen(req, timeout=300) as resp:
            body = resp.read()
            return resp.status, (json.loads(body) if body else {})
    except urllib.error.HTTPError as exc:
        body = exc.read()
        try:
            parsed = json.loads(body) if body else {}
        except ValueError:
            parsed = {"message": body.decode(errors="replace")}
        return exc.code, parsed


def notes_body(tag):
    path = ROOT / "release-notes" / f"{tag}.md"
    if not path.exists():
        raise SystemExit(f"missing {path}; cannot publish a release page without notes")
    text = path.read_text(encoding="utf-8")
    title = None
    for line in text.splitlines():
        if line.strip().startswith("#"):
            title = line.strip().lstrip("#").strip()
            break
    return text, (title or f"{PRODUCT} {tag}")


def assets_for(tag):
    dist = ROOT / "dist" / tag
    if not dist.exists():
        return []
    return [p for p in sorted(dist.glob("*"))
            if p.is_file() and p.name not in ("SHA256SUMS.txt", "PUBLISH.txt")]


def _recorded_hashes(tag):
    """Hashes the pipeline recorded into SHA256SUMS.txt, keyed by asset name."""
    path = ROOT / "dist" / tag / "SHA256SUMS.txt"
    if not path.exists():
        return {}
    recorded = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split()
        if len(parts) == 2:
            recorded[parts[1]] = parts[0].upper()
    return recorded


def dist_is_consistent(tag, files):
    """Refuse to upload bytes that do not match their own recorded checksums."""
    recorded = _recorded_hashes(tag)
    if not recorded:
        return False, "SHA256SUMS.txt is missing"
    for path in files:
        want = recorded.get(path.name)
        if want is None:
            return False, f"{path.name} is not listed in SHA256SUMS.txt"
        got = hashlib.sha256(path.read_bytes()).hexdigest().upper()
        if got != want:
            return False, f"{path.name} hashes {got[:16]}... but SHA256SUMS.txt says {want[:16]}..."
    return True, ""


def ensure_release(tok, slug, tag, dry_run):
    status, body = call(tok, "GET", f"{API}/repos/{slug}/releases/tags/{tag}")
    if status == 200:
        log(f"{tag}: release page exists")
        return body
    if status != 404:
        raise SystemExit(f"cannot look up {tag}: {status} {body.get('message')}")
    text, title = notes_body(tag)
    if dry_run:
        log(f"{tag}: would create the release page ({len(text)} bytes of notes)")
        return {"id": 0,
                "upload_url": f"{API}/repos/{slug}/releases/0/assets{{?name,label}}"}
    status, created = call(tok, "POST", f"{API}/repos/{slug}/releases", payload={
        "tag_name": tag, "name": title, "body": text, "draft": False, "prerelease": False,
    })
    if status not in (200, 201):
        raise SystemExit(f"cannot create release page for {tag}: {status} {created.get('message')}")
    log(f"{tag}: created the release page")
    return created


def upload_assets(tok, slug, tag, release, dry_run):
    files = assets_for(tag)
    if not files:
        log(f"{tag}: no local artifacts in dist/{tag}; release page left with notes only")
        return 0
    ok, why = dist_is_consistent(tag, files)
    if not ok:
        log(f"{tag}: local artifacts do not match their recorded checksums ({why}); "
            f"publishing the release page WITHOUT binaries rather than shipping unverified bytes")
        return 0
    status, current = call(tok, "GET",
                           f"{API}/repos/{slug}/releases/{release['id']}/assets?per_page=100")
    existing = {a["name"]: a["id"] for a in current} if status == 200 else {}
    upload_url = release["upload_url"].split("{")[0]
    uploaded = 0
    for path in files:
        if path.name in existing:
            log(f"{tag}: replacing existing asset {path.name}")
            if not dry_run:
                call(tok, "DELETE", f"{API}/repos/{slug}/releases/assets/{existing[path.name]}")
        ctype = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
        url = f"{upload_url}?name={urllib.parse.quote(path.name)}"
        if dry_run:
            log(f"{tag}: would upload {path.name} ({path.stat().st_size} bytes)")
            continue
        st, res = call(tok, "POST", url, raw=path.read_bytes(), content_type=ctype)
        if st not in (200, 201):
            raise SystemExit(f"upload of {path.name} failed: {st} {res.get('message')}")
        log(f"{tag}: uploaded {path.name} ({path.stat().st_size} bytes)")
        uploaded += 1
    return uploaded


def tags_missing(tok, slug):
    tags = subprocess.run(["git", "tag", "--list", "v*", "--sort=v:refname"], cwd=ROOT,
                          text=True, capture_output=True, check=True).stdout.split()
    missing = []
    for tag in tags:
        st, _ = call(tok, "GET", f"{API}/repos/{slug}/releases/tags/{tag}")
        if st == 404:
            missing.append(tag)
        elif st != 200:
            raise SystemExit(f"cannot look up {tag}: {st}")
    return missing


def main():
    ap = argparse.ArgumentParser(description="Upload locally built Aurelia artifacts")
    ap.add_argument("tag", nargs="?")
    ap.add_argument("--all-missing", action="store_true",
                    help="every local tag without a release page")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    if not args.tag and not args.all_missing:
        ap.error("give a tag or --all-missing")

    tok = get_token()
    if not tok and not args.dry_run:
        raise SystemExit("no GitHub token: set AURELIA_GITHUB_TOKEN or make sure 'git push' works")
    slug = repo_slug()
    log(f"repository {slug}")
    targets = tags_missing(tok, slug) if args.all_missing else [args.tag]
    if args.all_missing and not targets:
        log("every tag already has a release page")
    for tag in targets:
        if not (ROOT / "release-notes" / f"{tag}.md").exists():
            log(f"{tag}: skipped (no release-notes/{tag}.md in this repository)")
            continue
        release = ensure_release(tok, slug, tag, args.dry_run)
        upload_assets(tok, slug, tag, release, args.dry_run)
    log("done")


if __name__ == "__main__":
    main()
