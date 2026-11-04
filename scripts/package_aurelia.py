#!/usr/bin/env python3
"""Build the portable zip and per-user Inno Setup installer for a release."""
from __future__ import annotations

import argparse
import shutil
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ["README.md", "INSTALL.md", "LICENSE.md", "PRIVACY.md", "SECURITY.md", "ROADMAP.md"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--dist", required=True)
    parser.add_argument("--exe", required=True)
    parser.add_argument("--iscc", required=True)
    args = parser.parse_args()

    dist = Path(args.dist)
    staging = dist / "staging"
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir(parents=True)

    shutil.copy2(args.exe, staging / "aurelia.exe")
    for name in DOCS:
        shutil.copy2(ROOT / name, staging / name)
    shutil.copy2(ROOT / "assets" / "icon.ico", staging / "aurelia.ico")

    portable = dist / f"Aurelia-v{args.version}-win64.zip"
    with zipfile.ZipFile(portable, "w", zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(staging.iterdir()):
            archive.write(path, path.name)

    numeric = ".".join((args.version.split(".") + ["0"])[:4])
    subprocess.run(
        [
            args.iscc,
            f"/DMyAppVersion={args.version}",
            f"/DMyAppVersionNumeric={numeric}",
            f"/DMySourceDir={staging}",
            f"/DMyOutputDir={dist}",
            "aurelia.iss",
        ],
        cwd=ROOT / "installer",
        check=True,
    )
    setup = dist / f"Aurelia-Setup-{args.version}.exe"
    if not portable.is_file() or not setup.is_file():
        raise SystemExit("package outputs missing")
    print(portable)
    print(setup)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
