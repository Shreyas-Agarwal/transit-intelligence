# Security Policy

Transit Intelligence is a public, pre-1.0 repository, not yet published or deployed anywhere production traffic depends on it. This policy is intentionally lightweight — it'll grow if and when that changes.

## Reporting a Vulnerability

Please **do not** open a public issue for a suspected vulnerability. Instead, use [GitHub's private vulnerability reporting](../../security/advisories/new) for this repository, or email the maintainer directly (see the GitHub profile linked from the repository's commit history) with:

- A description of the issue and its potential impact.
- Steps to reproduce, or a proof of concept if you have one.
- Which domain (`domains/ingestion`, `domains/gtfs_s`, or repository governance tooling) is affected.

You should expect an initial response within **5 business days**. This is a single-maintainer project, so response time may vary — if you haven't heard back after a week, a follow-up nudge is welcome.

## Scope

Secret-scanning (gitleaks) and dependency updates (Renovate) are already automated per-domain — see `tools/governance/` and `.github/renovate.json5`. Reports about a leaked credential are still welcome even if you believe automation should have caught it.
