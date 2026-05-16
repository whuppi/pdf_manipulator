# Security Policy

## Reporting a vulnerability

If you discover a security vulnerability, please report it privately via [GitHub Security Advisories](https://github.com/whuppi/pdf_manipulator/security/advisories/new).

Do **not** open a public issue for security vulnerabilities.

## Scope

This package processes untrusted PDF input. Relevant concerns include:

- Buffer overflows in the Rust engine (pdf_oxide)
- Path traversal via embedded file names
- Denial of service via malformed PDFs
- Information disclosure via error messages

## Response

Reports are acknowledged within 48 hours. Fixes are released as patch versions.
