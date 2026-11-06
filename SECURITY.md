# Security Policy

## Supported versions

Security fixes are provided for the latest release only. The current release is
[v3.4.1](https://github.com/LeTrollologist/Aurelia/releases/tag/v3.4.1).

## Reporting a vulnerability

Please **do not** open a public issue for a suspected vulnerability. Email
`trollologistog@gmail.com` with the subject `[SECURITY] Aurelia Vulnerability Report` and include:

- affected version and Windows build;
- steps or a minimal proof-of-concept;
- impact assessment;
- whether the issue involves a project file, an import, the installer, or the release tooling.

You should receive an acknowledgement within 48 hours.

## Artifact integrity

- Release binaries and `SHA256SUMS.txt` carry detached Ed25519 signatures.
- The public key is committed at `keys/aurelia-publisher-2026.pub` and attached to releases.
- The private key is local, gitignored, and never committed. Rotation and loss procedures are in
  [`RELEASE_PROCESS.md`](RELEASE_PROCESS.md).
- The installer is **not Authenticode-signed**. This is a permanent distribution constraint:
  no public certificate path is available, and a self-signed certificate would not establish
  SmartScreen trust. SmartScreen reputation is independent of the artifact signature.

```powershell
certutil -hashfile Aurelia-Setup-3.4.1.exe SHA256
Get-Content SHA256SUMS.txt
```

Verify the `.sig` files against the included public key before running an installer. A matching
Ed25519 signature is the integrity boundary; a hash alone only proves the download matches the
manifest.

## VirusTotal policy

VirusTotal is **advisory, not a release veto**. Unsigned software routinely triggers heuristic
and ML engines; a detection count is not by itself evidence of malware.

- Scanning is enabled only when `VIRUSTOTAL_API_KEY` is present.
- `scan.mode` supports `off`, `advisory`, `threshold`, and `strict`; the current release uses
  `advisory`.
- Reports are attached to the journal/release where available; quota limits are respected.
- A clean scan is not proof of safety, and a positive scan is not proof of compromise.

## Safety boundary

Application logic is safe Rust. Win32/GDI/COM calls, the MSVC C runtime, and third-party image
decoders execute outside Rust’s memory-safety guarantees. See
[`docs/SECURITY_MODEL.md`](docs/SECURITY_MODEL.md).

## Network and privacy

Aurelia has no telemetry, no auto-update, and no outbound network connections. Optional Discord
Rich Presence talks only to the local Discord client over a named pipe; it is off by default.
Release tooling is separate from the application and may contact GitHub/VirusTotal.

## Dependencies and releases

- `cargo audit` is a manual pre-release check (see `RELEASE_PROCESS.md`).
- Releases are built locally by the journal-backed orchestrator; there is no third-party CI
  runner.
- Release artifacts are built from a clean checkout and verified after upload by downloading and
  re-hashing them.
