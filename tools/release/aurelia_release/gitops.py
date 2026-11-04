"""Thin, injectable wrapper around git plumbing/porcelain commands."""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Sequence

from .errors import CommandError
from .util import redact


@dataclass(frozen=True)
class GitResult:
    returncode: int
    stdout: str
    stderr: str


Runner = Callable[[Sequence[str], Path], GitResult]


class SubprocessRunner:
    def __call__(self, argv: Sequence[str], cwd: Path) -> GitResult:
        result = subprocess.run(
            ["git", *argv],
            cwd=str(cwd),
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
        )
        return GitResult(result.returncode, result.stdout, result.stderr)


class Git:
    def __init__(self, repo: Path, runner: Runner | None = None) -> None:
        self.repo = repo
        self.runner = runner or SubprocessRunner()

    def _run(self, *args: str) -> GitResult:
        return self.runner(list(args), self.repo)

    def _checked(self, *args: str) -> str:
        result = self._run(*args)
        if result.returncode != 0:
            detail = redact((result.stderr or result.stdout).strip())
            raise CommandError(f"git {' '.join(args)} failed: {detail}")
        return result.stdout.strip()

    # ── reads ───────────────────────────────────────────────────────────
    def is_worktree(self) -> bool:
        return self._run("rev-parse", "--is-inside-work-tree").returncode == 0

    def status_porcelain(self) -> str:
        return self._checked("status", "--porcelain")

    def current_branch(self) -> str:
        return self._checked("branch", "--show-current")

    def head(self) -> str:
        return self._checked("rev-parse", "HEAD")

    def commit_subject(self, ref: str) -> str:
        return self._checked("show", "-s", "--format=%s", ref)

    def rev_parse(self, ref: str) -> str | None:
        result = self._run("rev-parse", "--verify", "--quiet", ref)
        return result.stdout.strip() if result.returncode == 0 else None

    def tag_exists(self, tag: str) -> bool:
        return self.rev_parse(f"refs/tags/{tag}") is not None

    def tag_type(self, tag: str) -> str | None:
        result = self._run("cat-file", "-t", f"refs/tags/{tag}")
        return result.stdout.strip() if result.returncode == 0 else None

    def tag_commit(self, tag: str) -> str | None:
        return self.rev_parse(f"refs/tags/{tag}^{{commit}}")

    def remote_url(self, remote: str) -> str | None:
        result = self._run("remote", "get-url", remote)
        return result.stdout.strip() if result.returncode == 0 else None

    def ls_remote(self, remote: str) -> dict[str, str]:
        result = self._run("ls-remote", remote)
        if result.returncode != 0:
            raise CommandError(f"git ls-remote {remote} failed: {redact(result.stderr.strip())}")
        refs: dict[str, str] = {}
        for line in result.stdout.splitlines():
            if not line.strip():
                continue
            sha, _, name = line.partition("\t")
            if name.endswith("^{}"):
                continue
            refs[name] = sha
        return refs

    def verify_tag(self, tag: str) -> None:
        result = self._run("verify-tag", tag)
        if result.returncode != 0:
            detail = redact((result.stderr or result.stdout).strip())
            raise CommandError(f"git verify-tag {tag} failed: {detail}")

    def config_get(self, key: str) -> str | None:
        result = self._run("config", "--get", key)
        return result.stdout.strip() if result.returncode == 0 else None

    def is_ancestor(self, older: str, newer: str) -> bool:
        return self._run("merge-base", "--is-ancestor", older, newer).returncode == 0

    # ── writes ──────────────────────────────────────────────────────────
    def add(self, paths: Sequence[str]) -> None:
        self._checked("add", "--", *paths)

    def commit(self, message: str) -> None:
        self._checked("commit", "-m", message)

    def tag(self, name: str, message: str, *, sign: bool) -> None:
        flags = ["-s"] if sign else ["-a"]
        self._checked("tag", *flags, name, "-m", message)

    def push(self, remote: str, refs: Sequence[str], *, atomic: bool = True) -> None:
        args = ["push"]
        if atomic:
            args.append("--atomic")
        args.extend([remote, *refs])
        self._checked(*args)

    def delete_tag(self, name: str) -> None:
        self._checked("tag", "-d", name)

    def delete_remote_tag(self, remote: str, name: str) -> None:
        self._checked("push", remote, f":refs/tags/{name}")

    def reset_hard(self, rev: str) -> None:
        self._checked("reset", "--hard", rev)

    def checkout(self, paths: Sequence[str]) -> None:
        self._checked("checkout", "--", *paths)
