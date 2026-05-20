# ADR 0002: Adopt TypeScript Across Stack

## Status

Approved

## Context

A critical goal of the Transit Intelligence Platform is type safety and clean interface boundaries. Code redundancy (such as duplicating DTOs between API and UI) leads to runtime failures. A strongly typed language is necessary to guarantee consistency, especially with AI agents generating code.

## Decision

We adopt **TypeScript** as the primary programming language for the frontend (`web`), backend (`api`), and node-based utilities.

- Python is permitted strictly for analytics, data science, and ML workers where the library ecosystem (e.g. Pandas, NumPy) offers distinct advantages.
- All TypeScript packages must operate under strict compilation settings.

## Consequences

- **Pros:**
  - Standardized tooling and types across frontend and backend.
  - Fewer runtime type mismatches.
  - Auto-completion and immediate API verification.
- **Cons:**
  - Slightly longer compilation step.
  - Developers/agents must explicitly declare and maintain strict typings.
