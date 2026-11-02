#!/usr/bin/env python3
"""
Aurelia Local Release Pipeline
==============================
Staged, resumable, self-verifying release orchestrator for Aurelia.
Architecture adopted from the Morrow release pipeline; packaging adapted from
Inkwell's Inno Setup flow. VPack packaging is intentionally retired.

Stages:
  1. preflight: toolchain checks (cargo, Inno Setup), git wiring, clean-tree policy.
  2. version:   rewrite every version declaration to the tag and prove consistency.
  3. build:     cargo build --release -p aurelia-app.
  4. test:      cargo test --workspace, with pass/fail counts recorded.
  5. package:   portable zip + Inno Setup installer + release notes in dist/<tag>/.
  6. verify:    SHA256SUMS.txt, archive integrity, installer sanity, ledger drift.
  7. publish:   commit pipeline-managed changes, annotated tag, push, verify origin.
  8. ship:      attach the verified bytes to the GitHub release page (REST, no CLI).

Usage:
  python scripts/pipeline.py v2.1.0                 # full release pipeline
  python scripts/pipeline.py v2.1.0 --dry-run       # preview stages, no side effects
  python scripts/pipeline.py v2.1.0 --from package  # resume from packaging
  python scripts/pipeline.py v2.1.0 --skip test     # skip a stage
  python scripts/pipeline.py v2.1.0 --no-publish    # build/package/verify only
  python scripts/pipeline.py v2.1.0 --skip-installer
  python scripts/pipeline.py v2.1.0 --clobber       # republish a released version
  python scripts/pipeline.py v2.1.0 --force         # publish a dirty tree / move a tag

Publishing is pure git: the release *is* the annotated tag, the artifacts in
dist/<tag>/ and the record in releases/<tag>.json. GitHub is only a file host.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STAGES = ["preflight", "version", "build", "test", "package", "verify", "publish", "ship"]

PRODUCT = "Aurelia"
APP_CRATE = "aurelia-app"
EXE_NAME = "aurelia.exe"
CARGO_BIN = "aurelia-app.exe"
INSTALLER_ISS = ROOT / "installer" / "aurelia.iss"

# Files that carry the product version and must agree with the tag.
VERSION_FILES = [
    "Cargo.toml",
    "crates/aurelia-core/Cargo.toml",
    "crates/aurelia-render/Cargo.toml",
    "crates/aurelia-io/Cargo.toml",
    "crates/aurelia-ui/Cargo.toml",
    "crates/aurelia-app/Cargo.toml",
    "README.md",
    "Makefile",
    "assets/banner.svg",
    "installer/aurelia.iss",
]

# Paths the pipeline is allowed to change and then commit on its own.
MANAGED_PATHS = set(VERSION_FILES) | {"Cargo.lock", "release-notes", "releases", "dist"}


def log(msg: str, header: bool = False):
    if header:
        print(f"\n{'=' * 70}\n[PIPELINE] {msg}\n{'=' * 70}")
    else:
        print(f"[PIPELINE] {msg}")


def run(cmd, cwd=None, check=True, capture_output=False):
    """Run a command; list or string (shell) forms both supported."""
    shell = isinstance(cmd, str)
    res = subprocess.run(cmd, cwd=cwd or ROOT, shell=shell, text=True,
                         capture_output=capture_output)
    if check and res.returncode != 0:
        detail = (res.stderr or res.stdout or "") if capture_output else f"exit {res.returncode}"
        raise RuntimeError(f"Command failed ({detail.strip()}): {cmd}")
    return res


def compute_sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest().upper()


def numeric_version(version: str) -> str:
    """2.1.0 -> 2.1.0.0 (Inno Setup wants four components)."""
    clean = re.sub(r"[^0-9.].*$", "", version)
    parts = [p for p in clean.split(".") if p != ""]
    while len(parts) < 4:
        parts.append("0")
    return ".".join(parts[:4])


def iscc_path():
    """Locate Inno Setup's compiler the way Inkwell's pipeline does."""
    candidates = []
    for env in ("ISCC", "INNO_ISCC"):
        if os.environ.get(env):
            candidates.append(os.environ[env])
    for name in ("ISCC", "iscc"):
        found = shutil.which(name)
        if found:
            candidates.append(found)
    pf = os.environ.get("ProgramFiles(x86)") or os.environ.get("ProgramFiles") or r"C:\Program Files (x86)"
    local = os.environ.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
    candidates += [
        str(Path(pf) / "Inno Setup 6" / "ISCC.exe"),
        str(Path(pf) / "Inno Setup 5" / "ISCC.exe"),
        str(Path(local) / "Programs" / "Inno Setup 6" / "ISCC.exe"),
        str(Path(local) / "Programs" / "Inno" / "ISCC.exe"),
        str(Path(local) / "Inno Setup 6" / "ISCC.exe"),
    ]
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            return Path(candidate)
    return None


class ReleasePipeline:
    def __init__(self, tag: str, dry_run=False, no_publish=False, no_bump=False,
                 force=False, clobber=False, no_release_page=False,
                 skip_installer=False):
        self.tag = tag if tag.startswith("v") else f"v{tag}"
        self.ver = self.tag.lstrip("v")
        self.dry_run = dry_run
        self.no_publish = no_publish
        self.no_bump = no_bump
        self.force = force
        self.clobber = clobber
        self.no_release_page = no_release_page
        self.skip_installer = skip_installer

        self.dist_dir = ROOT / "dist" / self.tag
        self.notes_path = ROOT / "release-notes" / f"{self.tag}.md"
        self.ledger_path = ROOT / "releases" / f"{self.tag}.json"
        self.remote = None
        self.test_summary = None
        self.iscc = None

    # ── shared helpers ──────────────────────────────────────────────────
    def _state_path(self) -> Path:
        return ROOT / "target" / "pipeline-state.json"

    def _save_state(self, **facts):
        if self.dry_run:
            return
        path = self._state_path()
        path.parent.mkdir(parents=True, exist_ok=True)
        current = {}
        if path.exists():
            try:
                current = json.loads(path.read_text(encoding="utf-8"))
            except ValueError:
                current = {}
        current.setdefault(self.tag, {}).update(facts)
        path.write_text(json.dumps(current, indent=2) + "\n", encoding="utf-8")

    def _load_state(self):
        path = self._state_path()
        if not path.exists():
            return {}
        try:
            return json.loads(path.read_text(encoding="utf-8")).get(self.tag, {})
        except ValueError:
            return {}

    def _recover_facts(self):
        state = self._load_state()
        self.test_summary = self.test_summary or state.get("tests")
        return state

    def _load_ledger(self):
        if not self.ledger_path.exists():
            return {}
        try:
            return json.loads(self.ledger_path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return {}

    def _asset_digests(self):
        return [
            {"name": a.name, "bytes": a.stat().st_size, "sha256": compute_sha256(a)}
            for a in sorted(self.dist_dir.glob("*"))
            if a.is_file() and a.name not in ("SHA256SUMS.txt", "PUBLISH.txt")
        ]

    def _read_preserving(self, path: Path) -> str:
        return path.read_text(encoding="utf-8")

    def _write_preserving(self, path: Path, text: str):
        path.write_text(text, encoding="utf-8", newline="")

    # ── stage 1: preflight ──────────────────────────────────────────────
    def stage_preflight(self):
        log("STAGE 1: Preflight checks", header=True)
        rustc = run("rustc --version", capture_output=True).stdout.strip()
        cargo = run("cargo --version", capture_output=True).stdout.strip()
        log(f"Toolchain: {rustc} / {cargo}")

        self.iscc = iscc_path()
        if self.skip_installer:
            log("Installer build skipped (--skip-installer): portable zip only.")
        elif not self.iscc:
            raise RuntimeError(
                "Inno Setup (ISCC.exe) not found. Install it with "
                "'winget install JRSoftware.InnoSetup', or run with --skip-installer "
                "for a portable-only package."
            )
        else:
            log(f"Inno Setup: {self.iscc}")
        if not self.skip_installer and not INSTALLER_ISS.exists():
            raise FileNotFoundError(f"missing installer script {INSTALLER_ISS}")

        res = run("git rev-parse --is-inside-work-tree", capture_output=True, check=False)
        if res.returncode != 0:
            raise RuntimeError("Not inside a git work tree; cannot release.")
        branch = run("git branch --show-current", capture_output=True).stdout.strip()
        if not branch:
            raise RuntimeError("HEAD is detached; check out the release branch first.")
        log(f"Branch: {branch}")

        res = run("git remote get-url origin", capture_output=True, check=False)
        if res.returncode != 0:
            raise RuntimeError("No 'origin' remote configured; add one or run with --no-publish.")
        self.remote = res.stdout.strip()
        log(f"Remote: origin = {self.remote}")

        dirty = run("git status --porcelain", capture_output=True).stdout.strip()
        if dirty:
            log(f"Warning: working tree has modifications:\n{dirty}")
            if not (self.force or self.no_publish or self.clobber):
                log("The publish stage will refuse unmanaged changes unless --force is given.")

        log("Preflight passed.")

    # ── stage 2: version ────────────────────────────────────────────────
    def _current_version(self) -> str:
        text = (ROOT / "crates" / APP_CRATE / "Cargo.toml").read_text(encoding="utf-8")
        match = re.search(r'^version = "([^"]+)"', text, re.MULTILINE)
        if not match:
            raise RuntimeError("cannot read the current version from the app manifest")
        return match.group(1)

    def _bump_declarations(self):
        current = self._current_version()
        if current == self.ver:
            return []
        changed = []
        pattern = re.compile(re.escape(current))
        for rel in VERSION_FILES:
            path = ROOT / rel
            if not path.exists():
                continue
            text = self._read_preserving(path)
            updated = pattern.sub(self.ver, text)
            if updated != text:
                self._write_preserving(path, updated)
                changed.append(rel)
        return changed

    def _read_declared_versions(self):
        """Collect every version string the pipeline cares about, per file."""
        declared = {}
        for rel in VERSION_FILES:
            path = ROOT / rel
            if not path.exists():
                continue
            text = path.read_text(encoding="utf-8")
            versions = set()
            if rel.endswith("Cargo.toml"):
                versions |= set(re.findall(r'version = "(\d+\.\d+\.\d+)"', text))
            if rel == "README.md":
                versions |= set(re.findall(r"Release-v(\d+\.\d+\.\d+)", text))
                versions |= set(re.findall(r"Aurelia-v(\d+\.\d+\.\d+)-win64", text))
            if rel == "Makefile":
                versions |= set(re.findall(r"TAG \?= v(\d+\.\d+\.\d+)", text))
            if rel == "assets/banner.svg":
                versions |= set(re.findall(r"v(\d+\.\d+\.\d+) · PRODUCTION", text))
            if rel == "installer/aurelia.iss":
                versions |= set(re.findall(r'MyAppVersion "(\d+\.\d+\.\d+)"', text))
            if versions:
                declared[rel] = versions
        return declared

    def stage_version(self):
        log("STAGE 2: Version bump & declaration consistency", header=True)
        if self.dry_run:
            log(f"[DRY-RUN] Would rewrite version declarations to {self.ver}.")
            return
        if self.no_bump:
            log("--no-bump requested; declarations are left untouched.")
        else:
            changed = self._bump_declarations()
            for item in changed:
                log(f"  bumped {item}")
            if not changed:
                log(f"Declarations already declare {self.ver}.")

        declared = self._read_declared_versions()
        stale = {rel: sorted(v - {self.ver}) for rel, v in declared.items() if v - {self.ver}}
        if stale:
            detail = "; ".join(f"{rel}: {', '.join(vs)}" for rel, vs in stale.items())
            raise RuntimeError(
                f"Version declarations do not match {self.tag} ({detail}). "
                f"Re-run without --no-bump or fix them by hand."
            )
        missing = [rel for rel in VERSION_FILES if not (ROOT / rel).exists()]
        if missing:
            raise RuntimeError(f"Missing version declaration files: {', '.join(missing)}")
        log(f"All {len(declared)} declaration files agree on {self.ver}.")

    # ── stage 3: build ──────────────────────────────────────────────────
    def stage_build(self):
        log("STAGE 3: Release build", header=True)
        if self.dry_run:
            log(f"[DRY-RUN] Would run cargo build --release -p {APP_CRATE}.")
            return
        run(["cargo", "build", "--release", "-p", APP_CRATE])
        binary = ROOT / "target" / "release" / CARGO_BIN
        if not binary.exists():
            raise FileNotFoundError(f"expected {binary} after the build")
        size_mb = binary.stat().st_size / (1024 * 1024)
        log(f"Built {binary.relative_to(ROOT)} ({size_mb:.1f} MB)")
        self._save_state(binary_sha256=compute_sha256(binary),
                         binary_bytes=binary.stat().st_size)

    # ── stage 4: test ───────────────────────────────────────────────────
    def stage_test(self):
        log("STAGE 4: Workspace test suite", header=True)
        if self.dry_run:
            log("[DRY-RUN] Would run cargo test --workspace.")
            return
        res = run(["cargo", "test", "--workspace"], capture_output=True, check=False)
        output = (res.stdout or "") + (res.stderr or "")
        passed = sum(int(m) for m in re.findall(r"test result: ok\. (\d+) passed", output))
        failed = sum(int(m) for m in re.findall(r"(\d+) failed", output))
        for line in output.splitlines():
            if "test result:" in line or line.startswith("error") or "panicked" in line:
                log(f"  {line.strip()}")
        if res.returncode != 0 or failed:
            raise RuntimeError(f"tests failed ({failed} failing, {passed} passing)")
        self.test_summary = f"{passed} tests passed across the workspace (0 failed)"
        log(self.test_summary)
        self._save_state(tests=self.test_summary)

    # ── stage 5: package ────────────────────────────────────────────────
    ZIP_DATE = (1980, 1, 1, 0, 0, 0)
    SHIPPED_DOCS = ("README.md", "INSTALL.md", "LICENSE.md", "PRIVACY.md",
                    "SECURITY.md", "ROADMAP.md")

    @classmethod
    def _zip_add(cls, zf, path: Path, arcname: str):
        info = zipfile.ZipInfo(arcname.replace("\\", "/"), date_time=cls.ZIP_DATE)
        info.compress_type = zipfile.ZIP_DEFLATED
        info.create_system = 0          # MS-DOS: host-independent archives
        info.external_attr = 0o644 << 16
        zf.writestr(info, path.read_bytes())

    def _generated_notes(self) -> str:
        tests = self.test_summary or "(test stage skipped in this run)"
        assets = "`Aurelia-{0}-win64.zip`, `Aurelia-Setup-{1}.exe`, `SHA256SUMS.txt`".format(
            self.tag, self.ver)
        return f"""# Aurelia {self.tag}

