# Contributing Guidelines

Thank you for contributing to the Transit Intelligence Platform. To ensure a smooth development process and maintain high standards, please follow the instructions below.

## Monorepo Workflow

Per ADR 0013 (domain-first workspace organization), this is a polyglot monorepo organized around bounded contexts under `domains/`, not a single shared language workspace. Each domain is independently buildable, testable, and lintable using only the manifests inside it — there is no repository-wide install/build/test command.

### Setup Instructions

`cd` into the domain you're working on and use its own toolchain:

- **`domains/ingestion`** (Rust, Cargo workspace):
  ```bash
  cd domains/ingestion
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets
  cargo test --workspace
  ```
- **`domains/gtfs_s`** (Python, `uv`-managed):
  ```bash
  cd domains/gtfs_s
  uv sync
  uv run ruff check .
  uv run pyright
  ```

`mise.toml` at the repository root pins the default toolchain versions (Rust, Python, etc.) used across domains.

## Branching Model

Name your branches using the following prefixes:

- `feature/` for new capabilities (e.g., `feature/gps-ingestion`)
- `bugfix/` for bug fixes (e.g., `bugfix/connection-leak`)
- `refactor/` for code restructuring (e.g., `refactor/logger-interface`)
- `docs/` for documentation updates (e.g., `docs/add-adr-0005`)
- `chore/` for build or maintenance tasks (e.g., `chore/bump-turbo`)

## Commit Messages

We enforce the **Conventional Commits** specification:

```text
<type>(<scope>): <description>

[optional body]
```

### Allowed Types:

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation changes
- `style`: Changes that do not affect code logic (formatting, whitespace)
- `refactor`: A code change that neither fixes a bug nor adds a feature
- `perf`: A code change that improves performance
- `test`: Adding missing tests or correcting existing tests
- `chore`: Changes to build tooling or auxiliary libraries

### Example:

`feat(api): add validation schema for vehicle coordinate telemetry`

## Pull Request Guidelines

Before submitting a Pull Request:

1. Ensure all TypeScript files compile with no errors.
2. Run `pnpm run format` to clean up file layouts.
3. Add appropriate test coverage.
4. Fill out the pull request template completely.
