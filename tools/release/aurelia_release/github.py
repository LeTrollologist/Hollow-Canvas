"""Minimal GitHub Releases client with an injectable HTTP transport."""

from __future__ import annotations

import json
import os
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .errors import ReleaseError

API_BASE = "https://api.github.com"
UPLOADS_BASE = "https://uploads.github.com"
USER_AGENT = "aurelia-release/0.1"


class GitHubError(ReleaseError):
    """GitHub returned an error the operator needs to see."""


@dataclass(frozen=True)
class HttpResponse:
    status: int
    headers: dict[str, str]
    body: bytes


HttpTransport = Callable[[str, str, dict[str, str], bytes | None], HttpResponse]


def _urllib_transport(
    method: str, url: str, headers: dict[str, str], body: bytes | None
) -> HttpResponse:
    request = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            return HttpResponse(response.status, dict(response.headers), response.read())
    except urllib.error.HTTPError as exc:
        return HttpResponse(exc.code, dict(exc.headers or {}), exc.read())


def infer_repo(remote_url: str) -> str:
    """Turn git@github.com:owner/repo.git or https://github.com/owner/repo into owner/repo."""
    match = re.search(r"github\.com[:/]([^/]+)/([^/]+?)(?:\.git)?/?$", remote_url.strip())
    if not match:
        return ""
    return f"{match.group(1)}/{match.group(2)}"


class GitHubClient:
    def __init__(
        self,
        repo: str,
        token: str,
        *,
        api_base: str = API_BASE,
        uploads_base: str = UPLOADS_BASE,
        transport: HttpTransport | None = None,
    ) -> None:
        if "/" not in repo:
            raise GitHubError(f"invalid repository {repo!r}; expected owner/name")
        self.repo = repo
        self.token = token
        self.api_base = api_base.rstrip("/")
        self.uploads_base = uploads_base.rstrip("/")
        self.transport = transport or _urllib_transport

    # ── transport ───────────────────────────────────────────────────────
    def _request(
        self,
        method: str,
        url: str,
        *,
        body: bytes | None = None,
        headers: dict[str, str] | None = None,
    ) -> HttpResponse:
        merged = {
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": USER_AGENT,
        }
        if self.token:
            merged["Authorization"] = f"Bearer {self.token}"
        if body is not None and "Content-Type" not in merged:
            merged["Content-Type"] = "application/json"
        if headers:
            merged.update(headers)
        return self.transport(method, url, merged, body)

    def _json(self, method: str, url: str, payload: dict | None = None) -> dict:
        body = json.dumps(payload).encode("utf-8") if payload is not None else None
        response = self._request(method, url, body=body)
        if response.status == 404:
            return {}
        if response.status not in {200, 201, 204}:
            detail = response.body.decode("utf-8", "replace")[:400]
            raise GitHubError(f"{method} {url}: HTTP {response.status}: {detail}")
        if response.status == 204 or not response.body:
            return {}
        try:
            return json.loads(response.body)
        except json.JSONDecodeError as exc:
            raise GitHubError(f"{method} {url}: invalid JSON response") from exc

    # ── releases ────────────────────────────────────────────────────────
    def get_release(self, tag: str) -> dict | None:
        data = self._json("GET", f"{self.api_base}/repos/{self.repo}/releases/tags/{tag}")
        if data:
            return data
        # Draft releases are not returned by the tag endpoint. GitHub's release
        # list can also lag a moment behind the create response, so retry briefly.
        for attempt in range(20):
            releases = self._json(
                "GET", f"{self.api_base}/repos/{self.repo}/releases?per_page=100"
            )
            if isinstance(releases, list):
                for release in releases:
                    if release.get("tag_name") == tag:
                        return release
            if attempt < 19:
                time.sleep(0.5)
        return None

    def create_release(self, tag: str, *, name: str, body: str, draft: bool) -> dict:
        try:
            release = self._json(
                "POST",
                f"{self.api_base}/repos/{self.repo}/releases",
                {"tag_name": tag, "name": name, "body": body, "draft": draft},
            )
        except GitHubError:
            existing = self.get_release(tag)
            if existing:
                return existing
            raise
        if release:
            return release
        existing = self.get_release(tag)
        if existing:
            return existing
        raise GitHubError(f"could not create release for tag {tag}")

    def update_release(self, release_id: int, **fields: object) -> dict:
        return self._json(
            "PATCH",
            f"{self.api_base}/repos/{self.repo}/releases/{release_id}",
            dict(fields),
        )

    def delete_release(self, release_id: int) -> None:
        self._json("DELETE", f"{self.api_base}/repos/{self.repo}/releases/{release_id}")

    # ── assets ─────────────────────────────────────────────────────────
    def list_assets(self, release_id: int) -> list[dict]:
        assets: list[dict] = []
        page = 1
        while True:
            data = self._json(
                "GET",
                f"{self.api_base}/repos/{self.repo}/releases/{release_id}/assets?per_page=100&page={page}",
            )
            batch = data if isinstance(data, list) else []
            assets.extend(batch)
            if len(batch) < 100:
                return assets
            page += 1

    def upload_asset(self, release_id: int, path: Path) -> dict:
        query = urllib.parse.urlencode({"name": path.name})
        url = (
            f"{self.uploads_base}/repos/{self.repo}/releases/{release_id}/assets?{query}"
        )
        body = path.read_bytes()
        response = self._request(
            "POST",
            url,
            body=body,
            headers={"Content-Type": "application/octet-stream"},
        )
        if response.status in {200, 201}:
            return json.loads(response.body)
        if response.status == 422:
            for asset in self.list_assets(release_id):
                if asset.get("name") == path.name:
                    return asset
        detail = response.body.decode("utf-8", "replace")[:400]
        raise GitHubError(f"upload {path.name}: HTTP {response.status}: {detail}")

    def delete_asset(self, asset_id: int) -> None:
        self._json("DELETE", f"{self.api_base}/repos/{self.repo}/releases/assets/{asset_id}")

    def download_asset(self, asset_id: int) -> bytes:
        response = self._request(
            "GET",
            f"{self.api_base}/repos/{self.repo}/releases/assets/{asset_id}",
            headers={"Accept": "application/octet-stream"},
        )
        if response.status != 200:
            raise GitHubError(
                f"download asset {asset_id}: HTTP {response.status}: "
                f"{response.body.decode('utf-8', 'replace')[:300]}"
            )
        return response.body


def client_for_context(ctx: object) -> GitHubClient:
    """Build a client from ctx.cfg.publish, raising a clear configuration error."""
    cfg = getattr(ctx, "cfg")
    git = getattr(ctx, "git")
    publish = cfg.publish
    repo = publish.repo or infer_repo(git.remote_url(cfg.git.remote) or "")
    if not repo:
        raise GitHubError(
            "cannot determine the GitHub repository; set [publish].repo = \"owner/name\""
        )
    token = os.environ.get(publish.token_env, "") or os.environ.get("GH_TOKEN", "")
    if not token:
        raise GitHubError(
            f"no GitHub token found; set {publish.token_env} (or GH_TOKEN)"
        )
    return GitHubClient(repo, token)
