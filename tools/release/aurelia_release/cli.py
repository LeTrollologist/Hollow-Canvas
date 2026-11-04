"""Command-line interface for the Aurelia release orchestrator."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from .config import ReleaseConfig, load_config
from .engine import Context, Engine
from .errors import CompensationError, ReleaseError
from .github import client_for_context
from .gitops import Git
from .journal import Journal
from .lock import ReleaseLock
from .recovery import apply_compensation, plan_compensation, rollback, yank
from .stages import build_operations
from .util import find_iscc

_VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")


def _normalise_tag(raw: str, prefix: str) -> str:
    candidate = raw.strip()
    if prefix and candidate.startswith(prefix):
        candidate = candidate[len(prefix) :]
    if not _VERSION_RE.match(candidate):
        raise ReleaseError(f"{raw!r} is not a semantic version (expected X.Y.Z[-pre])")
    return f"{prefix}{candidate}"


def _paths(repo: Path, tag: str) -> tuple[Journal, Path, Path]:
    state_dir = repo / ".release"
    return (
        Journal(state_dir / "journal" / f"{tag}.jsonl"),
        state_dir / "lock",
        state_dir,
    )


def _context(args: argparse.Namespace) -> tuple[Context, ReleaseConfig]:
    repo = Path(args.repo).resolve()
    if not repo.is_dir():
        raise ReleaseError(f"repository path does not exist: {repo}")
    cfg = load_config(
        repo,
        path=Path(args.config).resolve() if args.config else None,
        profile=args.profile,
    )
    tag_value = getattr(args, "tag", None)
    if tag_value is None:
        # `doctor` and `plan` without a tag only need a placeholder release id.
        tag_value = "0.0.0"
    tag = _normalise_tag(tag_value, cfg.tag_prefix)
    version = tag[len(cfg.tag_prefix) :] if cfg.tag_prefix else tag
    journal, lock_path, _ = _paths(repo, tag)
    git = Git(repo)
    ctx = Context(
        repo=repo,
        cfg=cfg,
        tag=tag,
        version=version,
        journal=journal,
        git=git,
        profile=cfg.profile,
        dry_run=getattr(args, "dry_run", False),
        log=print,
        base_commit=journal.base_commit() or git.head(),
    )
    return ctx, cfg


def _engine(ctx: Context) -> Engine:
    return Engine(ctx, build_operations(ctx))


def _print_plan(ctx: Context) -> None:
    print(f"release {ctx.tag}  profile={ctx.cfg.profile}  config={ctx.cfg.digest[:12]}")
    print(f"base commit: {ctx.base_commit}")
    rows = _engine(ctx).plan()
    print(f"{'action':8} {'stage':12} {'operation':22} remote  description")
    for row in rows:
        print(
            f"{row['action']:8} {row['stage']:12} {row['op']:22} "
            f"{'yes' if row['remote'] else 'no':6}  {row['description']}"
        )


def _cmd_doctor(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    operations = build_operations(ctx)
    preflight = next(op for op in operations if op.name == "preflight")
    details = preflight.run(ctx)
    print(f"repository: {ctx.repo}")
    for key, value in details.items():
        print(f"{key}: {value}")
    print(f"inno_setup: {find_iscc() or 'not found'}")
    print("doctor: ok")
    return 0


def _cmd_plan(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    _print_plan(ctx)
    return 0


def _cmd_run(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    _, lock_path, _ = _paths(ctx.repo, ctx.tag)
    operations = build_operations(ctx)
    engine = Engine(ctx, operations)
    with ReleaseLock(lock_path, break_stale=args.break_lock):
        try:
            status = engine.run()
        except ReleaseError as exc:
            if ctx.cfg.auto_compensate != "none":
                steps = plan_compensation(
                    ctx,
                    operations,
                    include_remote=ctx.cfg.auto_compensate == "all",
                )
                if steps:
                    print(
                        f"failure: {exc}\nauto-compensating {len(steps)} completed operation(s)",
                        file=sys.stderr,
                    )
                    try:
                        apply_compensation(ctx, steps, dry_run=False)
                    except CompensationError as comp:
                        print(
                            f"auto-compensation stopped: {comp}",
                            file=sys.stderr,
                        )
            raise
    print(f"release {ctx.tag}: {status}")
    if getattr(args, "dry_run", False):
        return 0
    return 0 if status in {"verified", "published"} else 1


def _cmd_status(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    record = ctx.journal.release_record()
    if not record:
        print(f"no journal for {ctx.tag} at {ctx.journal.path}")
        return 0
    print(f"release: {record.get('release')}  profile: {record.get('profile')}")
    print(f"base: {record.get('base_commit')}")
    print(f"status: {ctx.journal.status()}")
    print(f"journal: {ctx.journal.path}")
    rows = _engine(ctx).plan()
    print(f"\n{'action':8} {'status':8} {'verified':8} {'operation':22} stage")
    states = ctx.journal.state()
    for row in rows:
        state = states.get(row["op"])
        print(
            f"{row['action']:8} {state.status if state else '-':8} "
            f"{'yes' if state and state.verified else 'no':8} {row['op']:22} {row['stage']}"
        )
    error = ctx.journal.last_error()
    if error:
        print(f"\nlast error: {error}")
    return 0


def _cmd_verify(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    _, lock_path, _ = _paths(ctx.repo, ctx.tag)
    with ReleaseLock(lock_path, break_stale=args.break_lock):
        results = _engine(ctx).verify_all()
    if not results:
        print("nothing to verify; the journal has no completed operations")
        return 1
    for row in results:
        print(f"{row['op']}: {row['status']}")
    print(f"release {ctx.tag}: verified")
    return 0


def _cmd_recover(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    operations = build_operations(ctx)
    steps = plan_compensation(ctx, operations, include_remote=args.include_remote)
    if not steps:
        print("nothing to compensate")
        return 0

    print(f"compensation plan for {ctx.tag}:")
    for step in steps:
        marker = "remote" if step.operation.remote else "local"
        print(f"  - {step.operation.name} ({marker})")

    if not args.apply:
        print("\ndry run; pass --apply to execute")
        return 0

    if not args.yes:
        if not sys.stdin.isatty():
            raise ReleaseError("--apply needs --yes when stdin is not a terminal")
        answer = input("Apply these compensations? [y/N] ").strip().lower()
        if answer not in {"y", "yes"}:
            print("aborted")
            return 1

    _, lock_path, _ = _paths(ctx.repo, ctx.tag)
    with ReleaseLock(lock_path, break_stale=args.break_lock):
        apply_compensation(ctx, steps, dry_run=False)
    print(f"release {ctx.tag}: compensated")
    return 0


def _cmd_rollback(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    _, lock_path, _ = _paths(ctx.repo, ctx.tag)
    with ReleaseLock(lock_path, break_stale=args.break_lock):
        result = rollback(ctx, build_operations(ctx))
    print(f"rolled back {result['tag']} to {result['base_commit']}")
    return 0


def _cmd_yank(args: argparse.Namespace) -> int:
    ctx, _ = _context(args)
    client = None
    if ctx.cfg.publish.github:
        try:
            client = client_for_context(ctx)
        except ReleaseError as exc:
            print(f"warning: hosted yank unavailable: {exc}", file=sys.stderr)
    _, lock_path, _ = _paths(ctx.repo, ctx.tag)
    with ReleaseLock(lock_path, break_stale=args.break_lock):
        result = yank(ctx, args.reason, client=client)
    suffix = " (hosted release updated)" if result.get("hosted") else ""
    print(f"yanked {result['tag']}: {result['reason']}{suffix}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="aurelia-release",
        description="Journal-backed Aurelia release orchestrator (git and full profiles).",
    )
    parser.add_argument("--repo", default=".", help="repository root (default: .)")
    parser.add_argument("--config", help="path to release.toml")
    parser.add_argument("--profile", help="override the configured profile")
    parser.add_argument("--break-lock", action="store_true", help="clear a stale lock")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("doctor", help="check repository and environment readiness")
    p.set_defaults(func=_cmd_doctor)

    for name, func, help_text in (
        ("plan", _cmd_plan, "read-only preview of the release"),
        ("run", _cmd_run, "run or resume the release"),
        ("resume", _cmd_run, "resume a failed release from the journal"),
    ):
        p = sub.add_parser(name, help=help_text)
        p.add_argument("tag")
        if name == "run":
            p.add_argument("--dry-run", action="store_true")
        p.set_defaults(func=func)

    p = sub.add_parser("status", help="show journal state and per-operation status")
    p.add_argument("tag")
    p.set_defaults(func=_cmd_status)

    p = sub.add_parser("verify", help="re-run independent verification for recorded operations")
    p.add_argument("tag")
    p.set_defaults(func=_cmd_verify)

    p = sub.add_parser("recover", help="plan or apply compensation for an incomplete release")
    p.add_argument("tag")
    p.add_argument("--apply", action="store_true")
    p.add_argument("--include-remote", action="store_true")
    p.add_argument("--yes", action="store_true")
    p.set_defaults(func=_cmd_recover)

    p = sub.add_parser("rollback", help="undo local, unpublished release changes")
    p.add_argument("tag")
    p.set_defaults(func=_cmd_rollback)

    p = sub.add_parser("yank", help="mark a published release withdrawn")
    p.add_argument("tag")
    p.add_argument("--reason", required=True)
    p.set_defaults(func=_cmd_yank)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return int(args.func(args))
    except ReleaseError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        return 130
