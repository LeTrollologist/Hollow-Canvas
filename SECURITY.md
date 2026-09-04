# Security Policy

## Supported Versions

We provide security updates and patches for the following versions of **Hollow Canvas**:

| Version | Supported          |
| ------- | ------------------ |
| 0.17.x  | :white_check_mark: |
| 0.16.x  | :white_check_mark: |
| 0.15.x  | :white_check_mark: |
| 0.14.x  | :white_check_mark: |
| 0.13.x  | :white_check_mark: |
| 0.12.x  | :white_check_mark: |
| 0.11.x  | :white_check_mark: |
| 0.10.x  | :white_check_mark: |
| < 0.10.0| :x:                |

---

## Ed25519 Digital Signing & Chain of Trust (VPack 2.0.0)

Starting in **v0.15.0**, all official `.vpack` release packages are cryptographically signed with our sovereign **Ed25519 publisher keypair** to guarantee supply chain integrity and eliminate tampering:

- **Publisher Public Key**: `3af86cc3d8c5181d12409d73e75dc03bad704fa2946be5e04da4e57044ec5f2f`
- **Key Asset**: `hollow-publisher.pub` (distributed alongside every release)
- **Signature Verification**: VPack 2.0.0 validates the digital signature automatically on testing or extracting:
  ```bash
  vpack test hollow-canvas-v0.17.0-windows-x86_64.vpack
  ```

---

## Automated Dependency Security Audit (`cargo audit`)

Every release build is scanned through `cargo audit` before packaging to verify that zero dependencies contain known security advisories or vulnerabilities tracked by the [RustSec Advisory Database](https://rustsec.org/):

1. **Pre-Release Security Gate**: The release pipeline automatically aborts if vulnerable crates are detected in the workspace dependency graph.
2. **Audit Logging**: The full dependency advisory report is generated and archived in `dist/v{VERSION}/audit/cargo-audit.txt` and `security-audit.txt`.
3. **Reproducible Local Verification**: You can run the dependency audit locally at any time:
   ```powershell
   cargo audit
   ```

---

## VirusTotal & Binary Integrity Verification

Every official release distribution package (`.zip` and `.vpack`) undergoes automated binary analysis, cryptographic hashing, and VirusTotal verification prior to publication:

1. **Automated VirusTotal Scanning**: Release bundles are verified against 70+ antivirus and anti-malware engines via VirusTotal API v3.
2. **Cryptographic SHA-256 Checksums**: Every release artifact includes an official SHA-256 digest in `SHA256SUMS.txt` and an attached `virustotal-summary.txt` audit report.
3. **Independent Verification**: Users can independently verify any downloaded release archive against VirusTotal using its SHA-256 hash or by uploading the `.zip` archive directly to [VirusTotal](https://www.virustotal.com/gui/home/upload).

### How to Verify a Downloaded Release
```powershell
# 1. Compute SHA-256 on Windows
certutil -hashfile hollow-canvas-v0.17.0-windows-x86_64.zip SHA256

# 2. Check the VirusTotal analysis report directly in your browser:
# https://www.virustotal.com/gui/file/<SHA256_HASH>
```

---

## Reporting a Vulnerability

The Hollow Canvas team takes the security of our application and our users seriously. If you believe you have discovered a security vulnerability in Hollow Canvas, please follow these steps:

1. **Do NOT disclose publicly**: Please do not file public GitHub issues or discuss potential vulnerabilities in public forums.
2. **Contact Privately**: Send an email directly to the maintainer at `trollologistog@gmail.com` with the subject line `[SECURITY] Hollow Canvas Vulnerability Report`.
3. **Include Details**:
   - A detailed description of the vulnerability.
   - Steps or proof-of-concept to reproduce the issue.
   - The version of Hollow Canvas and your OS environment.
   - Potential impact of the issue.

### Response Timeline
- **Acknowledgement**: You will receive an initial response within 48 hours.
- **Assessment**: We will verify the vulnerability and evaluate its severity.
- **Fix & Disclosure**: We will work on a patch and coordinate a disclosure timeline once a fix is released.

---

## Security Invariants & Design Principles

Hollow Canvas is engineered with strict local security principles:

- **Memory Safety**: Hollow Canvas is implemented in 100% Rust, taking full advantage of Rust's compile-time memory safety, thread safety, and boundary checks to prevent buffer overflows and memory corruption.
- **Offline First**: Hollow Canvas does not make outbound network connections, run background telemetry, or transmit user artwork.
- **Deterministic File Parsing**: Project archives (`.hcv`) and imported image files are parsed using memory-safe decoders with strict bounds validation to prevent arbitrary execution or corrupted heap allocations.
- **Reproducible Local Pipeline**: Release builds are produced locally using the deterministic pipeline in `scripts/pipeline.py` without third-party CI/CD runner vulnerabilities.
