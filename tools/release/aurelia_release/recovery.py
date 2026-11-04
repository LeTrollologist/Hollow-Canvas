"""Compensation, local rollback, and yank.

Rollback undoes local, unpublished work. Compensation reverses completed remote
mutations of an incomplete release. Yank marks a published release withdrawn;
it never pretends publication did not happen.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .engine import Context, Operation
from .errors import CompensationError, PreflightError
from .journal import OpState


@dataclass
class CompensationStep:
    operation: Operation
    state: OpState
    reason: str


def plan_compensation(
    ctx: Context, operations: list[Operation], *, include_remote: bool = False
) -> list[CompensationStep]:
    states = ctx.journal.state()
    steps: list[CompensationStep] = []
    for operation in reversed(operations):
        state = states.get(operation.name)
        if not state or state.status != "ok" or state.compensated:
            continue
        if operation.compensate is None:
            continue
        if operation.remote and not include_remote:
            continue
        steps.append(
            CompensationStep(
                operation=operation,
                state=state,
                reason=f"undo {operation.name}",
            )
        )
    return steps


def apply_compensation(
    ctx: Context,
    steps: list[CompensationStep],
    *,
    dry_run: bool,
) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for step in steps:
        entry = {"op": step.operation.name, "status": "pending", "error": None}
        if dry_run:
            entry["status"] = "would_compensate"
            results.append(entry)
            continue

        ctx.log(f"  compensate {step.operation.name}: {step.reason}")
        try:
            assert step.operation.compensate is not None
            step.operation.compensate(ctx, step.state)
        except Exception as exc:  # noqa: BLE001 - reported and journaled
            message = str(exc)
            entry["status"] = "failed"
            entry["error"] = message
            ctx.journal.append(
                "compensate",
                op=step.operation.name,
                status="error",
                error=message,
            )
            results.append(entry)
            raise CompensationError(
                f"{step.operation.name}: compensation failed: {message}"
            ) from exc

        ctx.journal.append("compensate", op=step.operation.name, status="ok")
        entry["status"] = "compensated"
        results.append(entry)
    return results


def rollback(ctx: Context, operations: list[Operation]) -> dict[str, Any]:
    """Undo local, unpublished release changes only."""
    states = ctx.journal.state()
    publish = states.get("publish.push")
    if publish and publish.status == "ok":
        raise PreflightError(
            "this release was pushed; use `release yank` or `release recover`, not rollback"
        )
    remote = ctx.cfg.git.remote
    if ctx.git.ls_remote(remote).get(f"refs/tags/{ctx.tag}"):
        raise PreflightError(
            f"tag {ctx.tag} exists on {remote}; rollback is local-only. Use recover/yank."
        )

    base = ctx.journal.base_commit()
    if not base:
        raise PreflightError("journal has no base commit; nothing to roll back to")
    if ctx.git.tag_exists(ctx.tag):
        ctx.git.delete_tag(ctx.tag)
    ctx.git.reset_hard(base)
    ctx.journal.append("rollback", release=ctx.tag, base_commit=base)
    return {"tag": ctx.tag, "base_commit": base}


def yank(ctx: Context, reason: str, *, client: object | None = None) -> dict[str, Any]:
    """Mark a published release withdrawn without rewriting history."""
    if not reason.strip():
        raise PreflightError("a yank reason is required")

    hosted = False
    if client is not None and ctx.cfg.publish.github:
        release = client.get_release(ctx.tag)
        if release:
            body = (release.get("body") or "").rstrip()
            note = f"\n\n> **Yanked**: {reason.strip()}"
            client.update_release(release["id"], body=body + note)
            hosted = True

    ctx.journal.append("yank", release=ctx.tag, reason=reason, hosted=hosted)
    return {"tag": ctx.tag, "reason": reason, "status": "yanked", "hosted": hosted}
