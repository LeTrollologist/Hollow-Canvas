from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from aurelia_release.config import load_config
from aurelia_release.engine import Context, Engine
from aurelia_release.gitops import Git, GitResult
from aurelia_release.journal import Journal
from aurelia_release.stages import build_operations


def git(cwd: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=str(cwd),
        text=True,
        encoding="utf-8",
        capture_output=True,
    )
    if result.returncode != 0:
        raise AssertionError(f"git {' '.join(args)} failed:\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip()


class LocalTransportGit(Git):
    """Real git for everything local, in-memory refs for the remote.

    The Windows sandbox blocks the MSYS helper that git's local-file transport
    spawns, so this keeps the end-to-end test meaningful without weakening what
    the orchestrator itself does.
    """

    def __init__(self, repo: Path) -> None:
        super().__init__(repo)
        self.remote_refs = {"refs/heads/main": self.head()}

    def ls_remote(self, remote: str) -> dict[str, str]:
        return dict(self.remote_refs)

    def push(self, remote: str, refs, *, atomic: bool = True) -> None:
        for spec in refs:
            source, separator, target = spec.partition(":")
            if not separator:
                target = source
            if source == "HEAD":
                sha = self.head()
            else:
                sha = self.rev_parse(source)
            assert sha, f"cannot resolve {source}"
            self.remote_refs[target] = sha


def make_repo(tmp_path: Path) -> Path:
    work = tmp_path / "work"
    work.mkdir()
    git(work, "init", "-b", "main")
    git(work, "config", "user.name", "Release Test")
    git(work, "config", "user.email", "release@example.com")

    (work / "Cargo.toml").write_text(
        '[package]\nname = "aurelia-test"\nversion = "0.0.1"\n', encoding="utf-8"
    )
    (work / "README.md").write_text("# Test\n\nRelease-v0.0.1\n", encoding="utf-8")
    (work / "Makefile").write_text("TAG ?= v0.0.1\n", encoding="utf-8")
    (work / ".gitignore").write_text(".release/\ndist/\n", encoding="utf-8")
    git(work, "add", ".")
    git(work, "commit", "-m", "initial")
    git(work, "remote", "add", "origin", "https://example.invalid/aurelia.git")
    return work


def make_context(work: Path, tmp_path: Path) -> Context:
    config = tmp_path / "release.toml"
    config.write_text(
        """
[release]
profile = "git"
version_files = ["Cargo.toml", "README.md", "Makefile"]

[git]
sign_tags = false
""",
        encoding="utf-8",
    )
    cfg = load_config(work, path=config)
    return Context(
        repo=work,
        cfg=cfg,
        tag="v0.1.0",
        version="0.1.0",
        journal=Journal(work / ".release" / "journal" / "v0.1.0.jsonl"),
        git=LocalTransportGit(work),
        profile=cfg.profile,
        log=lambda *_: None,
    )


@pytest.mark.skipif(
    subprocess.run(["git", "--version"], capture_output=True).returncode != 0,
    reason="git is required",
)
def test_git_profile_end_to_end_and_idempotent(tmp_path):
    work = make_repo(tmp_path)
    ctx = make_context(work, tmp_path)

    engine = Engine(ctx, build_operations(ctx))
    assert engine.run() == "verified"

    head = git(work, "rev-parse", "HEAD")
    assert git(work, "log", "-1", "--format=%s") == "chore(release): v0.1.0"
    assert 'version = "0.1.0"' in (work / "Cargo.toml").read_text(encoding="utf-8")
    assert git(work, "rev-parse", "refs/tags/v0.1.0^{commit}") == head
    assert ctx.git.remote_refs["refs/heads/main"] == head
    assert ctx.git.remote_refs["refs/tags/v0.1.0"] == git(work, "rev-parse", "refs/tags/v0.1.0")

    second = Engine(ctx, build_operations(ctx))
    assert second.run() == "verified"
    assert git(work, "rev-parse", "HEAD") == head, "a verified release must not create another commit"

    # Simulate a lost remote tag: the probe must notice and republish it.
    del ctx.git.remote_refs["refs/tags/v0.1.0"]
    third = Engine(ctx, build_operations(ctx))
    assert third.run() == "verified"
    assert ctx.git.remote_refs["refs/tags/v0.1.0"] == git(work, "rev-parse", "refs/tags/v0.1.0")


def test_push_command_is_atomic_and_explicit():
    calls: list[list[str]] = []

    def runner(argv, cwd):
        calls.append(list(argv))
        return GitResult(0, "", "")

    Git(Path("."), runner).push("origin", ["HEAD:refs/heads/main", "refs/tags/v1.0.0"])
    assert calls == [["push", "--atomic", "origin", "HEAD:refs/heads/main", "refs/tags/v1.0.0"]]
