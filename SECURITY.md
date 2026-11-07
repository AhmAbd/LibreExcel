# Security Policy

## Supported versions

Security fixes go into the latest release and the `main` branch.

| Version | Supported |
|---|---|
| latest release | Yes |
| older releases | No |

## Reporting a vulnerability

Do not open a public issue for security problems.

Report them privately through
[GitHub Security Advisories](https://github.com/AhmAbd/LibreExcel/security/advisories/new).
Include:

- A description of the problem and its impact
- Steps to reproduce, and a sample file if the problem is triggered by opening a workbook
- The Tabula version and your operating system

You can expect a first response within 7 days. When the fix is ready, we
publish a release and credit you in the advisory, unless you prefer to stay
anonymous.

## Scope

Tabula opens files from untrusted sources (`.xlsx`, `.ods`, CSV). Crashes,
hangs or memory issues caused by a crafted file are in scope.
