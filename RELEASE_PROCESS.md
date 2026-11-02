# Aurelia: Local Release & Packaging Process

Aurelia releases are cut on a developer machine, never in CI. The pipeline is
`scripts/pipeline.py`, whose stage model is adopted from the Morrow release
pipeline; the Windows installer stage is adapted from Inkwell's Inno Setup flow.
VPack packaging and ed25519 signing are retired for now — releases ship a portable
zip, an Inno Setup installer and a SHA-256 manifest.

---

## 1. Principles

1. **No hosted CI.** Nothing builds on GitHub. A release is reproducible from a
   checkout plus `python scripts/pipeline.py <tag>`.
2. **Staged and resumable.** Every stage can be re-entered with `--from` or skipped
   with `--skip`; facts (test counts, binary hash, asset digests) are parked in
   `target/pipeline-state.json` so a resumed run reports them correctly.
3. **The release *is* the git tag.** `publish` commits only pipeline-owned files,
   creates an annotated tag, pushes, and then proves the tag object on `origin`
   matches the local one. GitHub is a file host, never a builder.
4. **Verified before shipped.** `verify` hashes every asset, tests the archive,
   sanity-checks the installer PE image and refuses to upload bytes that disagree
   with `SHA256SUMS.txt`.
5. **Recorded locally.** Each publication writes `releases/<tag>.json` (assets,
   hashes, test summary, republish count), so a `--clobber` rebuild can prove it
   reproduced the published bytes.

---

## 2. Tooling

| Tool | Needed for | Notes |
| :--- | :--- | :--- |
| Rust (`cargo`, `rustc`) | build, test | 1.80+ recommended |
| **Inno Setup 6** (`ISCC.exe`) | installer | `winget install JRSoftware.InnoSetup`; skip with `--skip-installer` |
| git + `origin` | publish | the HTTPS credential is reused for uploads |
| **VirusTotal API key** | virustotal | environment variable `VIRUSTOTAL_API_KEY` (never a file); skip with `--skip virustotal` |
| Python 3.10+ | the pipeline itself | no third-party packages |

`preflight` locates `ISCC.exe` in `%ProgramFiles(x86)%`, `%ProgramFiles%`,
`%LOCALAPPDATA%\Programs\Inno*`, `PATH` or `%ISCC%`, and fails with the install hint
above if it cannot find it.

---

## 3. Stages

| Stage | Operation | Detail |
| :--- | :--- | :--- |
| **`preflight`** | Environment | Toolchain versions, Inno Setup discovery, git branch/remote, dirty-tree policy |
| **`version`** | Version bump | Rewrites the tag into every declaration (5 crates, README, Makefile, banner, installer) then *proves* no stale version survives |
| **`build`** | Compile | `cargo build --release -p aurelia-app`; records the binary hash |
| **`test`** | Verify | `cargo test --workspace`; records `N tests passed` |
| **`package`** | Bundle | Portable zip + `Aurelia-Setup-<ver>.exe` + release notes in `dist/<tag>/` |
| **`virustotal`** | Scan | Submits the zip **and** the installer to the VirusTotal API v3, writes the verdict into the release notes and `virustotal-summary.txt`, and aborts on detections unless `--force` |
| **`verify`** | Integrity | `SHA256SUMS.txt`, zip member/licensing checks, installer `MZ`/size check, ledger drift detection |
| **`publish`** | Git | Commits pipeline-owned changes only, annotated tag, push branch + tag, verifies `origin` |
| **`ship`** | Upload | `scripts/release_page.py` attaches the verified bytes to the GitHub release page over the REST API |

---

## 4. Cutting a release

```bash
# Full release (bump, build, test, package, verify, publish, ship)
make release TAG=v2.1.0

# Equivalent
python scripts/pipeline.py v2.1.0
```

Common variants:

```bash
python scripts/pipeline.py v2.1.0 --dry-run        # print the plan, touch nothing
python scripts/pipeline.py v2.1.0 --no-publish     # build/package/verify only
python scripts/pipeline.py v2.1.0 --skip-installer # portable zip only (no Inno Setup)
python scripts/pipeline.py v2.1.0 --from package   # resume after a failed stage
python scripts/pipeline.py v2.1.0 --skip test      # skip a stage
python scripts/pipeline.py v2.1.0 --no-bump        # declarations already correct
python scripts/pipeline.py v2.1.0 --clobber        # republish an existing tag
python scripts/pipeline.py v2.1.0 --force          # publish a dirty tree / move a tag
python scripts/pipeline.py v2.1.0 --from ship      # retry just the upload
```

Makefile shortcuts: `release`, `check`, `version`, `build`, `test`, `package`,
`portable`, `verify`, `ship`, `republish`, `dry-run`, `clean`.


---

## 5. Output layout (`dist/<tag>/`)

