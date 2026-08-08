# ADR 0005: AI-Assisted Development Governance

## Status

Approved

## Context

AI agents are key contributors to this project. Unchecked generation can lead to duplicate patterns, loose typings, security risks, and structural entropy.

## Decision

- Establish a strict review system:
  1. No code is merged to main without human review.
  2. All agent actions must satisfy the Definitions of Done (DoD) in `AGENTS.md`.
  3. Pre-commit/PR pipelines must run strict linter (`eslint`) and type checking (`tsc --noEmit`).
  4. Changes to core APIs must update matching schemas and mock tests.

## Consequences

- **Pros:**
  - High quality codebase despite rapid AI agent updates.
  - Transparent verification boundaries.
- **Cons:**
  - Increased human operator review time.
