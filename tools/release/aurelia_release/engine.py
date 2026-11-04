"""Journal-backed stage engine: check, run, verify, reconcile, skip.

The engine never treats "the command exited zero" as success. It records a
write-ahead intent, executes, hashes outputs, and then runs an independent
verification before marking an operation complete.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

from .config import ReleaseConfig
from .errors import ConfigError, StageError, VerificationError
from .gitops import Git
from .journal import Journal, OpState
from .util import find_iscc, fingerprint, redact, sha256_file


@dataclass
class Context:
    repo: Path
    cfg: ReleaseConfig
    tag: str
    version: str
    journal: Journal
    git: Git
    profile: str
    dry_run: bool = False
    log: Callable[[str], None] = print
    base_commit: str = ""

    @property
    def dist(self) -> Path:
        return self.repo / "dist" / self.tag

    @property
    def replacements(self) -> dict[str, str]:
        return {
            "tag": self.tag,
            "version": self.version,
            "dist": self.dist.as_posix(),
            "repo": self.repo.as_posix(),
            "iscc": str(find_iscc() or ""),
        }


@dataclass
class Operation:
    name: str
    stage: str
    description: str
    run: Callable[[Context], dict[str, Any]]
    verify: Callable[[Context, dict[str, Any]], dict[str, Any] | None]
    compensate: Callable[[Context, OpState], None] | None = None
    snapshot: Callable[[Context], dict[str, Any]] | None = None
    probe: Callable[[Context], bool] | None = None
    params: dict[str, Any] = field(default_factory=dict)
    remote: bool = False
    always_run: bool = False


class Engine:
    def __init__(self, ctx: Context, operations: list[Operation]) -> None:
        self.ctx = ctx
        self.operations = operations
        self.by_stage: dict[str, list[Operation]] = {}
        for operation in operations:
            self.by_stage.setdefault(operation.stage, []).append(operation)
        self._check_profile()

    def _check_profile(self) -> None:
        for stage in self.ctx.cfg.stages():
            if not self.by_stage.get(stage):
                raise ConfigError(
                    f"profile {self.ctx.cfg.profile!r} enables stage {stage!r} "
                    "but this build has no operations for it"
                )

    # ── release record ──────────────────────────────────────────────────
    def _ensure_release(self) -> None:
        record = self.ctx.journal.release_record()
        if record is None:
            self.ctx.base_commit = self.ctx.git.head()
            if self.ctx.dry_run:
                return
            self.ctx.journal.append(
                "release",
                release=self.ctx.tag,
                version=self.ctx.version,
                profile=self.ctx.cfg.profile,
                base_commit=self.ctx.base_commit,
                config_digest=self.ctx.cfg.digest,
            )
            return

        if record.get("release") != self.ctx.tag:
            raise ConfigError(
                f"journal belongs to {record.get('release')!r}, not {self.ctx.tag!r}"
            )
        if record.get("config_digest") != self.ctx.cfg.digest:
            raise ConfigError(
                "release.toml changed since this release started; "
                "start a new tag or restore the original configuration"
            )
        if record.get("profile") != self.ctx.cfg.profile:
            raise ConfigError(
                f"release started with profile {record.get('profile')!r}, "
                f"but {self.ctx.cfg.profile!r} was requested"
            )
        self.ctx.base_commit = str(record["base_commit"])

    # ── skip logic ──────────────────────────────────────────────────────
    def _fingerprint(self, operation: Operation) -> str:
        return fingerprint(
            {
                "release": self.ctx.tag,
                "op": operation.name,
                "stage": operation.stage,
                "params": operation.params,
                "config": self.ctx.cfg.digest,
                "base_commit": self.ctx.base_commit,
                "profile": self.ctx.cfg.profile,
            }
        )

    def _files_match(self, files: dict[str, str]) -> bool:
        for rel, expected in files.items():
            path = self.ctx.repo / rel
            if not path.is_file():
                return False
            if sha256_file(path) != expected:
                return False
        return True

    def _action_for(self, operation: Operation, state: OpState | None) -> str:
        if state is None:
            return "run"
        if state.fp != self._fingerprint(operation):
            return "run"
        if state.status != "ok":
            return "run"
        if not self._files_match((state.outputs or {}).get("files", {})):
            return "run"
        if operation.probe is not None and not operation.probe(self.ctx):
            return "run"
        if state.verified:
            return "skip"
        return "verify"

    # ── execution ───────────────────────────────────────────────────────
    def run(self) -> str:
        self._ensure_release()
        for stage in self.ctx.cfg.stages():
            self.ctx.log(f"\n== {stage} ==")
            for operation in self.by_stage[stage]:
                self._execute(operation)
        return self.ctx.journal.status()

    def _execute(self, operation: Operation) -> None:
        states = self.ctx.journal.state()
        state = states.get(operation.name)
        action = self._action_for(operation, state)

        if action == "skip" and not operation.always_run:
            self.ctx.log(f"  {operation.name}: skip (verified)")
            return

        if action == "verify" and not operation.always_run:
            self.ctx.log(f"  {operation.name}: verify existing result")
            details = self._verify(operation, state.outputs if state else {})
            self.ctx.journal.append(
                "verify",
                op=operation.name,
                stage=operation.stage,
                status="passed",
                details=details,
            )
            return

        if state and state.status == "running":
            raise VerificationError(
                f"{operation.name}: a previous run was interrupted mid-operation. "
                "Run `release status` then `release recover`; do not resume blindly."
            )

        if self.ctx.dry_run:
            self.ctx.log(f"  {operation.name}: would run ({operation.description})")
            return

        fp = self._fingerprint(operation)
        before = operation.snapshot(self.ctx) if operation.snapshot else {}
        self.ctx.journal.append(
            "intent",
            op=operation.name,
            stage=operation.stage,
            fp=fp,
            before=before,
        )
        self.ctx.log(f"  {operation.name}: run")

        try:
            result = operation.run(self.ctx) or {}
        except Exception as exc:  # noqa: BLE001 - journal the concrete failure
            message = redact(str(exc))
            self.ctx.journal.append(
                "result",
                op=operation.name,
                stage=operation.stage,
                status="error",
                error=message,
            )
            raise StageError(f"{operation.name}: {message}") from exc

        paths = [str(path) for path in result.get("paths", [])]
        files: dict[str, str] = {}
        for rel in paths:
            path = self.ctx.repo / rel
            if not path.is_file():
                raise VerificationError(
                    f"{operation.name}: declared output {rel} does not exist"
                )
            files[rel] = sha256_file(path)
        meta = {key: value for key, value in result.items() if key != "paths"}
        outputs = {"files": files, "meta": meta}
        self.ctx.journal.append(
            "result",
            op=operation.name,
            stage=operation.stage,
            status="ok",
            outputs=outputs,
        )

        details = self._verify(operation, outputs)
        self.ctx.journal.append(
            "verify",
            op=operation.name,
            stage=operation.stage,
            status="passed",
            details=details,
        )

    def _verify(self, operation: Operation, outputs: dict[str, Any]) -> dict[str, Any]:
        try:
            return operation.verify(self.ctx, outputs) or {}
        except Exception as exc:  # noqa: BLE001 - verification failure is a release failure
            message = redact(str(exc))
            self.ctx.journal.append(
                "verify",
                op=operation.name,
                stage=operation.stage,
                status="failed",
                error=message,
                details={},
            )
            raise VerificationError(f"{operation.name}: {message}") from exc

    # ── read-only views ─────────────────────────────────────────────────
    def plan(self) -> list[dict[str, Any]]:
        states = self.ctx.journal.state()
        rows: list[dict[str, Any]] = []
        for stage in self.ctx.cfg.stages():
            for operation in self.by_stage[stage]:
                state = states.get(operation.name)
                rows.append(
                    {
                        "stage": stage,
                        "op": operation.name,
                        "description": operation.description,
                        "remote": operation.remote,
                        "action": self._action_for(operation, state),
                        "verified": bool(state and state.verified),
                    }
                )
        return rows

    def verify_all(self) -> list[dict[str, Any]]:
        """Re-run every recorded verification; never mutates."""
        results: list[dict[str, Any]] = []
        for operation in self.operations:
            states = self.ctx.journal.state()
            state = states.get(operation.name)
            if not state or state.status != "ok":
                continue
            details = self._verify(operation, state.outputs)
            self.ctx.journal.append(
                "verify",
                op=operation.name,
                stage=operation.stage,
                status="passed",
                details=details,
            )
            results.append({"op": operation.name, "status": "passed", "details": details})
        return results
