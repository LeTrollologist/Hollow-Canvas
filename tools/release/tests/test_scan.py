from __future__ import annotations

import json

import pytest

from aurelia_release import stages
from aurelia_release.config import ScanConfig, load_config
from aurelia_release.engine import Context
from aurelia_release.github import HttpResponse
from aurelia_release.journal import Journal
from aurelia_release.scan import ScanError, VirusTotalClient, evaluate


def cfg(**kwargs) -> ScanConfig:
    return ScanConfig(**kwargs)


def test_advisory_allows_any_detection():
    decision = evaluate({"EngineA": True, "EngineB": True}, cfg(mode="advisory"), "a" * 64)
    assert decision.allowed is True
    assert decision.detections == 2


def test_threshold_blocks_at_the_configured_count():
    stats = {"A": True, "B": True, "C": False}
    assert evaluate(stats, cfg(mode="threshold", threshold=3), "a" * 64).allowed is True
    assert evaluate(stats, cfg(mode="threshold", threshold=2), "a" * 64).allowed is False


def test_blocking_engine_beats_threshold():
    decision = evaluate(
        {"BadEngine": True},
        cfg(mode="threshold", threshold=99, blocking_engines=("BadEngine",)),
        "a" * 64,
    )
    assert decision.allowed is False
    assert "BadEngine" in decision.reason


def test_strict_blocks_any_detection_and_off_never_blocks():
    assert evaluate({"A": True}, cfg(mode="strict"), "a" * 64).allowed is False
    assert evaluate({"A": True}, cfg(mode="off"), "a" * 64).allowed is True


def test_allowlist_wins():
    decision = evaluate(
        {"A": True}, cfg(mode="strict", allowlist=("b" * 64,)), "B" * 64
    )
    assert decision.allowed is True
    assert "allowlist" in decision.reason


def test_virustotal_client_reads_engine_categories():
    payload = {
        "data": {
            "attributes": {
                "last_analysis_results": {
                    "Good": {"category": "undetected"},
                    "Bad": {"category": "malicious"},
                    "Odd": {"category": "suspicious"},
                }
            }
        }
    }

    def transport(method, url, headers, body):
        return HttpResponse(200, {}, json.dumps(payload).encode())

    client = VirusTotalClient("key", transport=transport)
    assert client.get_report("a" * 64) == {"Good": False, "Bad": True, "Odd": True}


def test_virustotal_client_upload_polls_then_reads_report(tmp_path):
    target = tmp_path / "artifact.zip"
    target.write_bytes(b"payload")
    calls: list[str] = []

    payload = {
        "data": {
            "attributes": {
                "last_analysis_results": {"Good": {"category": "undetected"}}
            }
        }
    }

    def transport(method, url, headers, body):
        calls.append(f"{method} {url}")
        if method == "GET" and "/files/" in url:
            if not any("/analyses/" in call for call in calls):
                return HttpResponse(404, {}, b"{}")
            return HttpResponse(200, {}, json.dumps(payload).encode())
        if method == "POST" and url.endswith("/files"):
            return HttpResponse(200, {}, json.dumps({"data": {"id": "analysis-1"}}).encode())
        if "/analyses/analysis-1" in url:
            return HttpResponse(
                200, {}, json.dumps({"data": {"attributes": {"status": "completed"}}}).encode()
            )
        raise AssertionError(f"unexpected call {method} {url}")

    client = VirusTotalClient("key", transport=transport, sleep=lambda _: None)
    digest, stats = client.scan_file(target)
    assert len(digest) == 64
    assert stats == {"Good": False}
    assert any("POST" in call for call in calls)


def test_virustotal_errors_are_not_silent():
    def transport(method, url, headers, body):
        return HttpResponse(403, {}, b'{"error":"nope"}')

    client = VirusTotalClient("bad", transport=transport)
    with pytest.raises(ScanError):
        client.get_report("a" * 64)


class FakeGit:
    def head(self) -> str:
        return "a" * 40


def make_scan_ctx(tmp_path, mode: str, threshold: int = 3) -> Context:
    (tmp_path / "dist" / "v1.0.0").mkdir(parents=True)
    (tmp_path / "dist" / "v1.0.0" / "a.zip").write_bytes(b"artifact")
    (tmp_path / "release.toml").write_text(
        f"""
[release]
profile = "soft"
version_files = []

[profiles]
soft = ["scan"]

[scan]
mode = "{mode}"
threshold = {threshold}

[publish]
assets = ["dist/{{tag}}/*"]
""",
        encoding="utf-8",
    )
    cfg = load_config(tmp_path, path=tmp_path / "release.toml")
    return Context(
        repo=tmp_path,
        cfg=cfg,
        tag="v1.0.0",
        version="1.0.0",
        journal=Journal(tmp_path / ".release" / "v1.0.0.jsonl"),
        git=FakeGit(),
        profile=cfg.profile,
        log=lambda *_: None,
    )


def test_scan_stage_blocks_before_tagging(tmp_path, monkeypatch):
    ctx = make_scan_ctx(tmp_path, "threshold", threshold=2)
    monkeypatch.setenv("VIRUSTOTAL_API_KEY", "test-key")

    class FakeVT:
        def __init__(self, key):
            assert key == "test-key"

        def scan_file(self, path):
            return "a" * 64, {"A": True, "B": True, "C": False}

    monkeypatch.setattr(stages, "VirusTotalClient", FakeVT)
    with pytest.raises(ScanError):
        stages._scan(ctx)


def test_scan_stage_advisory_records_but_allows(tmp_path, monkeypatch):
    ctx = make_scan_ctx(tmp_path, "advisory")
    monkeypatch.setenv("VIRUSTOTAL_API_KEY", "test-key")

    class FakeVT:
        def __init__(self, key):
            pass

        def scan_file(self, path):
            return "a" * 64, {"Heuristic": True}

    monkeypatch.setattr(stages, "VirusTotalClient", FakeVT)
    result = stages._scan(ctx)
    assert result["scans"][0]["allowed"] is True
    assert result["scans"][0]["detections"] == 1
