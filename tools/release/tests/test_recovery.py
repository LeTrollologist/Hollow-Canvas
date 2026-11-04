from __future__ import annotations

import pytest

from aurelia_release.config import load_config
from aurelia_release.engine import Context, Engine, Operation
from aurelia_release.errors import CompensationError, PreflightError
from aurelia_release.journal import Journal
from aurelia_release.recovery import (
    apply_compensation,
    plan_compensation,
    rollback,
    yank,
)


class FakeGit:
    def __init__(self) -> None:
        self._head = "c" * 40
        self.reset_to: str | None = None
        self.deleted: list[str] = []

    def head(self) -> str:
        return self._head

    def ls_remote(self, remote: str) -> dict:
        return {}

    def tag_exists(self, tag: str) -> bool:
        return False

    def delete_tag(self, tag: str) -> None:
        self.deleted.append(tag)

    def reset_hard(self, rev: str) -> None:
        self.reset_to = rev
        self._head = rev


def make_ctx(tmp_path) -> Context:
    (tmp_path / "release.toml").write_text(
        """
[release]
profile = "test"
version_files = []

[profiles]
test = ["version"]

[git]
sign_tags = false
""",
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
        log=lambda *_: None,
    )


def op(name: str, calls: list[str], *, fail: bool = False) -> Operation:
    def run(ctx: Context) -> dict:
        (ctx.repo / f"{name}.txt").write_text(name, encoding="utf-8")
        return {"paths": [f"{name}.txt"]}

    def verify(ctx: Context, outputs: dict) -> dict:
        return {}

    def compensate(ctx: Context, state) -> None:
        calls.append(name)
        if fail:
            raise CompensationError("cannot undo")

    return Operation(name, "version", f"write {name}", run, verify, compensate=compensate)


def test_compensation_plan_is_reverse_order_and_idempotent(tmp_path):
    ctx = make_ctx(tmp_path)
    calls: list[str] = []
    operations = [op("a", calls), op("b", calls)]
    Engine(ctx, operations).run()

    plan = plan_compensation(ctx, operations)
    assert [step.operation.name for step in plan] == ["b", "a"]

    apply_compensation(ctx, plan, dry_run=False)
    assert calls == ["b", "a"]
    assert all(state.compensated for state in ctx.journal.state().values())
    assert plan_compensation(ctx, operations) == []


def test_compensation_failure_stops_and_is_journaled(tmp_path):
    ctx = make_ctx(tmp_path)
    calls: list[str] = []
    operations = [op("a", calls), op("b", calls, fail=True)]
    Engine(ctx, operations).run()

    plan = plan_compensation(ctx, operations)
    with pytest.raises(CompensationError):
        apply_compensation(ctx, plan, dry_run=False)

    states = ctx.journal.state()
    assert states["b"].compensated is False
    assert states["b"].compensation_error
    assert states["a"].compensated is False


def test_rollback_resets_to_base_when_nothing_was_pushed(tmp_path):
    ctx = make_ctx(tmp_path)
    ctx.journal.append(
        "release",
        release=ctx.tag,
        version=ctx.version,
        profile=ctx.cfg.profile,
        base_commit="b" * 40,
        config_digest=ctx.cfg.digest,
    )
    result = rollback(ctx, [])
    assert result["base_commit"] == "b" * 40
    assert ctx.git.reset_to == "b" * 40
    assert ctx.journal.status() == "rolled_back"


def test_rollback_refuses_published_releases(tmp_path):
    ctx = make_ctx(tmp_path)
    ctx.journal.append(
        "release",
        release=ctx.tag,
        version=ctx.version,
        profile=ctx.cfg.profile,
        base_commit="b" * 40,
        config_digest=ctx.cfg.digest,
    )
    ctx.journal.append(
        "result", op="publish.push", stage="publish", status="ok", outputs={"files": {}}
    )
    with pytest.raises(PreflightError):
        rollback(ctx, [])


def test_yank_marks_withdrawn_and_requires_reason(tmp_path):
    ctx = make_ctx(tmp_path)
    ctx.journal.append("release", release=ctx.tag, version=ctx.version, profile="git",
                       base_commit="b" * 40, config_digest=ctx.cfg.digest)
    with pytest.raises(PreflightError):
        yank(ctx, "")
    yank(ctx, "broken installer")
    assert ctx.journal.status() == "yanked"


def test_yank_updates_the_hosted_release_body(tmp_path):
    ctx = make_ctx(tmp_path)
    ctx.journal.append("release", release=ctx.tag, version=ctx.version, profile="git",
                       base_commit="b" * 40, config_digest=ctx.cfg.digest)

    class FakeClient:
        def __init__(self):
            self.fields = None

        def get_release(self, tag):
            return {"id": 9, "body": "release notes"}

        def update_release(self, release_id, **fields):
            self.fields = fields
            return {"id": release_id, **fields}

    client = FakeClient()
    result = yank(ctx, "withdrawn", client=client)
    assert result["hosted"] is True
    assert client.fields["body"].startswith("release notes")
    assert "Yanked" in client.fields["body"]
    assert ctx.journal.status() == "yanked"
