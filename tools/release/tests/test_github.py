from __future__ import annotations

import json
from pathlib import Path

from aurelia_release.github import GitHubClient, HttpResponse, infer_repo


def test_infer_repo_handles_ssh_and_https():
    assert infer_repo("git@github.com:LeTrollologist/Aurelia.git") == "LeTrollologist/Aurelia"
    assert infer_repo("https://github.com/LeTrollologist/Aurelia") == "LeTrollologist/Aurelia"
    assert infer_repo("https://gitlab.com/owner/repo") == ""


def test_missing_release_is_none():
    client = GitHubClient(
        "o/r",
        "token",
        api_base="https://api.test",
        transport=lambda method, url, headers, body: HttpResponse(404, {}, b"{}"),
    )
    assert client.get_release("v1.0.0") is None


def test_create_release_posts_the_expected_payload():
    seen: dict = {}

    def transport(method, url, headers, body):
        seen["method"] = method
        seen["url"] = url
        seen["payload"] = json.loads(body)
        return HttpResponse(201, {}, json.dumps({"id": 7, "tag_name": "v1.0.0", "draft": True}).encode())

    client = GitHubClient("o/r", "token", api_base="https://api.test", transport=transport)
    release = client.create_release("v1.0.0", name="v1.0.0", body="", draft=True)
    assert release["id"] == 7
    assert seen["method"] == "POST"
    assert seen["url"].endswith("/repos/o/r/releases")
    assert seen["payload"] == {"tag_name": "v1.0.0", "name": "v1.0.0", "body": "", "draft": True}


def test_upload_lists_and_downloads_assets(tmp_path):
    artifact = tmp_path / "a.zip"
    artifact.write_bytes(b"binary")
    state = {"assets": []}

    def transport(method, url, headers, body):
        if method == "POST" and "/assets?name=" in url:
            asset = {
                "id": 11,
                "name": "a.zip",
                "size": len(b"binary"),
                "digest": "sha256:" + "0" * 64,
            }
            state["assets"].append(asset)
            return HttpResponse(201, {}, json.dumps(asset).encode())
        if method == "GET" and "/releases/7/assets" in url:
            return HttpResponse(200, {}, json.dumps(state["assets"]).encode())
        if method == "GET" and "/releases/assets/11" in url:
            return HttpResponse(200, {}, b"binary")
        raise AssertionError(f"unexpected {method} {url}")

    client = GitHubClient(
        "o/r",
        "token",
        api_base="https://api.test",
        uploads_base="https://uploads.test",
        transport=transport,
    )
    uploaded = client.upload_asset(7, artifact)
    assert uploaded["id"] == 11
    assert client.list_assets(7)[0]["name"] == "a.zip"
    assert client.download_asset(11) == b"binary"
