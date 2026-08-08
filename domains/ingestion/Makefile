# Entry point CI (and contributors) can call generically — see
# .github/workflows/ci.yml, which discovers domains by directory and runs
# `make ci` in each without knowing what language or toolchain lives inside.

.PHONY: ci fmt lint test

ci: fmt lint test

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets

test:
	cargo test --workspace
