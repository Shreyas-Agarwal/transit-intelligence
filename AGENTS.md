# AI-Assisted Engineering & Governance

This project is built using a combination of a human software architect/operator and multiple AI coding agents. To maintain codebase cleanliness, avoid architectural drift, and ensure high code quality, we adhere to the strict rules of engagement defined below.

## 1. Allowed AI Responsibilities

AI agents are encouraged to:

- Generate boilerplates, unit tests, and integrations matching established patterns.
- Refactor local modules to improve performance or code readability.
- Write initial system design overviews and markdown document scaffolds.
- Propose bugs/security patches and identify potential performance bottlenecks.

## 2. Disallowed AI Responsibilities

AI agents **must not**:

- Add new framework dependencies or libraries without human operator consent.
- Modify core database schemas or transactional flow logic without an approved Architecture Decision Record (ADR).
- Commit code directly to protected repository branches. All changes must go through pull requests with human approval.
- Delete, truncate, or bypass existing test suites to pass CI.

## 3. Human Review Checklist

The human operator evaluates all AI pull requests against the following criteria:

- **Architectural Match:** Does this follow the established monorepo patterns and packages?
- **Type Rigidity:** Are there any `any` types, loose coercions, or ignored TS errors?
- **Mock/Real Data separation:** Did the agent write robust unit/integration tests with real-like mocks instead of omitting tests?
- **Documentation Sync:** Did the agent update the relevant markdown files (e.g., APIs, ADRs) if any system contracts changed?

## 4. Definition of Done (DoD)

An implementation is considered complete if and only if:

1. **Types Compile:** `pnpm run build` runs with zero TypeScript compile errors.
2. **Lint and Format Pass:** `pnpm run lint` and `pnpm run format:check` execute with zero issues.
3. **Tests Succeed:** All unit, integration, and validation tests pass successfully.
4. **Docs Updated:** Architecture overlays, API references, or ADR logs are updated to match the changes.
