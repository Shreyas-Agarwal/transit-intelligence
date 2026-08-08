# Contributing Guidelines

Thank you for contributing to the Transit Intelligence Platform. To ensure a smooth development process and maintain high standards, please follow the instructions below.

## Monorepo Workflow

Per ADR 0013 (domain-first workspace organization), this is a polyglot monorepo organized around bounded contexts under `domains/`, not a single shared language workspace. Each domain is independently buildable, testable, and lintable using only the manifests inside it — there is no repository-wide install/build/test command.

### Setup Instructions

`cd` into the domain you're working on and use its own toolchain.

**`domains/ingestion`** (Rust, Cargo workspace):

```bash
cd domains/ingestion
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

**`domains/gtfs_s`** (Python, `uv`-managed):

```bash
cd domains/gtfs_s
uv sync
uv run ruff check .
uv run pyright
```

`mise.toml` at the repository root pins the default toolchain versions (Rust, Python, etc.) used across domains, plus repository governance tooling (see below).

### Git Hooks

This repository uses [Lefthook](https://github.com/evilmartians/lefthook) for git hooks and [Conform](https://github.com/siderolabs/conform) for commit-message policy — both static binaries pinned in `mise.toml`, chosen so hook enforcement doesn't require any domain's language runtime. After installing the repo's mise tools, install the hooks once:

```bash
mise install
lefthook install
```

`pre-commit` runs EditorConfig, Markdownlint, and Vale against staged files and a gitleaks secret scan; `commit-msg` runs Conform (see below); `pre-push` runs a broader gitleaks scan as defense in depth. None of these run a domain's own build/lint/test — that stays in each domain's CI job (`.github/workflows/`).

Vale's style package is vendored under `tools/governance/vale/styles/` so it works offline out of the box; run `vale sync` to refresh it after an upstream update.

`.editorconfig-checker.json` disables the indentation/indent-size checks: those two flag any whitespace not aligned to a multiple of the configured `indent_size`, which false-positives on deliberately-indented ASCII diagrams in prose docs (see e.g. the diagram in ADR 0012) and is redundant for source code anyway — Rust and Python indentation correctness is already enforced by `cargo fmt`/`ruff format` in each domain's own lint step. The other checks (trailing whitespace, final newline, charset) stay on for every file type.

## Branching Model

Name your branches using the following prefixes:

- `feature/` for new capabilities (e.g., `feature/gps-ingestion`)
- `bugfix/` for bug fixes (e.g., `bugfix/connection-leak`)
- `refactor/` for code restructuring (e.g., `refactor/logger-interface`)
- `docs/` for documentation updates (e.g., `docs/add-adr-0005`)
- `chore/` for build or maintenance tasks (e.g., `chore/bump-turbo`)

## Commit Messages

We enforce the **Conventional Commits** specification, checked locally by the Conform `commit-msg` hook and again in CI (`.conform.yaml` is the source of truth for the enforced policy):

```text
<type>(<scope>): <description>

[optional body]
```

Scopes are free-form (not restricted to a fixed list) — per ADR 0013, nothing at root should need editing just because a domain was added or renamed. A scope naming the domain or area touched (e.g. `ingestion`, `gtfs_s`, `ci`) is encouraged but not enforced beyond basic format.

### Allowed Types

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation changes
- `style`: Changes that do not affect code logic (formatting, whitespace)
- `refactor`: A code change that neither fixes a bug nor adds a feature
- `perf`: A code change that improves performance
- `test`: Adding missing tests or correcting existing tests
- `chore`: Changes to build tooling or auxiliary libraries
- `ci`: Changes to CI configuration or workflows
- `build`: Changes affecting the build system or external dependencies
- `revert`: Reverts a previous commit

### Example

`feat(ingestion): add validation for vehicle coordinate telemetry`

### Developer Certificate of Origin (DCO)

Every commit must be signed off, certifying you have the right to submit the change under this project's license (see [DCO](https://developercertificate.org)):

```bash
git commit -s -m "feat(ingestion): ..."
```

This adds a `Signed-off-by:` trailer, checked by the same Conform hook and in CI.

## Pull Request Guidelines

Before submitting a Pull Request:

1. Ensure the domain(s) you touched pass their own build/lint/test (see Setup Instructions above) — CI runs the same checks per domain.
2. If your PR touches more than one domain's manifests, make sure that's intentional (see the PR template's domain-scope checklist item) — most changes should stay within one domain per ADR 0013.
3. Add appropriate test coverage.
4. Fill out the pull request template completely.
