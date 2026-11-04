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


def check_files(repo: Path, files: Iterable[str], version: str) -> list[str]:
    """Return human-readable inconsistencies; empty means every file agrees."""
    problems: list[str] = []
    for rel in files:
        path = repo / rel
        try:
            text = _read(path)
        except PreflightError as exc:
            problems.append(str(exc))
            continue
        found = declared_versions(text)
        if not found:
            # Some configured manifests (for example the workspace root) legitimately
            # carry no package version; they are normalised when they do have one.
            continue
        if found != {version}:
            problems.append(f"{rel}: declares {', '.join(sorted(found))}, expected {version}")
    return problems


def bump_files(repo: Path, files: Iterable[str], version: str) -> list[str]:
    """Normalise every declaration to `version`; return the paths written."""
    changed: list[str] = []
    for rel in files:
        path = repo / rel
        text = _read(path)
        updated, replacements = normalize_text(text, version)
        if replacements and updated != text:
            _write(path, updated)
            changed.append(Path(rel).as_posix())
    return changed
