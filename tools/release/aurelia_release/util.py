"""Small, dependency-free helpers shared by the release orchestrator."""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence

from .errors import CommandError

_TOKEN_RE = re.compile(
    r"(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|Bearer\s+[A-Za-z0-9._~+/=-]+)"
)


def utc_now() -> str:
    """RFC 3339 UTC timestamp with a trailing Z."""
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_text(text: str) -> str:
    return sha256_bytes(text.encode("utf-8"))


def sha256_file(path: str | os.PathLike[str]) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def fingerprint(value: Any) -> str:
    return sha256_text(canonical_json(value))


def redact(text: str) -> str:
    """Best-effort removal of credentials from logs and error messages."""
    return _TOKEN_RE.sub("[REDACTED]", text or "")


def atomic_write_text(path: str | os.PathLike[str], text: str) -> None:
    """Write text so readers see either the old file or the complete new one."""
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=f".{target.name}.", dir=str(target.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp, target)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def run_command(
    command: str | Sequence[str],
    *,
    cwd: str | os.PathLike[str],
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run a command, capturing text output, with credentials redacted on failure."""
    shell = isinstance(command, str)
    merged_env = None
    if env is not None:
        merged_env = os.environ.copy()
        merged_env.update(env)

    result = subprocess.run(
        command,
        cwd=str(cwd),
        shell=shell,
        text=True,
        encoding="utf-8",
        errors="replace",
        capture_output=True,
        env=merged_env,
    )
    if check and result.returncode != 0:
        detail = redact((result.stderr or result.stdout or "").strip())
        rendered = command if isinstance(command, str) else " ".join(command)
        raise CommandError(
            f"command failed ({result.returncode}): {rendered}"
            + (f"\n{detail}" if detail else ""),
            stdout=redact(result.stdout or ""),
            stderr=redact(result.stderr or ""),
        )
    return result


def ensure_relative(path: str | os.PathLike[str]) -> str:
    """Normalise a repo-relative path for config and journal records."""
    return Path(path).as_posix().lstrip("./")


def expand_globs(root: Path, patterns: Sequence[str], values: dict[str, str]) -> list[Path]:
    """Expand simple glob patterns after substituting {tag}/{version}/{dist}."""
    import glob

    found: list[Path] = []
    for pattern in patterns:
        rendered = pattern.format(**values)
        candidate = Path(rendered)
        if not candidate.is_absolute():
            candidate = root / candidate
        for match in sorted(glob.glob(str(candidate), recursive=True)):
            path = Path(match)
            if path.is_file() and path not in found:
                found.append(path)
    return found


def find_iscc(
    env: Mapping[str, str] | None = None,
    *,
    which: Callable[[str], str | None] | None = None,
) -> Path | None:
    """Locate Inno Setup's compiler.

    Covers the environment overrides, PATH, Program Files, and the per-user
    installs, including the non-standard ``%LOCALAPPDATA%\\Programs\\Inno``
    layout used on some machines.
    """
    environment = os.environ if env is None else env
    lookup = which or shutil.which

    for variable in ("ISCC", "INNO_ISCC"):
        value = environment.get(variable)
        if value:
            candidate = Path(value)
            if candidate.is_file():
                return candidate

    for name in ("ISCC.exe", "ISCC", "iscc"):
        found = lookup(name)
        if found and Path(found).is_file():
            return Path(found)

    roots: list[Path] = []
    for variable in ("ProgramFiles", "ProgramFiles(x86)"):
        base = environment.get(variable)
        if base:
            roots.append(Path(base))
    local = environment.get("LOCALAPPDATA")
    if local:
        roots.extend([Path(local) / "Programs", Path(local)])

    relatives = (
        Path("Inno Setup 6") / "ISCC.exe",
        Path("Inno Setup 5") / "ISCC.exe",
        Path("Inno") / "ISCC.exe",
    )
    for root in roots:
        for relative in relatives:
            candidate = root / relative
            if candidate.is_file():
                return candidate
    return None
