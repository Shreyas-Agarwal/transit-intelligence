# ADR 0001: Use Monorepo Architecture

## Status

Approved

## Context

The Transit Intelligence Platform contains multiple client applications, backend APIs, data workers, and shared logic. Dividing these into separate repositories would result in significant dependency sync challenges, CI/CD duplication, configuration drift, and friction when modifying shared interfaces.

## Alternatives Considered

1. **Multi-Repo Structure:** Each application and package runs in a separate git repository. Rejected due to heavy coordination overhead and difficulty in synchronizing interface changes across boundaries.
2. **Standard npm Workspaces:** Workspace management with npm. Rejected because it lacks the caching, parallelization, and dependency pruning capabilities of pnpm + Turborepo.

## Decision

We will use a **monorepo** structure powered by **pnpm workspaces** and **Turborepo** (`turbo`).

## Consequences

- **Pros:**
  - Easy cross-workspace refactoring and unified code base.
  - Simplified package-sharing (e.g., shared-types, shared-logger).
  - Faster builds and testing via Turborepo caching.
  - Centralized tooling (ESLint, Prettier, TypeScript).
- **Cons:**
  - Increased local checkout size.
  - Requires clean boundaries to prevent tight coupling between packages.