Built and verified locally by `scripts/pipeline.py` (see `RELEASE_PROCESS.md`).

## Verification recorded by the pipeline

| Gate | Result |
|---|---|
| Workspace tests | {tests} |
| Distribution | {assets} |

> This skeleton was generated because `release-notes/{self.tag}.md` is missing.
> Commit authored notes for that tag to replace it.
"""

    def stage_package(self):
        log("STAGE 5: Packaging release assets", header=True)
        if self.dry_run:
            log(f"[DRY-RUN] Would package {PRODUCT} {self.tag} into {self.dist_dir}")
            return

        if self.dist_dir.exists() and (self.dist_dir / "PUBLISH.txt").exists():
            if not self.clobber:
                raise RuntimeError(
                    f"{self.dist_dir} already holds a published release record (PUBLISH.txt). "
                    f"Pass --clobber to rebuild it, or cut a new version."
                )
            log(f"--clobber: overwriting the published artifacts in {self.dist_dir.name}/")
        if self.dist_dir.exists():
            shutil.rmtree(self.dist_dir)
        self.dist_dir.mkdir(parents=True, exist_ok=True)
        self._recover_facts()

        # 1. Staging folder: the exact tree a portable user unzips.
        staging = self.dist_dir / "staging"
        staging.mkdir()
        binary = ROOT / "target" / "release" / CARGO_BIN
        if not binary.exists():
            raise FileNotFoundError(f"missing {binary}; run the build stage first")
        shutil.copy2(binary, staging / EXE_NAME)
        for doc in self.SHIPPED_DOCS:
            source = ROOT / doc
            if source.exists():
                shutil.copy2(source, staging / doc)
            else:
                log(f"  note: {doc} not present, omitted from the package")

        # 2. Portable archive.
        zip_path = self.dist_dir / f"{PRODUCT}-{self.tag}-win64.zip"
        with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
            for path in sorted(staging.iterdir()):
                if path.is_file():
                    self._zip_add(zf, path, f"{PRODUCT}/{path.name}")
        log(f"Created {zip_path.name} ({zip_path.stat().st_size / (1024 * 1024):.2f} MB)")

        # 3. Per-user Inno Setup installer (adapted from Inkwell's installer flow).
        if self.skip_installer:
            log("Installer skipped (--skip-installer); portable zip only.")
        else:
            if not self.iscc:
                raise RuntimeError("Inno Setup was not resolved in preflight")
            cmd = [
                str(self.iscc),
                f"/DMyAppVersion={self.ver}",
                f"/DMyAppVersionNumeric={numeric_version(self.ver)}",
                f"/DMySourceDir={staging}",
                f"/DMyOutputDir={self.dist_dir}",
                str(INSTALLER_ISS),
            ]
            res = run(cmd, capture_output=True, check=False)
            for line in ((res.stdout or "") + (res.stderr or "")).splitlines():
                if line.strip() and ("Successful" in line or "Error" in line or "Warning" in line):
                    log(f"  ISCC: {line.strip()}")
            setup = self.dist_dir / f"{PRODUCT}-Setup-{self.ver}.exe"
            if res.returncode != 0 or not setup.exists():
                raise RuntimeError(
                    f"Inno Setup failed (exit {res.returncode}); expected {setup.name}"
                )
            log(f"Created {setup.name} ({setup.stat().st_size / (1024 * 1024):.2f} MB)")

        # 4. Release notes: authored notes win, otherwise a factual skeleton.
        notes_file = self.dist_dir / "RELEASE_NOTES.md"
        if self.notes_path.exists():
            shutil.copy2(self.notes_path, notes_file)
            log(f"Release notes taken from {self.notes_path.relative_to(ROOT)}")
        else:
            notes_file.write_text(self._generated_notes(), encoding="utf-8")
            log(f"No release-notes/{self.tag}.md found; wrote a generated skeleton.")

        shutil.rmtree(staging, ignore_errors=True)

    # ── stage 6: verify ─────────────────────────────────────────────────
    def stage_verify(self):
        log("STAGE 6: Verification & checksum generation", header=True)
        if self.dry_run:
            log("[DRY-RUN] Would verify and hash the release assets.")
            return

        zip_name = f"{PRODUCT}-{self.tag}-win64.zip"
        setup_name = f"{PRODUCT}-Setup-{self.ver}.exe"
        required = [zip_name, "RELEASE_NOTES.md"]
        if not self.skip_installer:
            required.append(setup_name)
        missing = [name for name in required if not (self.dist_dir / name).exists()]
        if missing:
            raise FileNotFoundError(
                f"Missing release assets in {self.dist_dir}: {', '.join(missing)}"
            )

        # Archive integrity + the files a user must receive.
        with zipfile.ZipFile(self.dist_dir / zip_name, "r") as zf:
            bad = zf.testzip()
            if bad:
                raise RuntimeError(f"Corrupted member in {zip_name}: {bad}")
            names = set(zf.namelist())
            for needed in (f"{PRODUCT}/{EXE_NAME}", f"{PRODUCT}/LICENSE.md",
                           f"{PRODUCT}/README.md", f"{PRODUCT}/INSTALL.md"):
                if needed not in names:
                    raise RuntimeError(f"{zip_name} is missing {needed}")
        log(f"{zip_name}: archive intact, contains {EXE_NAME} + licensing")

        # Installer sanity: a PE image, not a stub.
        if not self.skip_installer:
            setup = self.dist_dir / setup_name
            if setup.read_bytes()[:2] != b"MZ":
                raise RuntimeError(f"{setup_name} is not a Windows executable")
            if setup.stat().st_size < 512 * 1024:
                raise RuntimeError(f"{setup_name} looks too small to be an installer")
            log(f"{setup_name}: valid PE image ({setup.stat().st_size / (1024 * 1024):.2f} MB)")

        # Checksums for everything that ships.
        assets = [p for p in sorted(self.dist_dir.glob("*"))
                  if p.is_file() and p.name not in ("SHA256SUMS.txt", "PUBLISH.txt")]
        lines = []
        for asset in assets:
            digest = compute_sha256(asset)
            lines.append(f"{digest}  {asset.name}")
            log(f"  {asset.name} -> {digest}")
        (self.dist_dir / "SHA256SUMS.txt").write_text("\n".join(lines) + "\n",
                                                      encoding="utf-8")
        log("Wrote SHA256SUMS.txt")

        # Cross-check against the local release record (republish byte equality).
        ledger = self._load_ledger()
        if ledger:
            recorded = {a["name"]: a["sha256"] for a in ledger.get("assets", [])}
            current = {a["name"]: a["sha256"] for a in self._asset_digests()}
            drift = sorted(n for n in current if n in recorded and recorded[n] != current[n])
            added = sorted(set(current) - set(recorded))
            removed = sorted(set(recorded) - set(current))
            if drift:
                if not self.clobber:
                    raise RuntimeError(
                        "Artifacts differ from the recorded publication of "
                        f"{self.tag} ({', '.join(drift)}). Pass --clobber to record the new "
                        "bytes, or investigate why the build changed."
                    )
                log("Warning: --clobber recording new bytes for: " + ", ".join(drift))
            if added or removed:
                log(f"Asset set changed vs {self.ledger_path.name}: "
                    f"+{added or '[]'} -{removed or '[]'}")
            if not drift and not added and not removed:
                log(f"Artifacts match the recorded publication of {self.tag} byte-for-byte.")

        self._save_state(assets=self._asset_digests())

    # ── stage 7: publish ────────────────────────────────────────────────
    def _split_dirty(self, lines):
        managed, unmanaged = [], []
        for line in lines:
            path = line[3:].strip().strip('"')
            if " -> " in path:
                path = path.split(" -> ", 1)[1].strip().strip('"')
            if any(path == p or path.startswith(p.rstrip("/") + "/") for p in MANAGED_PATHS):
                managed.append(path)
            else:
                unmanaged.append(line)
        return managed, unmanaged

    def _write_ledger(self, previous):
        commit = run("git rev-parse HEAD", capture_output=True).stdout.strip()
        entry = {
            "tag": self.tag,
            "version": self.ver,
            "product": PRODUCT,
            "remote": self.remote,
            "tests": self.test_summary,
            "notes": (f"release-notes/{self.tag}.md" if self.notes_path.exists() else None),
            "assets": self._asset_digests(),
            "installer": (None if self.skip_installer
                          else f"{PRODUCT}-Setup-{self.ver}.exe"),
            "published_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "republish_count": int(previous.get("republish_count", 0)) + (1 if previous else 0),
            "recorded_near_commit": commit,
            "previous_publication": (
                {"published_at": previous.get("published_at"),
                 "assets": previous.get("assets")}
                if previous else None
            ),
        }
        self.ledger_path.parent.mkdir(parents=True, exist_ok=True)
        if previous:
            same = (previous.get("tests") == entry["tests"]
                    and previous.get("assets") == entry["assets"]
                    and previous.get("installer") == entry["installer"])
            if same:
                log(f"{self.ledger_path.name} already records this publication exactly; "
                    f"nothing to re-record.")
                return False
        self.ledger_path.write_text(json.dumps(entry, indent=2) + "\n", encoding="utf-8")
        log(f"Wrote release record {self.ledger_path.relative_to(ROOT)} "
            f"(republish #{entry['republish_count']})")
        return True

    def _release_title(self) -> str:
        return f"{PRODUCT} {self.tag}"

    def stage_publish(self):
        log("STAGE 8: Publish (git tag + push)", header=True)
        if self.dry_run:
            log("[DRY-RUN] Would commit, tag and push the release.")
            return
        if self.no_publish:
            log("--no-publish requested; skipping git tag/push.")
            return

        self._recover_facts()
        if not self.remote:
            # A run resumed at `publish` skips preflight, so resolve origin here.
            self.remote = run("git remote get-url origin", capture_output=True,
                              check=False).stdout.strip() or None
        head = run("git rev-parse HEAD", capture_output=True).stdout.strip()
        tag_exists = run(f"git rev-parse -q --verify refs/tags/{self.tag}",
                         check=False, capture_output=True).returncode == 0
        if tag_exists:
            tag_sha = run(f"git rev-list -n 1 {self.tag}", capture_output=True).stdout.strip()
            at_head = tag_sha == head
            ancestor = at_head or run(f"git merge-base --is-ancestor {self.tag} HEAD",
                                      check=False, capture_output=True).returncode == 0
            if not ancestor and not self.force:
                raise RuntimeError(
                    f"Tag {self.tag} points at {tag_sha[:8]}, which is not an ancestor of "
                    f"HEAD ({head[:8]}). Nothing was committed. Check out the tagged commit, "
                    f"cut a new version, or pass --force to move the tag."
                )
            if not at_head and not self.clobber:
                raise RuntimeError(
                    f"Tag {self.tag} points at {tag_sha[:8]}, behind HEAD ({head[:8]}). "
                    f"That is a republication; pass --clobber to record it."
                )

        ledger = self._load_ledger()
        if ledger and tag_exists and not self.clobber:
            raise RuntimeError(
                f"{self.tag} is already published (local record {self.ledger_path.name}, "
                f"publication #{ledger.get('republish_count', 0)} at "
                f"{ledger.get('published_at')}). Pass --clobber to rebuild and record another "
                f"publication."
            )
        self._write_ledger(ledger)

        # Commit only what the pipeline owns (version bumps, notes, ledger).
        dirty_lines = [l for l in run("git status --porcelain", capture_output=True)
                       .stdout.splitlines() if l.strip()]
        if dirty_lines:
            managed, unmanaged = self._split_dirty(dirty_lines)
            if unmanaged and not self.force:
                raise RuntimeError(
                    "Uncommitted changes the pipeline does not own; commit them first or "
                    "pass --force:\n" + "\n".join(unmanaged)
                )
            if self.force and unmanaged:
                run("git add -A")
                count = len(managed) + len(unmanaged)
            else:
                for path in managed:
                    run(["git", "add", "--", path])
                count = len(managed)
            if count:
                suffix = " (republish)" if self.clobber else ""
                run(["git", "commit", "-m", f"chore(release): {self.tag}{suffix}",
                     "-m", "Version declarations, release notes and the local release record "
                           "written by scripts/pipeline.py."])
                log(f"Committed {count} pipeline-managed change(s).")
        else:
            log("Working tree clean.")

        # Tag HEAD (create, or move when --force).
        head = run("git rev-parse HEAD", capture_output=True).stdout.strip()
        if run(f"git rev-parse -q --verify refs/tags/{self.tag}", check=False,
               capture_output=True).returncode != 0:
            run(["git", "tag", "-a", self.tag, "-m", self._release_title()])
            log(f"Created annotated tag {self.tag} at {head[:8]}")
        else:
            tag_sha = run(f"git rev-list -n 1 {self.tag}", capture_output=True).stdout.strip()
            if tag_sha != head:
                if self.force:
                    run(["git", "tag", "-f", "-a", self.tag, "-m", self._release_title()])
                    log(f"Moved tag {self.tag} to {head[:8]}")
                else:
                    log(f"Tag {self.tag} stays at {tag_sha[:8]} (release commit); "
                        f"the new publication record is committed on top of it.")

        # Push branch + tag, then prove origin agrees with the local objects.
        branch = run("git branch --show-current", capture_output=True).stdout.strip()
        run(["git", "push", "origin", branch])
        run(["git", "push", "--force", "origin", f"refs/tags/{self.tag}"],
            check=False, capture_output=True)
        tag_obj = run(f"git rev-parse refs/tags/{self.tag}",
                      capture_output=True).stdout.strip()
        ls = run(["git", "ls-remote", "origin", f"refs/tags/{self.tag}",
                  f"refs/tags/{self.tag}^{{}}"], capture_output=True, check=False)
        remote_obj = ""
        for line in (ls.stdout or "").splitlines():
            parts = line.split()
            if len(parts) == 2 and parts[1] == f"refs/tags/{self.tag}":
                remote_obj = parts[0]
        if remote_obj != tag_obj:
            raise RuntimeError(
                f"Remote tag {self.tag} ({remote_obj[:8]}) does not match local "
                f"({tag_obj[:8]}) after push."
            )
        log(f"Remote tag {self.tag} verified (object {tag_obj[:8]})")

        # Publish manifest next to the artifacts.
        if not (self.dist_dir / "RELEASE_NOTES.md").exists():
            raise FileNotFoundError(
                f"{self.dist_dir / 'RELEASE_NOTES.md'} is missing; run the package stage first"
            )
        manifest = self.dist_dir / "PUBLISH.txt"
        lines = [
            self._release_title(),
            "",
            f"commit:  {head}",
            f"branch:  {branch}",
            f"remote:  {self.remote}",
            f"tests:   {self.test_summary or 'n/a'}",
            "",
            "assets (SHA-256 in SHA256SUMS.txt):",
        ]
        for asset in sorted(self.dist_dir.glob("*")):
            if asset.is_file() and asset.name not in ("SHA256SUMS.txt", "PUBLISH.txt"):
                lines.append(f"  {asset.name}  ({asset.stat().st_size} bytes)")
        lines += ["", "Installer: Inno Setup per-user package (no admin rights).",
                  "Publishing = git tag + push; GitHub stores the bytes only.", ""]
        manifest.write_text("\n".join(lines), encoding="utf-8")
        log(f"Wrote {manifest.relative_to(ROOT)}")
        log(f"{self.tag} published: tag pushed and verified on origin.")


    # ── stage 9: ship ───────────────────────────────────────────────────
    def stage_ship(self):
        log("STAGE 9: Ship artifacts to the release page", header=True)
        if self.dry_run:
            log(f"[DRY-RUN] Would upload dist/{self.tag}/* to the release page.")
            return
        if self.no_publish:
            log("--no-publish requested; skipping the release page.")
            return
        if self.no_release_page:
            log("--no-release-page requested; artifacts stay local in dist/.")
            return
        script = ROOT / "scripts" / "release_page.py"
        if not script.exists():
            raise FileNotFoundError(f"missing {script}")
        # GitHub is a file host here, never a builder: these bytes were produced and
        # verified on this machine. No Actions workflow, no CI runner, no forge CLI.
        res = run([sys.executable, str(script), self.tag], check=False, capture_output=True)
        for line in ((res.stdout or "") + (res.stderr or "")).splitlines():
            if line.strip():
                log(line.strip())
        if res.returncode != 0:
            raise RuntimeError(
                f"release page upload failed (exit {res.returncode}); the local release is "
                f"complete, rerun with --from ship to retry"
            )


def main():
    parser = argparse.ArgumentParser(description="Aurelia local release pipeline.")
    parser.add_argument("tag", help="Release tag (e.g. v2.1.0)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Preview stages without running commands")
    parser.add_argument("--from", dest="from_stage", choices=STAGES, default="preflight",
                        help="Resume the pipeline from this stage")
    parser.add_argument("--skip", action="append", choices=STAGES, default=[],
                        help="Skip a stage (repeatable)")
    parser.add_argument("--no-publish", action="store_true",
                        help="Run everything except git tag/push and the release page")
    parser.add_argument("--no-bump", action="store_true",
                        help="Do not rewrite version declarations")
    parser.add_argument("--force", action="store_true",
                        help="Publish a dirty tree or move an existing tag")
    parser.add_argument("--clobber", action="store_true",
                        help="Republish an already-released version")
    parser.add_argument("--no-release-page", action="store_true",
                        help="Keep artifacts local; do not attach them to GitHub")
    parser.add_argument("--skip-installer", action="store_true",
                        help="Portable zip only (no Inno Setup package)")
    args = parser.parse_args()

    pipeline = ReleasePipeline(
        args.tag, dry_run=args.dry_run, no_publish=args.no_publish,
        no_bump=args.no_bump, force=args.force, clobber=args.clobber,
        no_release_page=args.no_release_page, skip_installer=args.skip_installer,
    )
    start = STAGES.index(args.from_stage)
    handlers = {
        "preflight": pipeline.stage_preflight,
        "version": pipeline.stage_version,
        "build": pipeline.stage_build,
        "test": pipeline.stage_test,
        "package": pipeline.stage_package,
        "verify": pipeline.stage_verify,
        "publish": pipeline.stage_publish,
        "ship": pipeline.stage_ship,
    }
    try:
        for stage in STAGES[start:]:
            if stage in args.skip:
                log(f"Skipping stage '{stage}' (--skip).")
                continue
            handlers[stage]()
    except Exception as exc:  # clean failure, not a traceback dump
        print(f"\n[PIPELINE] FAILED: {exc}", file=sys.stderr)
        return 1
    print(f"\n[PIPELINE] {pipeline.tag} complete.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
