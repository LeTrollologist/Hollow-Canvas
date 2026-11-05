# Aurelia Release Process

Aurelia releases are cut on a developer machine, never in CI. The release system is the
journal-backed orchestrator in [`tools/release`](tools/release), configured by
[`release.toml`](release.toml). `scripts/pipeline.py` is retired and kept only for historical
reference; do not use it for new releases.

---

## 1. Principles

1. **Local-first.** Nothing builds on GitHub. A release runs from a clean checkout with the
   signing key and tokens available on that machine.
2. **Git is the record.** A release is an annotated tag on a verified commit. The journal,
   release page and assets are derived from it.
3. **Write-ahead journal.** Every mutation records an intent, then a result and an independent
   verification in `.release/journal/<tag>.jsonl`. Failed runs are resumed or compensated,
   never guessed.
4. **Artifacts are signed.** Ed25519 detached signatures cover every binary and
   `SHA256SUMS.txt`; the tag itself is annotated but not GPG/SSH-signed.
5. **No incomplete publication.** Assets upload to a GitHub draft, are verified by digest, and
   only then is the release published.

---

## 2. One-time setup

| Requirement | Purpose |
| :--- | :--- |
| Rust toolchain + MSVC linker | build `aurelia-app` |
| Inno Setup 6 (`ISCC.exe`) | installer; discovered from `%LOCALAPPDATA%\Programs\Inno`, Program Files, PATH or `ISCC`/`INNO_ISCC` |
| `GITHUB_TOKEN` | create/upload/publish the GitHub release (`repo` scope) |
| `keys/aurelia-publisher-2026.priv` | Ed25519 artifact signing (local only, gitignored) |
| `keys/aurelia-publisher-2026.pub` | public verification key (committed) |
| `VIRUSTOTAL_API_KEY` | optional; `scan.mode = "threshold"` or `"strict"` |

`release.toml` selects the `full` profile, the version files, the Windows package command,
signing key, scan policy and GitHub repository. The private key must **never** be committed;
`keys/*.priv` is ignored and verified absent from git history before a release.

---

## 3. Cut a release

```powershell
$env:PYTHONPATH = "tools/release"

python -m aurelia_release --repo . doctor
python -m aurelia_release --repo . plan v3.1.0
python -m aurelia_release --repo . run  v3.1.0
```

`run` is resumable: repeat the same command after fixing a failure. Journaled, verified
operations are skipped; an interrupted operation requires `status`/`recover` first.

Inspect or recover:

```powershell
python -m aurelia_release --repo . status v3.1.0
python -m aurelia_release --repo . verify v3.1.0
python -m aurelia_release --repo . recover v3.1.0            # dry run
python -m aurelia_release --repo . recover v3.1.0 --apply --yes
python -m aurelia_release --repo . rollback v3.1.0           # local, unpublished only
python -m aurelia_release --repo . yank v3.1.0 --reason "…"  # withdrawn published release
```

---

## 4. Stages

| Stage | Operations | Notes |
| :--- | :--- | :--- |
| `preflight` | clean tree, branch, remote, tag, signing key, version consistency | no mutations |
| `version` | rewrite declarations, commit `chore(release): vX.Y.Z` | includes every crate, docs and `Cargo.lock` |
| `quality` | `cargo test --release --workspace` | configurable command |
| `build` | `cargo build --release -p aurelia-app` | configurable command |
| `package` | portable zip + Inno installer | `scripts/package_aurelia.py` |
| `sign` | Ed25519-sign assets and `SHA256SUMS.txt`, verify locally | no changelog generation |
| `scan` | VirusTotal policy (`off`/`advisory`/`threshold`/`strict`) | advisory tolerates no key |
| `tag` | annotated tag, signature optional | `sign_tags = false` in current config |
| `publish` | atomic push of branch + tag | verifies remote refs |
| `release` | draft → upload → digest verify → publish | assets never go straight to a published release |
| `verify` | re-read refs, download assets, verify hashes/signatures | independent of `dist/` |

Profiles are configuration, not separate code: `git` runs preflight → version → tag → publish →
verify; `full` adds build, package, sign, scan and the GitHub release stages.

---

## 5. Signing keys

### Current key

- Public: `keys/aurelia-publisher-2026.pub` (32-byte raw Ed25519, hex)
- Private: `keys/aurelia-publisher-2026.priv` (local, gitignored)
- Fingerprint: SHA-256 of the public key file
  `d5183063edb79a1d584bbbbd67488e5cd97c7069e4b0d88c431d478c9d216717`

The previous `keys/aurelia-publisher.pub` has no known private key and must not be used for new
signatures.

### Rotation

1. Generate a new Ed25519 keypair offline.
2. Sign a rotation statement with the old key (if available) and the new key.
3. Commit the new public key as `keys/aurelia-publisher-<year>.pub`.
4. Update `[sign].private_key`/`public_key` in `release.toml`.
5. Publish the rotation statement with the next release; keep old public keys in the repo for
   verifying historical releases.
6. Never reuse a key after its private half is suspected compromised.

### Lost key

If the private key is lost, do **not** fabricate signatures. Publish a key-rotation notice, sign
the next release with the new key, and document that older artifacts remain verifiable only
against the old public key. A release whose private key is unavailable cannot be re-signed
retroactively without changing its published bytes.

---

## 6. Failure, rollback and withdrawal

- **Resume** is the first response to a failure. The journal shows the last verified operation.
- **Compensation** (`recover`) undoes completed mutations of an incomplete release; remote
  compensation is opt-in and only deletes refs this run created.
- **Rollback** is local-only: unpublished commits, tags and drafts.
- **Yank** marks a published release withdrawn without rewriting history; assets remain
  available unless separately removed.
- `auto_compensate = "local"` cleans local partial state automatically; remote changes still
  require an explicit `recover --include-remote`.

A published release is never treated as atomic with its GitHub page: if the tag is pushed and
the release upload fails, the journal records a partial state and the release must be resumed or
yanked.

---

## 7. Clean-machine rehearsal

Before trusting a release run:

1. Clone the repository into a fresh directory.
2. Provide only the private key, `release.toml` and a GitHub token.
3. Run `doctor`, `plan` and `run` against a test tag.
4. Verify the downloaded assets independently.
5. Confirm `git log --all -- keys/*.priv` is empty and `git check-ignore` covers the key.

## 8. Failure-injection coverage

The suite now covers the failure modes that exercise the journal's recovery paths:

- **Partial upload**: the second asset disconnects; the draft survives, resume uploads only the
  missing asset and publishes without duplicates.
- **Digest mismatch**: a corrupted download fails `release.upload` before publication; resume
  verifies the corrected bytes and publishes.
- **Release creation failure**: a failed GitHub draft leaves a resumable journal; resume
  creates and publishes it.
- **Remote tag drift**: a tag this run did not create is refused at preflight.
- **Interrupted operation**: a `running` journal entry refuses to resume until `recover`.
- **Published-draft verification**: re-verifying after publication accepts the published state.

Still worth adding: a killed process during the journal append itself (partial final line), a
compensation that fails partway, and a clean-clone rehearsal against a throwaway remote. The
orchestrator is now tested for the resume paths it depends on, not only the happy path.
