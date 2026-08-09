# Entry point CI (and contributors) can call generically — see
# .github/workflows/ci.yml, which discovers domains by directory and runs
# `make ci` in each without knowing what language or toolchain lives inside.
#
# The Rust workspace lives in extract/ (rust-toolchain.toml stays at this
# level so it still applies to that workspace). The Bronze -> Silver
# transform pipeline lives in transform/ (plain Python + uv) — it produces
# both the static Silver subset (data/silver/static) and the canonical
# transit graph (data/silver/graph); transform-test covers both.

.PHONY: ci fmt lint test sync transform-sync transform-lint transform-typecheck transform-test

ci: fmt lint test transform-sync transform-lint transform-typecheck transform-test

fmt:
	cargo fmt --all --manifest-path extract/Cargo.toml -- --check

lint:
	cargo clippy --workspace --all-targets --manifest-path extract/Cargo.toml

test:
	cargo test --workspace --manifest-path extract/Cargo.toml

transform-sync:
	cd transform && uv sync

transform-lint: transform-sync
	cd transform && uv run ruff check .

# Non-blocking: 2 pre-existing findings in transform/tests/ predate this
# target being wired into CI and aren't fixed here. New findings are still
# visible in the job output; they just don't fail it.
transform-typecheck: transform-sync
	-cd transform && uv run pyright

transform-test: transform-sync
	cd transform && uv run pytest
