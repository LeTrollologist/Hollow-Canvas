# Aurelia release orchestrator

One engine, two profiles (`git` and `full`), journal-backed execution. No
changelog generation.

This is a working slice of the design:

- **Implemented:** preflight, version bump + release commit, annotated/signed
  tag, atomic push, independent remote verification, write-ahead journal,
  skip-only-when-verified logic, compensation planning, local rollback, hosted
  yank, GitHub draft/upload/publish with remote digest verification, Ed25519
  artifact signing with checksum manifests, a configurable VirusTotal policy,
  automatic local compensation on failure, and command stages for `quality`,
  `build`, `package`, and `sign`.
- **Not wired yet:** SBOM/license bundling and the concrete Aurelia
  build/package commands (the `full` profile takes them from `[commands]`).

## Quick start

From the repository root:

```powershell
$env:PYTHONPATH = "tools/release"

# Read-only preview. No lock, no journal writes, no mutations.
python -m aurelia_release --repo . plan v3.1.0

# Execute (or resume) the configured profile.
python -m aurelia_release --repo . run v3.1.0

# Inspect journal state after a stop.
python -m aurelia_release --repo . status v3.1.0

# Independent re-verification of every completed operation.
python -m aurelia_release --repo . verify v3.1.0
```

Global flags (`--repo`, `--config`, `--profile`, `--break-lock`) go **before**
the subcommand.

## Configuration

`release.toml` at the repository root is optional; without it the `git` profile
and the workspace version files are used. See `release.toml.example`.

The two default profiles are:

```toml
[profiles]
git  = ["preflight", "version", "tag", "publish", "verify"]
full = ["preflight", "version", "quality", "build", "package", "sign", "scan", "tag", "publish", "release", "verify"]
```

`full` requires `[commands]` for every enabled command stage and a GitHub
token (`GITHUB_TOKEN` or `[publish].token_env`) unless `[publish].github = false`.
There is no hidden build logic: if a stage is listed, it must have a command.

Commands can use `{tag}`, `{version}`, `{dist}`, `{repo}`, and `{iscc}`.
`{iscc}` resolves Inno Setup's compiler from `ISCC`/`INNO_ISCC`, `PATH`,
`Program Files`, or the per-user `%LOCALAPPDATA%\Programs\Inno` and
`%LOCALAPPDATA%\Programs\Inno Setup 6` layouts. Using it when the compiler is
missing fails preflight, and `doctor` prints the resolved path.

## Invariants

1. **Write-ahead journal.** Every mutation writes an `intent` record before it
   runs and a `result` record after it. A crash leaves evidence, not a guess.
2. **Verification is separate from execution.** A stage is not complete when
   its command exits zero; its `verify()` callback re-reads the mutated world.
3. **Skips require proof.** An operation is skipped only when its input
   fingerprint matches, its recorded output hashes still match, its probe still
   passes, and its verification passed.
4. **Config drift is refused.** A journal started with a different
   `release.toml`, profile, or base commit is not silently resumed.
5. **Rollback, compensation, and yank are different.**
   - `rollback` undoes local, unpublished work only.
   - `recover` compensates completed mutations of an incomplete release.
   - `yank` records that a published release is withdrawn; history is not
     rewritten.

## GitHub and scanning

The `release` stage is draft → upload → publish, with verification between each
step. Assets are uploaded to a draft, remote size/digest is compared against
the local files, and only then is the release made public. `release.verify`
downloads every published asset and compares its SHA-256; a packaging exit code
is never accepted as proof.

Scanning is policy-driven, not an unconditional veto:

| `scan.mode` | Behaviour |
|---|---|
| `off` | no scan |
| `advisory` | record detections, never block; missing API key is tolerated |
| `threshold` | block at `threshold` detections or any `blocking_engines` hit |
| `strict` | block on any detection |

`allowlist` entries (SHA-256) bypass the policy with an audit trail. THRESHOLD
mode is the default because unsigned binaries routinely trip heuristic and ML
engines; the report is recorded in the journal either way.

## Signing

`[sign].enabled = true` turns the `sign` stage into Ed25519 artifact signing:

- every release asset gets a detached `<asset>.sig`;
- `SHA256SUMS.txt` is generated and signed as `SHA256SUMS.txt.sig`;
- the stage verifies every signature and checksum it just produced;
- `release.verify` re-verifies the published `.sig` files after download.

The private key can be a raw 32-byte seed (binary or 64 hex characters), a
PKCS#8 PEM, or an **encrypted PEM** whose passphrase is read from
`passphrase_env`. The repository ships only `keys/aurelia-publisher.pub`; the
private key is local and gitignored.

Signing needs the optional dependency:

```powershell
python -m pip install "cryptography>=42"
```

## Failure recovery

```powershell
python -m aurelia_release --repo . status v3.1.0
python -m aurelia_release --repo . recover v3.1.0            # dry run
python -m aurelia_release --repo . recover v3.1.0 --apply --yes
```

`recover` walks completed operations in reverse order. Remote compensation is
opt-in (`--include-remote`) and conservative: it deletes a remote tag only when
this run created it. It does not rewrite a branch that may have moved.

`resume` is the same command as `run`; verified operations are skipped.
`[release].auto_compensate` controls automatic cleanup after a failure:
`"local"` (default) undoes local changes only, `"all"` also compensates remote
mutations, and `"none"` leaves the journal for manual `recover`.

## Journal

State lives in `.release/` (gitignored):

```text
.release/
  lock
  journal/
    v3.1.0.jsonl
```

Each record carries `seq`, `ts`, `type`, a SHA-256 `checksum`, and
type-specific fields:

| Type | Fields |
|---|---|
| `release` | release, version, profile, base_commit, config_digest |
| `intent` | op, stage, fp, before |
| `result` | op, stage, status, outputs, error |
| `verify` | op, stage, status, details, error |
| `compensate` | op, status, error |
| `rollback` | release, base_commit |
| `yank` | release, reason |

A trailing partial line (a crash mid-append) is ignored. A checksum mismatch
anywhere earlier is a hard failure.

## Tests

```powershell
python -m pytest tools/release/tests -q
```

`conftest.py` provides a normal-mode `tmp_path` and disables pytest's default
temp factory because the Windows sandbox used for this repository denies
listing `0o700` directories.

The end-to-end test uses real git for commits/tags and an in-memory remote
transport, because the sandbox also blocks the MSYS helper git's local-file
push spawns. The push command itself is covered by a command-construction test.

## Layout

```text
tools/release/
  aurelia_release/
    config.py      # release.toml, profiles, digest
    journal.py     # append-only checksummed journal, state replay
    engine.py      # check/run/verify/compensate execution
    stages.py      # preflight, version, tag, publish, verify, command stages
    recovery.py    # compensation plan/apply, rollback, yank
    gitops.py      # injectable git wrapper
    versioning.py  # declaration discovery/rewrite/consistency
    cli.py         # doctor, plan, run/resume, status, verify, recover, rollback, yank
  tests/
```
