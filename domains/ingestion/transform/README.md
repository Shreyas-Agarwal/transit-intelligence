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
4. Derives the canonical transit graph (`graph.py`, see below) from that
   *same* snapshot's tables and publishes it to
   `data/silver/graph/<version>/`, advancing its own `latest`
   (`silver_paths.py:graph_root`) — same layout, separate root. This step is
   isolated from step 3: if graph construction or publishing fails, the
   already-written static Silver output is unaffected, and
   `data/silver/graph/latest` is simply left pointing at whatever it pointed
   at before (see `TransformResult.graph_path`/`.graph_row_counts`, both
   `None` on graph failure).

It has no knowledge of loading, SCD Type 2 persistence, PostgreSQL, or any
other downstream consumer — that's out of scope for this stage.

### Silver artifacts

Same flat, one-file-per-table convention as Bronze, just filtered/derived —
with one exception: `stops` is a fact table, so it isn't filtered. Every
bronze stop is kept and tagged with a `stop_type` column (`internal` /
`boundary` / `external`, see `subset.py`); `internal_stops` is the actual
Zurich-only subset that `trip_ids`, `trips`, `routes`, etc. are derived from.

```text
stops.parquet                internal_stops.parquet      trip_ids.parquet
trips.parquet                internal_trips.parquet      crossing_trips.parquet
routes.parquet               internal_routes.parquet     crossing_routes.parquet   mixed_routes.parquet
stop_times.parquet           internal_stop_times.parquet crossing_stop_times.parquet
calendar.parquet*            calendar_dates.parquet*     agencies.parquet*         frequencies.parquet*
```

\* only written if the corresponding optional GTFS table was present in the Bronze snapshot.

### Canonical transit graph (v1)

A separate Silver output, rooted at `data/silver/graph/` (not inside
`data/silver/static/`), built by `graph.py` from the same Bronze snapshot's
`trips`/`stop_times` and the classified `stops` artifact above:

```text
data/silver/graph/
├── <version>/
│   ├── nodes.parquet
│   └── edges.parquet
└── latest -> <version>
```

Same versioned-snapshot + `latest`-symlink convention as `data/silver/static/`
(`SilverLayout` is reused as-is, just pointed at a different root —
`graph_root()` in `silver_paths.py`), and always derived from the exact
version currently being processed, never from an independently-resolved
`latest`.

**What it represents.** One node per stop, one directed edge per observed
consecutive stop-to-stop traversal within a GTFS trip:

* **Nodes** — every `internal` and `boundary` stop from the classified
  `stops` artifact (`stop_id`, `stop_name`, `stop_lat`, `stop_lon`,
  `stop_type`). `external` stops are never materialized as nodes.
* **Edges** — directed, one row per distinct `(source_stop_id,
  target_stop_id)` pair seen as a consecutive `stop_sequence` step in any
  trip, aggregated with `route_count` (distinct routes making that exact
  traversal) and `trip_count` (distinct trips). A trip `A → B → C` produces
  `A → B` and `B → C`; it is never collapsed with the reverse trip `C → B →
  A`, which produces its own independent `C → B` and `B → A` edges.

**Why directed.** A transit network isn't symmetric — service frequency,
direction of travel, and even which platform is served can differ between
`A → B` and `B → A`. Collapsing the two into one undirected edge would throw
that away; v1 keeps both so later work (temporal weighting, route-level
abstraction) has the real topology to start from.

**Why bounded, and how `external` is excluded.** `internal` / `boundary` /
`external` are the same three-way stop classification `subset.py` already
computes for the Silver `stops` artifact: `internal` matches the current
operational scope (today: Zurich, by stop-name prefix); `boundary` is a stop
outside that scope but reachable by a trip that also touches an `internal`
stop — where the scoped network meets the rest of the feed; `external` is
everything else. `graph.py` keeps an edge only when **both** its endpoints are
non-`external` — so a trip segment `internal → boundary → external` yields
only the `internal → boundary` edge, and `external → boundary → internal`
yields only `boundary → internal`. This is why edges are derived from the
*full* Bronze `trips`/`stop_times` rather than the Zurich-only
`internal_trips` subset: a trip that enters and leaves the scope multiple
times, or two `boundary` stops linked only by a trip that never itself
touches an `internal` stop, still need every one of their in-scope segments
counted — filtering the trip universe first would silently drop some.

This is deliberately v1: a directed, bounded stop graph meant to be inspected
and validated before station collapsing, transfer modelling, temporal edge
weights, route-level abstractions, or other network analytics are layered on
top of it.

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
        if r.graph_path is not None:
            print(r.snapshot.version, "-> graph ->", r.graph_path, r.graph_row_counts)
```

`graph_path`/`graph_row_counts` are `None` whenever `validation.passed` is
`False` (no snapshot to build a graph from), and also — independently — if
graph construction or publishing itself failed for an otherwise-valid
snapshot; the static Silver fields above are unaffected either way.

## Configuration

* `GTFS_S_RAW_DIR` — where Bronze snapshots are read from. Defaults to
  `<repo_root>/data/bronze/static`, same as the Rust `ckan` downloader
  (`domains/ingestion/extract/ckan/src/config.rs`).
* `GTFS_S_SILVER_DIR` — where Silver static snapshots are written. Defaults
  to `<repo_root>/data/silver/static`.
* `GTFS_S_SILVER_GRAPH_DIR` — where Silver graph snapshots are written.
  Defaults to `<repo_root>/data/silver/graph`.
