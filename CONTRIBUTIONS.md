# Contributing to Aurelia

Thank you for your interest in contributing to **Aurelia**.

Aurelia is a local-first digital illustration and graphics studio built with Rust. Its architecture prioritizes predictable behavior, rendering performance, document integrity, offline operation, and maintainable code.

Contributions are welcome, whether they involve bug fixes, performance improvements, documentation, testing, or carefully scoped features.

This document explains how to participate in development and submit changes.

---

## Table of Contents

1. [Before You Begin](#before-you-begin)
2. [Development Environment](#development-environment)
3. [Repository Structure](#repository-structure)
4. [Building Aurelia](#building-aurelia)
5. [Testing and Verification](#testing-and-verification)
6. [Coding Standards](#coding-standards)
7. [Architectural Guidelines](#architectural-guidelines)
8. [Document Format and Compatibility](#document-format-and-compatibility)
9. [Performance Expectations](#performance-expectations)
10. [Security and Privacy](#security-and-privacy)
11. [Submitting Changes](#submitting-changes)
12. [Commit Conventions](#commit-conventions)
13. [Pull Request Review](#pull-request-review)
14. [License](#license)

---

## 1. Before You Begin

Before implementing a change, please check the existing repository for related issues, pull requests, and discussions.

### Bug fixes

Bug fixes are welcome.

When reporting or fixing a bug, provide:

- A clear description of the problem.
- Steps to reproduce it.
- Expected and actual behavior.
- Relevant screenshots, logs, or error messages.
- Information about the affected environment.

Whenever possible, include a regression test demonstrating the issue.

### Feature requests

For substantial features, please open an issue before beginning implementation.

This helps establish:

- Whether the proposed functionality fits Aurelia's direction.
- Whether the feature overlaps with existing functionality.
- Whether the architectural implications have been considered.
- Whether the proposed scope is appropriate.

Small, isolated improvements generally do not require prior discussion.

**Please avoid investing significant development effort into an unapproved architectural change.**

### Documentation

Documentation improvements are welcome, including corrections, clarifications, examples, and improvements to existing guides.

---

## 2. Development Environment

Aurelia is written in Rust and currently targets Windows 10/11 (64-bit).

### Requirements

- Windows 10 or Windows 11 (64-bit).
- Rust toolchain, preferably the version used by the current project release.
- Cargo, included with Rust.
- Git.
- Python 3 for repository release tooling.
- A C compiler and Windows development components where required by dependencies.

Install Rust through:

https://rustup.rs/

Verify your installation:

```powershell
rustc --version
cargo --version
git --version
python --version
```

### Clone the repository

```powershell
git clone https://github.com/LeTrollologist/Aurelia.git

cd Aurelia
```

Update your local checkout:

```powershell
git pull origin main
```

### Create a development branch

Avoid committing directly to `main`.

```powershell
git switch -c fix/short-description
```

Suggested branch prefixes:

| Prefix | Purpose |
|---|---|
| `feat/` | New functionality |
| `fix/` | Bug fixes |
| `perf/` | Performance improvements |
| `refactor/` | Internal restructuring |
| `docs/` | Documentation |
| `test/` | Testing improvements |
| `security/` | Security-related changes |

---

## 3. Repository Structure

Aurelia is organized into separate Rust crates with distinct responsibilities.

```text
Aurelia/
├── assets/
├── crates/
│   ├── aurelia-core/
│   ├── aurelia-render/
│   ├── aurelia-io/
│   ├── aurelia-ui/
│   └── aurelia-app/
├── docs/
├── installer/
├── keys/
├── release-notes/
├── releases/
├── scripts/
├── tools/
│   └── release/
├── Cargo.toml
├── Cargo.lock
├── Makefile
├── README.md
├── SECURITY.md
├── PRIVACY.md
├── RELEASE_PROCESS.md
└── CONTRIBUTING.md
```

### Crate responsibilities

| Crate | Responsibility |
|---|---|
| `aurelia-core` | Document models, layers, rasterization, geometry, and editing operations |
| `aurelia-render` | Software rendering, compositing, viewport rendering, and rendering primitives |
| `aurelia-io` | Native document serialization, parsing, and image import/export |
| `aurelia-ui` | Application interface, panels, tools, and interaction |
| `aurelia-app` | Windows host, application lifecycle, input handling, and CLI |

Please keep functionality within its appropriate crate.

Avoid introducing unnecessary cross-crate dependencies or moving responsibilities between crates without a clear architectural reason.

---

## 4. Building Aurelia

Aurelia uses Cargo workspaces.

To build the application in development mode:

```powershell
cargo build --workspace
```

To build the optimized application:

```powershell
cargo build --release -p aurelia-app
```

To launch the application:

```powershell
cargo run --release -p aurelia-app
```

The project also provides a Makefile for release and verification workflows.

Inspect the existing Makefile before invoking release-related targets.

**Important:** The `make check` target is a local release pipeline operation, not simply a Rust compilation check. It can perform additional packaging and verification work.

Do not use release or publishing targets when you only intend to validate a code change.

---

## 5. Testing and Verification

Testing is an important part of contributing to Aurelia.

All behavioral changes should include appropriate verification.

### Run the workspace tests

```powershell
cargo test --workspace
```

### Check compilation

```powershell
cargo check --workspace --all-targets
```

### Check formatting

```powershell
cargo fmt --all -- --check
```

### Run Clippy

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

These are recommended development checks. Where a particular check is incompatible with the current project configuration, document the limitation rather than silently ignoring it.

### Testing expectations

| Change | Expected verification |
|---|---|
| Bug fix | Regression test |
| New functionality | Tests covering the new behavior |
| Rendering changes | Correctness and visual verification |
| Serialization changes | Round-trip and compatibility tests |
| Performance changes | Before-and-after benchmarks |
| UI changes | Manual interaction testing |
| Security changes | Relevant security and boundary tests |
| Documentation | Verify commands, paths, and technical claims |

Do not remove or weaken existing tests simply to make a contribution pass.

A passing compilation is not a substitute for behavioral testing.

---

## 6. Coding Standards

Contributors should follow the existing conventions throughout the repository.

### Rust conventions

- Follow standard Rust formatting.
- Prefer descriptive names.
- Keep functions focused and reasonably sized.
- Prefer explicit error handling over silent failure.
- Avoid unnecessary allocations in performance-sensitive paths.
- Avoid unnecessary cloning of large document or pixel buffers.
- Use appropriate ownership and borrowing.
- Keep abstractions proportional to their actual use.
- Document non-obvious behavior and important invariants.

### Error handling

Errors should be handled deliberately.

Avoid:

- Silently discarding errors.
- Unnecessary `unwrap()` and `expect()` calls in production paths.
- Returning misleading success values.
- Hiding invalid document states.
- Swallowing failures during saving or exporting.

Error messages should provide enough context to diagnose the problem without exposing sensitive local information.

### Dependencies

New dependencies should be introduced sparingly.

A pull request adding a dependency should explain:

1. What functionality it provides.
2. Why existing functionality is insufficient.
3. Its maintenance and security implications.
4. Whether a smaller alternative exists.

Avoid introducing dependencies for trivial functionality that can reasonably be implemented using existing project utilities.

---

## 7. Architectural Guidelines

Aurelia is designed around a modular, local-first architecture.

Contributions should preserve the separation between document logic, rendering, serialization, UI, and application hosting.

### Core principles

**Separation of concerns**

Do not place application-specific Windows functionality inside platform-independent crates.

**Deterministic behavior**

Where practical, editing and rendering operations should produce predictable results for equivalent inputs.

**Performance awareness**

Avoid unnecessary full-document copies, repeated allocations, and expensive operations inside frequently executed rendering paths.

**Non-destructive editing**

Features should preserve original document data wherever the existing architecture supports non-destructive operations.

**Maintainability**

Prefer straightforward implementations over unnecessary abstractions or speculative generalization.

**Backward compatibility**

Changes to existing behavior, file formats, or public interfaces must be considered carefully.

### Before changing architecture

For substantial architectural changes, explain:

- The existing limitation.
- The proposed design.
- The affected crates.
- Performance implications.
- Compatibility implications.
- Testing requirements.

The maintainer may request a design discussion before accepting implementation work.

---

## 8. Document Format and Compatibility

Aurelia's native `.aur` document format is a critical part of the application.

Changes involving document serialization or deserialization require particular care.

### Compatibility expectations

- Do not silently reinterpret existing document data.
- Preserve existing supported document versions.
- Validate document metadata and serialized inputs.
- Handle malformed or truncated files safely.
- Avoid introducing data loss during loading or saving.
- Keep serialization changes covered by tests.

### Format changes

Changes to the native AUR format should document:

1. The format version involved.
2. The reason for the change.
3. Whether older documents remain readable.
4. Whether older application versions can open newly saved documents.
5. Migration or fallback behavior.
6. Relevant round-trip and compatibility tests.

Breaking compatibility must be intentional and documented.

Do not modify the native document format solely to simplify an isolated implementation.

---

## 9. Performance Expectations

Performance is a core engineering consideration in Aurelia.

Contributors working on rendering, compositing, painting, transformations, or document processing should consider the impact of their changes.

### Guidelines

- Avoid unnecessary intermediate buffers.
- Reuse allocations where appropriate.
- Avoid repeated work when cached results can safely be reused.
- Consider memory consumption for large canvases.
- Preserve correctness when optimizing pixel operations.
- Avoid introducing synchronization overhead without justification.

### Benchmarking

Performance-related pull requests should include measurable evidence.

Where applicable, provide:

- The benchmark used.
- Previous performance.
- New performance.
- Hardware and build configuration.
- Whether results are reproducible.
- Any relevant trade-offs.

Do not claim a performance improvement based solely on theoretical expectations.

An optimization that changes pixel output or introduces instability is not an acceptable improvement.

---

## 10. Security and Privacy

Aurelia is designed as a local-first application.

Contributions must respect its privacy and security expectations.

### Privacy requirements

Do not introduce:

- Telemetry or usage tracking.
- Analytics services.
- Background data collection.
- Undocumented network communication.
- Remote processing requirements.
- Unnecessary external service dependencies.

Any proposed change involving network communication requires explicit architectural discussion and maintainer approval.

### Security requirements

Pay particular attention to:

- Untrusted image and document input.
- Archive extraction and file parsing.
- Buffer boundaries and allocation sizes.
- Path traversal.
- Temporary file handling.
- Atomic save behavior.
- Dependency security.
- Sensitive information in logs.

Never commit:

- API keys.
- Authentication tokens.
- Private signing keys.
- Personal credentials.
- Locally generated secrets.

For suspected vulnerabilities, follow the private reporting instructions in [SECURITY.md](SECURITY.md).

Do not publicly disclose an exploitable vulnerability before coordinating responsible disclosure.

---

## 11. Submitting Changes

Once your changes are ready, submit a pull request.

### Recommended workflow

**1. Update your branch**

```powershell
git switch main
git pull origin main
```

**2. Return to your development branch**

```powershell
git switch fix/short-description
```

Rebase or merge the latest upstream changes as appropriate.

**3. Review your changes**

```powershell
git diff
git status
```

Check for:

- Unintended modifications.
- Debugging statements.
- Temporary files.
- Generated binaries.
- Unrelated formatting changes.
- Accidental configuration or environment changes.

**4. Run relevant tests**

At minimum, run the tests and checks applicable to your changes.

**5. Commit your changes**

```powershell
git add .
git commit -m "fix: correct layer mask compositing"
```

**6. Push your branch**

```powershell
git push origin fix/short-description
```

**7. Open a pull request**

Submit your pull request against the repository's `main` branch:

https://github.com/LeTrollologist/Aurelia/pulls

---

## 12. Commit Conventions

Aurelia uses descriptive commit messages.

The preferred format is:

```text
type: short description
```

Recommended types:

| Type | Usage |
|---|---|
| `feat` | New functionality |
| `fix` | Bug fixes |
| `perf` | Performance improvements |
| `refactor` | Internal restructuring |
| `docs` | Documentation |
| `test` | Test additions or modifications |
| `build` | Build system changes |
| `ci` | Automation and CI changes |
| `chore` | Maintenance tasks |
| `security` | Security improvements |

Examples:

```text
feat: add configurable brush dynamics

fix: prevent invalid layer mask dimensions

perf: reduce compositor buffer allocations

refactor: isolate document validation

docs: clarify AUR compatibility behavior

test: cover malformed document headers

security: validate archive extraction paths
```

Keep commits focused and avoid combining unrelated changes.

---

## 13. Pull Request Review

Every pull request is subject to maintainer review.

A pull request should include:

- A concise summary.
- The problem being addressed.
- The proposed solution.
- Relevant issue references.
- Tests performed.
- Any compatibility implications.
- Screenshots or recordings for visible UI changes.

### Review expectations

The maintainer may request:

- Code modifications.
- Additional tests.
- Architectural clarification.
- Benchmark evidence.
- Security review.
- Documentation updates.
- Changes to the scope of the contribution.

Please respond constructively to review feedback.

A pull request is not automatically accepted because it compiles, passes tests, or has no merge conflicts.

### Maintainer authority

The Aurelia maintainer retains final authority over:

- Project direction and roadmap.
- Architectural decisions.
- Feature acceptance.
- Release decisions.
- Merge approval.
- Changes to project scope.

Contributions are reviewed on their technical merit, compatibility, maintainability, and alignment with Aurelia's direction.

Submitting a pull request does not guarantee acceptance.

---

## 14. License

Aurelia is distributed under the **GNU General Public License v3.0 (GPL-3.0)**.

By submitting a contribution, you acknowledge that your contribution is subject to the repository's applicable licensing terms.

You must have the legal right to submit the code, documentation, assets, or other material included in your contribution.

Do not submit:

- Code copied from incompatible licenses.
- Assets without appropriate redistribution rights.
- Material whose ownership or licensing is unclear.
- Third-party code without the necessary attribution and license information.

See [LICENSE.md](LICENSE.md) for the complete license.

---

## Final Notes

Aurelia values contributions that improve the software without compromising its underlying engineering principles.

Correctness comes before convenience. Document integrity comes before shortcuts. Measurable performance comes before assumptions. Maintainability comes before unnecessary complexity.

Thank you for helping improve Aurelia.

**Aurelia is built carefully, and contributions should be too.**

---

*Maintained by [LeTrollologist](https://github.com/LeTrollologist).*  
*Repository: https://github.com/LeTrollologist/Aurelia*
