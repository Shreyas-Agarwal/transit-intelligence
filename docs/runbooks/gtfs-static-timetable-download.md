# Runbook: Downloading the Swiss GTFS Static Timetable

## Purpose

Getting a fresh nationwide Swiss GTFS-S ("Fahrplan") snapshot onto disk, validated, and ready for
the transform stage. This used to be a manual process; it's now automated by the `ckan` Rust
crate. This runbook covers running that automation day-to-day: prerequisites, the `mise` tasks,
configuration (including overriding the cutoff date on the fly), and where to look when something
goes wrong.

For the full design rationale, see [Design: GTFS Static Auto-Downloader & Updater](../design/gtfs-static-auto-downloader.md).

---

## Source

Dataset page:

```text
https://data.opentransportdata.swiss/dataset/timetable-2026-gtfs2020
```

Key facts about this dataset:

* Publisher: opentransportdata.swiss (Swiss national open transport data platform), backed by a CKAN catalog.
* Update cadence: **twice per week** ("Zweimal pro Woche"), no updates on Swiss public holidays.
* Each publish is a full nationwide GTFS-S snapshot, not a diff.
* Resource (file) naming convention: `GTFS_FP2026_YYYYMMDD.zip`, e.g. `GTFS_FP2026_20260805.zip`. Older resources (pre ~2025-09) used a hyphenated date suffix (`GTFS_FP2026_2025-09-22.zip`) — the downloader normalizes both forms.
* The downloader talks to the CKAN Action API (`api.opentransportdata.swiss/ckan-api`, `package_show`), not the HTML dataset page — resource ordering/markup on the page isn't a stable contract.

---

## Pipeline overview

Two stages, two tools, run separately:

1. **Extract** (`domains/ingestion/extract/ckan`, Rust) — detects new upstream versions, downloads
   and archive-validates each one, converts every CSV member to Parquet, and publishes it to
   `data/bronze/static/<version>/` with a `latest` symlink. See
   [the design doc](../design/gtfs-static-auto-downloader.md) for the full state machine.
2. **Transform** (`domains/ingestion/transform`, Python + Polars) — validates a Bronze snapshot's
   Parquet tables (required columns, row-count sanity, referential integrity),
   then derives the Zurich operational subset (ADR 0011) and publishes it to
   `data/silver/static/<version>/`, with its own `latest` symlink.

```text
data/bronze/static/
  20260729/
    stops.parquet
    trips.parquet
    routes.parquet
    stop_times.parquet
    ...
    .snapshot-meta.json
  20260805/
    ...
  latest -> 20260805/
  .manifest.json
```

No CSV ever lands here — the extractor converts to Parquet and deletes the CSVs before publishing
a snapshot (design doc §6.5).

---

## Prerequisites

* `mise` installed and trusted for this repo (`mise trust` if prompted) — it provisions `rust`, `uv`, and everything else via `mise.toml`.
* A `.env` at the repo root (copy from `.env.example`) with, at minimum:

  ```bash
  GTFS_S_CKAN_DATASET_ID=timetable-2026-gtfs2020
  GTFS_S_CKAN_API_TOKEN=...
  GTFS_S_CKAN_API_TOKEN_HASH=...
  ```

  Obtain the token/hash pair from <https://opentransportdata.swiss/en/dev-api/> — same
  opentransportdata.swiss application as `GTFS_RT_API_TOKEN`, but a distinct credential pair
  scoped to the CKAN API.

---

## Running the downloader

```bash
mise run ingestion:gtfs-static
```

This runs `cargo run --release -p ckan` from `domains/ingestion/extract` (see `mise.toml`). It's
safe to re-run on a schedule or by hand — idempotent, self-healing after a crash (design doc §12).

On success, check:

```bash
readlink data/bronze/static/latest
cat data/bronze/static/.manifest.json
```

---

## Configuration

All variables below are read from the environment (`.env` is loaded automatically); the full
annotated list lives in `.env.example`. The ones you're most likely to touch day-to-day:

