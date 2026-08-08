# Entry point CI (and contributors) can call generically — see
# .github/workflows/ci.yml, which discovers domains by directory and runs
# `make ci` in each without knowing what language or toolchain lives inside.
#
# The Rust workspace lives in extract/ (rust-toolchain.toml stays at this
# level so it still applies to that workspace).

.PHONY: ci fmt lint test

ci: fmt lint test

fmt:
	cargo fmt --all --manifest-path extract/Cargo.toml -- --check

lint:
	cargo clippy --workspace --all-targets --manifest-path extract/Cargo.toml

test:
	cargo test --workspace --manifest-path extract/Cargo.toml
