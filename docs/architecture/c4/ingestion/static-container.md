# C4 — Container: GTFS-S Auto-Downloader

Inside the "GTFS-S Auto-Downloader" system boundary: the binary itself, the shared library it's built on, and the
on-disk store it reads and writes. `domains/ingestion` is a Cargo workspace shared with `realtime` and
`service-alerts` (see the design doc's "Language & tooling boundary" section); this diagram shows only the
members involved in static ingestion.

```mermaid
C4Container
    title Container — GTFS-S Auto-Downloader

    System_Ext(scheduler, "Scheduler", "cron / GitHub Actions / script hook")
    System_Ext(ckan_api, "opentransportdata.swiss CKAN API", "package_show endpoint")
    System_Ext(resource_host, "opentransportdata.swiss resource host", "serves GTFS-S zip archives")
    System_Ext(subset_pipeline, "GTFS-S Subset Pipeline", "Python + DuckDB/SQLMesh, downstream, not automatically triggered")

    Container_Boundary(downloader, "GTFS-S Auto-Downloader") {
        Container(ckan_bin, "ckan", "Rust binary (Tokio async runtime)", "One-shot CLI: recovery/reconciliation, then a bounded queue of workers each running claim/download/verify/extract/convert/publish/complete, bounded by both a worker-pool limit and independent per-resource (network vs. CPU/disk) limits, then advance latest.")
        Container(ti_common, "ti-common", "Rust library", "Shared across all domains/ingestion crates: env config loading, HTTP client construction, bearer-token auth, retry/backoff, and observability bootstrap (tracing setup, OpenTelemetry export).")
    }

    ContainerDb(raw_store, "Raw Snapshot Store", "Local filesystem — data/bronze/static/", "Per-version snapshot directories containing Parquet files (one per GTFS member) + .snapshot-meta.json sidecars, .manifest.json rollup, latest symlink, .updater.lock, .work/ durable per-version state, .staging/ scratch space. CSV never lands here — only in .staging/ during conversion.")

    Rel(scheduler, ckan_bin, "Invokes")
    Rel(ckan_bin, ti_common, "Uses for config, HTTP client, auth, retry, observability")
    Rel(ckan_bin, ckan_api, "package_show", "HTTPS/JSON, Bearer token")
    Rel(ckan_bin, resource_host, "Streams zip download", "HTTPS")
    Rel(ckan_bin, raw_store, "Reads sidecars/manifest/latest/work-state on startup; writes snapshots, sidecars, manifest, latest symlink, lock file, work-state")
    Rel(subset_pipeline, raw_store, "Reads raw/latest", "Filesystem")
```

## Notes

* `ti-common` is a plain library dependency, not a separate running process — it's shown as its own container
  because it's a distinct, independently-versioned unit of code shared with `realtime`/`service-alerts`, not
  because it runs standalone.
* The Raw Snapshot Store is filesystem, not a database, but is modelled as a `ContainerDb` because it plays the
  same architectural role here: it's the durable state the system reads on startup and writes as its output. Per
  the design doc, the filesystem (specifically the sidecars) is authoritative for what's *installed* — the manifest
  is a derived, rebuildable cache over it, never the other way around. `.work/` is a second, independent durable
  record — of what's *supposed to happen* to each version — that the filesystem's installed-state can override but
  never the reverse (see [DD-001 §Durable Work State & Reconciliation](../../design/DD-001-gtfs-static-downloader.md#durable-work-state--reconciliation)).
* This container-level view is deliberately unchanged in shape from V1: reconciliation, the work queue, resource
  permits, and observability are all internal to how `ckan_bin` does its job, not new containers or new external
  dependencies. See [static-component.md](static-component.md) for where that internal structure actually lives.
