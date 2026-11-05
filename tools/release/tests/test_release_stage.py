from __future__ import annotations

from pathlib import Path

import pytest

from aurelia_release import stages
from aurelia_release.config import load_config
from aurelia_release.engine import Context, Engine
from aurelia_release.journal import Journal
from aurelia_release.stages import build_operations


class FakeGit:
    def head(self) -> str:
        return "a" * 40

    def remote_url(self, remote: str) -> str:
        return "https://github.com/o/r.git"


class FakeGitHub:
    def __init__(self) -> None:
        self.release: dict | None = None
        self.assets: dict[int, dict] = {}
        self.paths: dict[int, Path] = {}
        self._next = 1
        self.fail_upload_after: int | None = None
        self.fail_create = False
        self.corrupt_download = False

    def get_release(self, tag: str) -> dict | None:
        return dict(self.release) if self.release else None

    def create_release(self, tag: str, *, name: str, body: str, draft: bool) -> dict:
        if self.fail_create:
            raise RuntimeError("simulated GitHub release creation failure")
        self.release = {"id": 1, "tag_name": tag, "name": name, "body": body, "draft": draft}
        return dict(self.release)

    def update_release(self, release_id: int, **fields) -> dict:
        assert self.release and self.release["id"] == release_id
        self.release.update(fields)
        return dict(self.release)

    def list_assets(self, release_id: int) -> list[dict]:
        return [dict(asset) for asset in self.assets.values()]

    def upload_asset(self, release_id: int, path: Path) -> dict:
        if (
            self.fail_upload_after is not None
            and len(self.assets) >= self.fail_upload_after
        ):
            raise RuntimeError("simulated upload connection reset")
        asset = {
            "id": self._next,
            "name": path.name,
            "size": path.stat().st_size,
            "digest": None,
        }
        self._next += 1
        self.assets[asset["id"]] = asset
        self.paths[asset["id"]] = path
        return dict(asset)

    def download_asset(self, asset_id: int) -> bytes:
        if self.corrupt_download:
            return b"corrupted bytes"
        return self.paths[asset_id].read_bytes()

    def delete_asset(self, asset_id: int) -> None:
        self.assets.pop(asset_id, None)
        self.paths.pop(asset_id, None)

    def delete_release(self, release_id: int) -> None:
        self.release = None