| Variable | Default | Purpose |
| --- | --- | --- |
| `GTFS_S_CUTOFF_VERSION` | `20260101` | Ignore upstream versions older than this (`YYYYMMDD`), as if never published. Empty string disables the cutoff entirely. |
| `GTFS_S_RAW_DIR` | `<repo_root>/data/bronze/static` | Where snapshots are written. Override to point at a different disk/location (e.g. for a test run). |
| `GTFS_S_CKAN_API_URL` | `https://api.opentransportdata.swiss/ckan-api` | CKAN Action API base URL. |
| `GTFS_S_DOWNLOAD_REQUEST_TIMEOUT_SECS` | `1800` | Per-download timeout — archives are hundreds of MB. |

### Setting the cutoff date on the fly

`GTFS_S_CUTOFF_VERSION` is read fresh from the environment on every run — there's no need to edit
`.env` for a one-off. Export it inline for a single invocation:

```bash
GTFS_S_CUTOFF_VERSION=20260801 mise run ingestion:gtfs-static
```

This tells the downloader to ignore every upstream version published before 2026-08-01, so a first
run doesn't backfill the entire CKAN catalog's history. To make a new cutoff stick across future
runs (not just this one invocation), update `GTFS_S_CUTOFF_VERSION` in `.env` instead.

Two things worth knowing about how the cutoff interacts with what's already on disk:

* The cutoff only affects **discovery** — it never deletes or retroactively rejects a snapshot
  that's already installed under `data/bronze/static/`.
* Raising the cutoff after some older snapshots are already installed doesn't remove them; it just
  stops the downloader from reaching further back on future runs. If you want a clean slate at the
  new cutoff, remove the old snapshot directories (and rebuild `.manifest.json`, which regenerates
  automatically from the remaining sidecars on the next run) yourself first.

---

## Running the transform (validate + subset) stage

Once at least one Bronze snapshot is on disk, validate it and derive the Zurich Silver subset
(ADR 0011) via the `mise` task:

```bash
mise run ingestion:transform           # mode defaults to `latest` — the `latest` Bronze snapshot only
mise run ingestion:transform latest    # same, explicit
mise run ingestion:transform replay    # every retained Bronze snapshot, oldest first
```

This runs `uv run python -m ingestion.transform <mode>` from `domains/ingestion/transform` (see
`mise.toml`). Equivalent without `mise`:

```bash
cd domains/ingestion/transform
uv sync
uv run python -m ingestion.transform latest
uv run python -m ingestion.transform replay
```

For each snapshot processed: Bronze's Parquet tables are validated (missing column, out-of-range
row count, orphaned foreign key, out-of-bounds coordinate — logged per failing check); a snapshot
that fails validation gets **no** Silver output. A snapshot that passes gets the Zurich
subset/derived tables written to `data/silver/static/<version>/`, with `data/silver/static/latest`
advanced to match — same directory-per-version + `latest`-symlink convention as Bronze. Exit code
is non-zero if any processed snapshot failed validation.

See `domains/ingestion/transform/README.md` for the full artifact list and the Python API
equivalent (`from ingestion import transform; transform.run(mode=...)`).

---

## Troubleshooting

* **`latest` symlink and `.manifest.json` disagree** — the downloader asserts this on every run
  and refuses to guess which side is right (design doc §12); it's a bug, not a state to work
  around by hand. File an issue rather than manually repointing the symlink.
* **A version keeps failing** — check `data/bronze/static/.manifest.json` for its recorded status;
  a `failed` version never gets a directory of its own (design doc §4), so it's always safe to
  just re-run the downloader once the underlying issue (usually a transient network error) clears.
* **Stale lock file** (`data/bronze/static/.updater.lock`) after a crash — self-heals on the next
  run (design doc §11); no manual cleanup needed.

---

## Related documents

* [ADR 0011 — GTFS Static Preprocessing and Zurich Operational Subset Strategy](../adr/0011-gtfs-static-preprocessing-and-zurich-subset-strategy.md) — the row-count/subset figures the transform stage's validation checks are bounded against.
* [Design: GTFS Static Auto-Downloader & Updater](../design/gtfs-static-auto-downloader.md) — full design and rationale for the extract stage.
