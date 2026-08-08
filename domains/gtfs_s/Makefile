# Entry point CI (and contributors) can call generically — see
# .github/workflows/ci.yml, which discovers domains by directory and runs
# `make ci` in each without knowing what language or toolchain lives inside.

.PHONY: ci sync lint typecheck test

ci: sync lint typecheck

sync:
	uv sync

lint: sync
	uv run ruff check .

# Non-blocking: 2 pre-existing findings in scripts/tests/ predate CI wiring
# for this domain and aren't fixed here (see commit history). New findings
# are still visible in the job output; they just don't fail it.
typecheck: sync
	-uv run pyright

# Not wired into `ci`: transit_subset's tests depend on a downloaded GTFS-S
# snapshot under domains/gtfs_s/raw (see scripts/README.md) that CI has no
# step to fetch yet.
test: sync
	cd scripts && uv run --project .. python -m pytest