def make_ctx(tmp_path) -> Context:
    (tmp_path / "release.toml").write_text(
        """
[release]
profile = "rel"
version_files = []

[profiles]
rel = ["release"]

[publish]
repo = "o/r"
assets = ["dist/{tag}/*"]

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
        journal=Journal(tmp_path / ".release" / "v1.0.0.jsonl"),
        git=FakeGit(),
        profile=cfg.profile,
        log=lambda *_: None,
    )


def test_release_stage_creates_uploads_and_publishes(tmp_path, monkeypatch):
    artifacts = tmp_path / "dist" / "v1.0.0"
    artifacts.mkdir(parents=True)
    (artifacts / "a.zip").write_bytes(b"binary-a")
    (artifacts / "b.zip").write_bytes(b"binary-b")

    fake = FakeGitHub()
    monkeypatch.setattr(stages, "client_for_context", lambda ctx: fake)

    ctx = make_ctx(tmp_path)
    engine = Engine(ctx, build_operations(ctx))
    engine.run()

    assert fake.release is not None
    assert fake.release["draft"] is False
    assert {asset["name"] for asset in fake.assets.values()} == {"a.zip", "b.zip"}
    assert ctx.journal.state()["release.publish"].verified
    assert ctx.journal.status() == "published"

    # The independent verification downloads the bytes again.
    stages._release_verify_check(ctx, {})

    # Re-verifying the draft operation after publication must accept the
    # published state: the initial draft check happened before release.publish.
    stages._release_draft_verify(ctx, ctx.journal.state()["release.draft"].outputs)

    # A second run must not re-upload or duplicate assets.
    before = len(fake.assets)
    Engine(ctx, build_operations(ctx)).run()
    assert len(fake.assets) == before


def test_release_draft_is_compensated_when_not_published(tmp_path, monkeypatch):
    artifacts = tmp_path / "dist" / "v1.0.0"
    artifacts.mkdir(parents=True)
    (artifacts / "a.zip").write_bytes(b"binary")

    fake = FakeGitHub()
    monkeypatch.setattr(stages, "client_for_context", lambda ctx: fake)
    ctx = make_ctx(tmp_path)

    # Run only the draft operation directly, then compensate it.
    draft = next(op for op in build_operations(ctx) if op.name == "release.draft")
    draft.run(ctx)
    ctx.journal.append(
        "result", op="release.draft", stage="release", status="ok",
        outputs={"files": {}, "meta": {"release_id": fake.release["id"], "created": True}},
    )
    state = ctx.journal.state()["release.draft"]
    state.before = {"release_id": None}
    draft.compensate(ctx, state)
    assert fake.release is None


def test_published_release_refuses_deletion(tmp_path, monkeypatch):
    fake = FakeGitHub()
    fake.release = {"id": 1, "tag_name": "v1.0.0", "draft": False}
    monkeypatch.setattr(stages, "client_for_context", lambda ctx: fake)
    ctx = make_ctx(tmp_path)

    from aurelia_release.errors import CompensationError

    with pytest.raises(CompensationError):
        stages._release_publish_compensate(ctx, type("S", (), {"before": {"draft": True}})())


def test_partial_upload_failure_resumes_without_duplicates(tmp_path, monkeypatch):
    artifacts = tmp_path / "dist" / "v1.0.0"
    artifacts.mkdir(parents=True)
    for name in ("a.zip", "b.zip", "c.zip"):
        (artifacts / name).write_bytes(name.encode())

    fake = FakeGitHub()
    fake.fail_upload_after = 1
    monkeypatch.setattr(stages, "client_for_context", lambda ctx: fake)
    ctx = make_ctx(tmp_path)

    from aurelia_release.errors import StageError

    with pytest.raises(StageError):
        Engine(ctx, build_operations(ctx)).run()

    assert fake.release is not None and fake.release["draft"], "failure must leave a draft"
    assert len(fake.assets) == 1, "the first asset was uploaded before the disconnect"
    assert ctx.journal.status() == "failed"

    fake.fail_upload_after = None
    assert Engine(ctx, build_operations(ctx)).run() == "published"
    assert {asset["name"] for asset in fake.assets.values()} == {"a.zip", "b.zip", "c.zip"}
    assert len(fake.assets) == 3, "resume must not duplicate the already-uploaded asset"


def test_upload_digest_mismatch_blocks_publication_and_can_resume(tmp_path, monkeypatch):
    artifacts = tmp_path / "dist" / "v1.0.0"
    artifacts.mkdir(parents=True)
    (artifacts / "a.zip").write_bytes(b"binary")

    fake = FakeGitHub()
    fake.corrupt_download = True
    monkeypatch.setattr(stages, "client_for_context", lambda ctx: fake)
    ctx = make_ctx(tmp_path)

    from aurelia_release.errors import VerificationError

    with pytest.raises(VerificationError):
        Engine(ctx, build_operations(ctx)).run()

    assert fake.release is not None and fake.release["draft"], "a bad digest must not publish"
    assert ctx.journal.status() == "failed"

    fake.corrupt_download = False
    result = Engine(ctx, build_operations(ctx)).run()
    assert result == "published", (
        f"state={ctx.journal.status()} error={ctx.journal.last_error()} "
        f"tail={ctx.journal.records()[-6:]}"
    )


def test_release_creation_failure_leaves_a_resumable_draft(tmp_path, monkeypatch):
    artifacts = tmp_path / "dist" / "v1.0.0"
    artifacts.mkdir(parents=True)
    (artifacts / "a.zip").write_bytes(b"binary")

    fake = FakeGitHub()
    fake.fail_create = True
    monkeypatch.setattr(stages, "client_for_context", lambda ctx: fake)
    ctx = make_ctx(tmp_path)

    from aurelia_release.errors import StageError

    with pytest.raises(StageError):
        Engine(ctx, build_operations(ctx)).run()
    assert fake.release is None
    assert ctx.journal.status() == "failed"

    fake.fail_create = False
    assert Engine(ctx, build_operations(ctx)).run() == "published"
    assert fake.release is not None and not fake.release["draft"]
