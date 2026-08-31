# Hollow Canvas: Local Release & Packaging Process

This document outlines the standardized, local-first release pipeline for Hollow Canvas. All builds, tests, packaging, and publishing operations run deterministically on the local developer machine without relying on external CI/CD services.

---

## 1. Overview & Core Principles

1. **No GitHub Actions**: Everything is built, tested, and published locally via `scripts/pipeline.py` and the `gh` CLI.
2. **Canonical Asset Naming**: All assets follow `hollow-canvas-v{VER}-{platform}-{arch}.{ext}`.
3. **Clean Distribution Bundles**: Only standalone archives (`.zip` and `.vpack`) and `SHA256SUMS.txt` are published as release assets.
4. **VPack Integration**: Every release includes a high-compression `.vpack` archive compatible with the `vpack` archiver.

---

## 2. Release Pipeline Stages

| Stage | Operation | Description |
| :--- | :--- | :--- |
| **`preflight`** | Tool verification | Confirms `rustc`, `cargo`, `gh`, and `vpack` are present |
| **`build`** | Optimized compilation | `cargo build --release -p hollow-app` |
| **`test`** | Test execution | `cargo test --workspace` |
| **`security`** | Audit logs | Generates `dist/v{VER}/audit/security-audit.txt` |
| **`package`** | Asset bundling | Generates `.zip` and `.vpack` archives |
| **`verify`** | Checksums & lint | Generates `SHA256SUMS.txt` and tests `.vpack` integrity |
| **`publish`** | GitHub Release | Creates release on GitHub and uploads canonical assets |

---

## 3. Running a Release

### Full Release (Build, Test, Package & Publish)
```bash
python scripts/pipeline.py v0.1.0
```
*or via Make:*
```bash
make release TAG=v0.1.0
```

### Local Build & Package Only (No Upload)
```bash
python scripts/pipeline.py v0.1.0 --no-publish
```

### Create as GitHub Draft Release
```bash
python scripts/pipeline.py v0.1.0 --draft
```

---

## 4. Output Layout (`dist/`)

```text
dist/v0.1.0/
├── windows-staging/                                # Temporary staging folder
├── hollow-canvas-v0.1.0-windows-x86_64.zip         # Standard Zip distribution
├── hollow-canvas-v0.1.0-windows-x86_64.vpack       # VPack distribution
├── SHA256SUMS.txt                                  # SHA-256 Checksums
├── release_notes.md                                # Release markdown body
└── audit/
    └── security-audit.txt                          # Local security audit log
```

---

## 5. Verification & Integrity

To verify released packages:
```bash
# Check SHA-256
certutil -hashfile hollow-canvas-v0.1.0-windows-x86_64.zip SHA256

# Verify VPACK integrity and CRC-32
vpack test hollow-canvas-v0.1.0-windows-x86_64.vpack
```

---

## 6. Installation & Extraction

### Option A: Via VPack Archiver
```bash
# Extract all contents
vpack extract hollow-canvas-v0.1.0-windows-x86_64.vpack

# Or extract to a specific folder
vpack extract hollow-canvas-v0.1.0-windows-x86_64.vpack -o ./HollowCanvas/
```

### Option B: Via Native Windows Zip
```powershell
Expand-Archive -Path .\hollow-canvas-v0.1.0-windows-x86_64.zip -DestinationPath .\HollowCanvas
```
