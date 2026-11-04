from __future__ import annotations

import pytest

from aurelia_release.config import load_config
from aurelia_release.engine import Context, Engine, Operation
from aurelia_release.errors import VerificationError
from aurelia_release.journal import Journal


class FakeGit:
    def __init__(self, head: str = "a" * 40) -> None:
        self._head = head

    def head(self) -> str:
        return self._head


def make_ctx(tmp_path, *, stages=("version",), dry_run: bool = False) -> Context:
    (tmp_path / "release.toml").write_text(
        f"""
[release]
profile = "test"
version_files = []

[profiles]
test = {list(stages)!r}

[git]
sign_tags = false
""".replace("'", '"'),
        encoding="utf-8",
    )
    cfg = load_config(tmp_path, path=tmp_path / "release.toml")
    return Context(
        repo=tmp_path,
        cfg=cfg,
        tag="v1.0.0",
        version="1.0.0",
        journal=Journal(tmp_path / "journal.jsonl"),
        git=FakeGit(),
        profile=cfg.profile,
        dry_run=dry_run,
        log=lambda *_: None,
    )


def write_operation(counter: dict[str, int]) -> Operation:
    def run(ctx: Context) -> dict:
        counter["runs"] += 1
        (ctx.repo / "out.txt").write_text(f"run {counter['runs']}", encoding="utf-8")
        return {"paths": ["out.txt"]}

    def verify(ctx: Context, outputs: dict) -> dict:
        assert (ctx.repo / "out.txt").read_text(encoding="utf-8").startswith("run ")
        return {"ok": True}

    return Operation(
        name="write",
        stage="version",
        description="write out.txt",
        run=run,
        verify=verify,
    )


def test_second_run_skips_verified_operation(tmp_path):
    ctx = make_ctx(tmp_path)
    counter = {"runs": 0}
    op = write_operation(counter)

    Engine(ctx, [op]).run()
    assert counter["runs"] == 1
    state = ctx.journal.state()["write"]
    assert state.verified

    Engine(ctx, [op]).run()
    assert counter["runs"] == 1, "verified operation must not run twice"


def test_changed_outputs_invalidate_the_skip(tmp_path):
    ctx = make_ctx(tmp_path)
    counter = {"runs": 0}
    op = write_operation(counter)

    Engine(ctx, [op]).run()
    (tmp_path / "out.txt").write_text("tampered", encoding="utf-8")
    Engine(ctx, [op]).run()
    assert counter["runs"] == 2


def test_verification_failure_marks_release_failed(tmp_path):
    ctx = make_ctx(tmp_path)
    counter = {"runs": 0}

    def run(ctx: Context) -> dict:
        counter["runs"] += 1
        (ctx.repo / "out.txt").write_text("bad", encoding="utf-8")
        return {"paths": ["out.txt"]}

    def verify(ctx: Context, outputs: dict) -> dict:
        raise VerificationError("output is not what it should be")

    with pytest.raises(VerificationError):
        Engine(ctx, [Operation("bad", "version", "bad output", run, verify)]).run()

    assert counter["runs"] == 1
    assert ctx.journal.status() == "failed"
    assert "not what it should be" in (ctx.journal.last_error() or "")


def test_interrupted_operation_requires_recover(tmp_path):
    ctx = make_ctx(tmp_path)
    counter = {"runs": 0}
    ctx.journal.append(
        "release",
        release=ctx.tag,
        version=ctx.version,
        profile=ctx.cfg.profile,
        base_commit="a" * 40,
        config_digest=ctx.cfg.digest,
    )
    ctx.journal.append("intent", op="write", stage="version", fp="stale", before={})

    with pytest.raises(VerificationError):
        Engine(ctx, [write_operation(counter)]).run()
    assert counter["runs"] == 0


def test_dry_run_does_not_write_the_journal(tmp_path):
    ctx = make_ctx(tmp_path, dry_run=True)
    counter = {"runs": 0}
    Engine(ctx, [write_operation(counter)]).run()
    assert counter["runs"] == 0
    assert ctx.journal.records() == []