```text
dist/<tag>/
├── Aurelia-<tag>-win64.zip      # Portable distribution (exe + docs)
├── Aurelia-Setup-<ver>.exe      # Per-user Inno Setup installer
├── RELEASE_NOTES.md             # release-notes/<tag>.md + the VirusTotal block
├── virustotal-summary.txt       # Engine verdicts for both artifacts (hashed + uploaded)
├── audit/
│   ├── virustotal-summary.txt
│   └── virustotal-<artifact>.json
├── SHA256SUMS.txt               # Written by the verify stage
└── PUBLISH.txt                  # Written by the publish stage (commit, remote, asset list)
```

Plus, in the repository: `release-notes/<tag>.md` (authored) and
`releases/<tag>.json` (pipeline record).

---

## 6. The installer

`installer/aurelia.iss` is a per-user Inno Setup script (adapted from Inkwell's):

- installs to `%LOCALAPPDATA%\Programs\Aurelia` with `PrivilegesRequired=lowest`
  (no UAC prompt);
- optional `.aur` / `.hcv` associations using the same `Aurelia.Project` ProgID the
  app's `--register-shell` writes, so both mechanisms agree;
- Start-menu entries plus an optional desktop shortcut, uninstall entry with the app icon;
- uninstall clears an extension association **only if it still points at Aurelia**;
- `CloseApplications=yes`, so upgrading replaces a running binary safely.

The pipeline passes `MyAppVersion`, `MyAppVersionNumeric`, `MySourceDir` (the staging
folder) and `MyOutputDir` (the tag's `dist/` folder) as `/D` defines, then verifies the
produced `Setup` file is a real PE image above a size floor.

---

## 7. VirusTotal scanning (API v3)

The `virustotal` stage scans **both shipped binaries** — `Aurelia-v<ver>-win64.zip`
and `Aurelia-Setup-<ver>.exe` — and refuses to publish if any engine reports a
detection (`--force` overrides).

**Key handling.** The API key is read from the environment only:

```powershell
$env:VIRUSTOTAL_API_KEY = "<your key>"   # current session
python scripts/pipeline.py v2.2.1
```

Nothing reads `.env` and nothing is written to the repository. Without the variable the
stage fails with instructions; pass `--skip virustotal` to ship without a scan.

**API contract** (`scripts/virustotal.py`, verified against the official docs):

| Call | Purpose |
| :--- | :--- |
| `GET /files/{sha256}` | Reuse an existing report; a record with all-zero `last_analysis_stats` means "submitted but never analysed" |
| `POST /files` | Multipart upload of one file (≤ 32 MB) → analysis id |
| `POST /files/{sha256}/analyse` | Re-analyse a sample VT already holds (used for known hashes and `409 AlreadyExistsError`) |
| `GET /analyses/{id}` | Poll until `status == completed` **and** engines actually reported |

Errors follow `{"error": {"code", "message"}}`; the client retries `429 QuotaExceededError`,
`503 TransientError` and `504 DeadlineExceededError`, and treats `404 NotFoundError` as
"not visible yet" while polling.

**Quota.** The public API allows **4 requests/minute and 500/day** (daily reset 00:00 UTC),
so every request goes through a single pacer (`PUBLIC_MIN_INTERVAL = 15 s`). A full
two-artifact scan typically uses 6–12 requests.

**Outputs.** `dist/<tag>/virustotal-summary.txt` (hashed and uploaded like any asset),
`dist/<tag>/audit/virustotal-*.json` (per-artifact record) and a fenced
`<!-- virustotal:begin --> … <!-- virustotal:end -->` block appended to
`release-notes/<tag>.md`, which becomes the release body.

---

## 8. Uploading artifacts

`ship` runs `scripts/release_page.py <tag>`:

- resolves a token from `AURELIA_GITHUB_TOKEN`, `GITHUB_TOKEN` or `GH_TOKEN`, falling back
  to the credential `git push` already uses (`git credential fill`);
- creates the release page from `release-notes/<tag>.md` if it does not exist;
- **refuses to upload** any file whose hash disagrees with `SHA256SUMS.txt`;
- replaces existing assets rather than duplicating them.

`python scripts/release_page.py --all-missing` attaches artifacts for every local tag
that lacks a release page.

---

## 9. Verification & integrity

```powershell
# 1. Checksums shipped with the release
Get-FileHash .\Aurelia-Setup-2.1.0.exe -Algorithm SHA256

# 2. Local re-verification of an existing build
make verify TAG=v2.1.0

# 3. Confirm the tag matches origin
git ls-remote --tags origin v2.1.0
```

A `--clobber` republish re-runs every stage and compares the new asset hashes against
`releases/<tag>.json`; a mismatch is reported unless `--clobber` is passed, in which case
the new bytes are recorded as the next publication.

---

## 10. Installation

See [`INSTALL.md`](INSTALL.md). Short version:

```powershell
# Installer (recommended)
.\Aurelia-Setup-2.1.0.exe

# Portable
Expand-Archive .\Aurelia-v2.1.0-win64.zip -DestinationPath .\Aurelia
.\Aurelia\aurelia.exe
```
