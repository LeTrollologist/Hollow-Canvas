"""Version declaration discovery and rewriting for the Aurelia workspace."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Iterable

from .errors import PreflightError

SEMVER = r"\d+\.\d+\.\d+"

# Rewrite only declarations that actually carry the product version. Applying all
# rules to every configured file is intentional and idempotent: a pattern that
# does not occur in a file simply does nothing.
DECLARATION_RULES: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r'(?m)^version = "\d+\.\d+\.\d+"'), 'version = "{v}"'),
    (
        re.compile(
            r'([A-Za-z0-9_-]+\s*=\s*\{\s*path\s*=\s*"[^"]*"\s*,\s*version\s*=\s*")'
            r"\d+\.\d+\.\d+(\")"
        ),
        r"\g<1>{v}\g<2>",
    ),
    (re.compile(r"(Release-v)\d+\.\d+\.\d+"), r"\g<1>{v}"),
    (re.compile(r"(releases/tag/v)\d+\.\d+\.\d+"), r"\g<1>{v}"),
    (re.compile(r"(Aurelia-v)\d+\.\d+\.\d+(-win64)"), r"\g<1>{v}\g<2>"),
    (re.compile(r"(Aurelia-Setup-)\d+\.\d+\.\d+"), r"\g<1>{v}"),
    (re.compile(r"(TAG \?= v)\d+\.\d+\.\d+"), r"\g<1>{v}"),
    (re.compile(r"(v)\d+\.\d+\.\d+( · PRODUCTION)"), r"\g<1>{v}\g<2>"),
    (re.compile(r'(MyAppVersionNumeric ")\d+\.\d+\.\d+\.\d+(")'), r"\g<1>{v}.0\g<2>"),
    (re.compile(r'(MyAppVersion ")\d+\.\d+\.\d+(")'), r"\g<1>{v}\g<2>"),
]

DECLARATION_PATTERNS: list[re.Pattern[str]] = [
    re.compile(r'(?m)^version = "(\d+\.\d+\.\d+)"'),
    re.compile(
        r'path\s*=\s*"[^"]*"\s*,\s*version\s*=\s*"(\d+\.\d+\.\d+)"'
    ),
    re.compile(r"Release-v(\d+\.\d+\.\d+)"),
    re.compile(r"releases/tag/v(\d+\.\d+\.\d+)"),
    re.compile(r"Aurelia-v(\d+\.\d+\.\d+)-win64"),
    re.compile(r"Aurelia-Setup-(\d+\.\d+\.\d+)"),
    re.compile(r"TAG \?= v(\d+\.\d+\.\d+)"),
    re.compile(r"v(\d+\.\d+\.\d+) · PRODUCTION"),
    re.compile(r'MyAppVersion "(\d+\.\d+\.\d+)"'),
    re.compile(r'MyAppVersionNumeric "(\d+\.\d+\.\d+)\.\d+"'),
]


def declared_versions(text: str) -> set[str]:
    versions: set[str] = set()
    for pattern in DECLARATION_PATTERNS:
        versions.update(pattern.findall(text))
    return versions


def _cargo_lock_versions(text: str, package_names: set[str]) -> set[str]:
    """Versions of workspace packages only, never dependency versions."""
    versions: set[str] = set()
    for block in text.split("[[package]]")[1:]:
        name = re.search(r'(?m)^name = "([^"]+)"', block)
        version = re.search(r'(?m)^version = "(\d+\.\d+\.\d+)"', block)
        if name and version and name.group(1) in package_names:
            versions.add(version.group(1))
    return versions


def _rewrite_cargo_lock(text: str, version: str, package_names: set[str]) -> tuple[str, int]:
    """Rewrite only workspace package blocks in Cargo.lock.

    A blanket ``version =`` rewrite would change every dependency in the lockfile.
    """
    parts = text.split("[[package]]")
    changed = 0
    out = [parts[0]]
    for block in parts[1:]:
        name = re.search(r'(?m)^name = "([^"]+)"', block)
        if name and name.group(1) in package_names:
            block, count = re.subn(
                r'(?m)^version = "\d+\.\d+\.\d+"',
                f'version = "{version}"',
                block,
                count=1,
            )
            changed += count
        out.append(block)
    return "[[package]]".join(out), changed


def normalize_text(text: str, version: str) -> tuple[str, int]:
    """Return the text with all declarations at `version`, plus replacements made."""
    changed = 0
    result = text
    for pattern, template in DECLARATION_RULES:
        result, count = pattern.subn(template.format(v=version), result)
        changed += count
    return result, changed


def _read(path: Path) -> str:
    if not path.is_file():
        raise PreflightError(f"version declaration file is missing: {path}")
    with open(path, "r", encoding="utf-8", newline="") as handle:
        return handle.read()


def _write(path: Path, text: str) -> None:
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def check_files(
    repo: Path,
    files: Iterable[str],
    version: str,
    cargo_lock_packages: set[str] | None = None,
) -> list[str]:
    """Return human-readable inconsistencies; empty means every file agrees."""
    problems: list[str] = []
    for rel in files:
        path = repo / rel
        try:
            text = _read(path)
        except PreflightError as exc:
            problems.append(str(exc))
            continue
        if rel == "Cargo.lock" and cargo_lock_packages is not None:
            found = _cargo_lock_versions(text, cargo_lock_packages)
        else:
            found = declared_versions(text)
        if not found:
            # Some configured manifests (for example the workspace root) legitimately
            # carry no package version; they are normalised when they do have one.
            continue
        if found != {version}:
            problems.append(f"{rel}: declares {', '.join(sorted(found))}, expected {version}")
    return problems


def bump_files(
    repo: Path,
    files: Iterable[str],
    version: str,
    cargo_lock_packages: set[str] | None = None,
) -> list[str]:
    """Normalise every declaration to `version`; return the paths written."""
    changed: list[str] = []
    for rel in files:
        path = repo / rel
        text = _read(path)
        if rel == "Cargo.lock" and cargo_lock_packages is not None:
            updated, replacements = _rewrite_cargo_lock(
                text, version, cargo_lock_packages
            )
        else:
            updated, replacements = normalize_text(text, version)
        if replacements and updated != text:
            _write(path, updated)
            changed.append(Path(rel).as_posix())
    return changed
