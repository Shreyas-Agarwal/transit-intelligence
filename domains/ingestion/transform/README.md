# ingestion.transform

Bronze → Silver transform pipeline for GTFS static snapshots. Plain Python +
Polars — no DuckDB, no SQLMesh; this stage doesn't need a DAG engine or a
tabular query planner, just per-snapshot dataframe checks and Parquet writes.

## What this is

One transformation pipeline (`transform_snapshot`, in `pipeline.py`), run
against whichever snapshots an execution mode supplies (`snapshots.py`):

* **`latest`** (default) — the single Bronze snapshot `data/bronze/static/latest`
  points at. Normal operation.
* **`replay`** — every retained Bronze snapshot, oldest first. Historical
  reconstruction / static-evolution analysis.

`transform_snapshot` doesn't know which mode invoked it — mode only decides
*which* snapshots get supplied, never *how* a snapshot is transformed. Adding
a new execution policy (a specific version, a range, since-version, ...) means
adding a new iterator, not touching the pipeline.

For each snapshot, `transform_snapshot`:

1. Loads every Parquet table the Bronze snapshot contains (`pipeline.py`).
2. Validates them — design doc Tier 2: required columns, row-count sanity,
   referential integrity (`validate.py`). A snapshot that
   fails validation stops here: no Silver output is written for it.
3. Derives the Zurich operational subset per ADR 0011 (`subset.py` — ported
   from `domains/gtfs_s/scripts/transit_subset/subset_builder.py`, which is
   unchanged and still there) and publishes every artifact to
   `data/silver/static/<version>/`, advancing `latest` (`silver_paths.py`) —
   the same directory-per-version + `latest`-symlink convention Bronze uses.

It has no knowledge of loading, SCD Type 2 persistence, PostgreSQL, or any
other downstream consumer — that's out of scope for this stage.

### Silver artifacts

Same flat, one-file-per-table convention as Bronze, just filtered/derived:

```text
stops.parquet               trip_ids.parquet
trips.parquet                internal_trips.parquet      crossing_trips.parquet
routes.parquet               internal_routes.parquet     crossing_routes.parquet   mixed_routes.parquet
stop_times.parquet           internal_stop_times.parquet crossing_stop_times.parquet
calendar.parquet*            calendar_dates.parquet*     agencies.parquet*         frequencies.parquet*
```

\* only written if the corresponding optional GTFS table was present in the Bronze snapshot.

## Running it

```bash
uv sync

uv run python -m ingestion.transform            # latest, default
uv run python -m ingestion.transform latest
uv run python -m ingestion.transform replay
```

Exit code is non-zero if any processed snapshot failed validation.

## Python API

```python
from ingestion import transform

results = transform.run(mode="latest")   # or mode="replay"
for r in results:
    if not r.validation.passed:
        print(r.snapshot.version, r.validation.failures)
    else:
        print(r.snapshot.version, "->", r.silver_path, r.artifact_row_counts)
```

## Configuration

* `GTFS_S_RAW_DIR` — where Bronze snapshots are read from. Defaults to
  `<repo_root>/data/bronze/static`, same as the Rust `ckan` downloader
  (`domains/ingestion/extract/ckan/src/config.rs`).
* `GTFS_S_SILVER_DIR` — where Silver snapshots are written. Defaults to
  `<repo_root>/data/silver/static`.
