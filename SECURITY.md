# Security Policy

## Supported Versions

We provide security updates and patches for the following versions of **Hollow Canvas**:

| Version | Supported          |
| ------- | ------------------ |
| 0.4.1   | :white_check_mark: |
| 0.4.0   | :white_check_mark: |
| 0.3.0   | :x:                |
| 0.2.0   | :x:                |
| 0.1.0   | :x:                 |

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
