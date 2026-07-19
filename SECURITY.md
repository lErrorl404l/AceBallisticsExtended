# Security Policy

## Reporting a Vulnerability

ABE is an ARMA 3 mod — the attack surface is limited to ballistic simulation
data and the Rust extension. However, if you discover a security issue:

- **Do not** open a public GitHub issue.
- Email the maintainers directly, or open a private security advisory at:
  <https://github.com/lErrorl404l/AceBallisticsExtended/security/advisories/new>

We aim to acknowledge reports within 72 hours and provide an initial assessment
within one week.

## What to report

- Unsafe Rust code (potential memory corruption in the extension)
- Path traversal or arbitrary file access through mod data loading
- Remote code execution vectors via SQF or extension interface
- Credential or signing key exposure

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| Latest release | ✅ |
| Development builds | ⚠️ — expect instability |
| Older releases | ❌ |
