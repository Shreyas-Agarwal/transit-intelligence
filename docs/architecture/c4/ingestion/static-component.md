# C4 — Component: `ckan` crate

The Rust modules inside `domains/ingestion/extract/ckan/src/` and how they collaborate. `main.rs` is a thin wrapper —
essentially all logic lives in the library (`lib.rs`) so it's exercisable from integration tests
(`domains/ingestion/extract/ckan/tests/`).

```mermaid
C4Component
    title Component — ckan crate

    Container_Boundary(ckan_bin, "ckan") {
        Component(main, "main", "main.rs", "Parses CLI args, loads config, builds HTTP clients, calls pipeline::run.")
        Component(pipeline, "pipeline", "pipeline.rs", "Orchestrates the state machine: recovery checks, then discover → download → verify → extract → validate → convert → publish per pending version, then advance latest.")
        Component(ckan_client, "ckan_client", "ckan_client.rs", "CKAN Action API client. Calls package_show, parses the resources array into UpstreamResource values.")
        Component(download, "download", "download.rs", "Streams a zip to staging, hashing as it goes; verifies byte count against Content-Length.")
        Component(archive, "archive", "archive.rs", "Tier 1 (archive-level) validation: CRC32 integrity, required GTFS member presence/non-zero-size; extraction to CSV staging.")
        Component(parquet_convert, "parquet_convert", "parquet_convert.rs", "Converts every extracted *.txt member to a same-named *.parquet file (all columns as Utf8 — bytes-format conversion, not GTFS-typed reasoning).")
        Component(manifest, "manifest", "manifest.rs", "Per-snapshot sidecar read/write; rollup manifest rebuild from sidecars (never trusted from disk as ground truth).")
        Component(lock, "lock", "lock.rs", "Exclusive-create updater lock with dead-PID staleness detection and RAII release.")
        Component(symlink, "symlink", "symlink.rs", "Atomic latest symlink advance/read via symlink+rename (the ln -sfn equivalent).")
        Component(paths, "paths", "paths.rs", "RawLayout — the single source of truth for every path convention under raw/, including both the CSV and Parquet staging directories.")
        Component(domain, "domain", "domain.rs", "VersionId, UpstreamResource, filename parsing/normalization, upstream hash verification.")
        Component(config, "config", "config.rs", "CkanConfig — loads all GTFS_S_CKAN_*/GTFS_S_* environment variables.")
    }

    Container(ti_common, "ti-common", "Rust library", "config/http/auth/retry/logging primitives")
    System_Ext(ckan_api, "opentransportdata.swiss CKAN API", "")
    System_Ext(resource_host, "opentransportdata.swiss resource host", "")
    ContainerDb(raw_store, "Raw Snapshot Store", "data/bronze/static/", "Persisted snapshots are Parquet only — CSV never lands here, only in staging.")

    Rel(main, config, "Loads config from")
    Rel(main, pipeline, "Calls run()")
    Rel(main, ckan_client, "Constructs")
    Rel(main, ti_common, "Builds HTTP clients via")

    Rel(pipeline, lock, "Acquires/releases")
    Rel(pipeline, manifest, "Scans sidecars, rebuilds & writes manifest")
    Rel(pipeline, ckan_client, "Lists upstream resources")
    Rel(pipeline, download, "Downloads each pending version")
    Rel(pipeline, archive, "Validates & extracts each download to CSV staging")
    Rel(pipeline, parquet_convert, "Converts CSV staging to Parquet staging after Tier 1 passes")
    Rel(pipeline, symlink, "Advances / reads latest")
    Rel(pipeline, paths, "Resolves staging/final/sidecar/lock/latest paths")
    Rel(pipeline, domain, "Verifies upstream hash, orders versions")

    Rel(ckan_client, domain, "Parses filenames into VersionId/UpstreamResource")
    Rel(ckan_client, ti_common, "Auth + retry")
    Rel(ckan_client, ckan_api, "HTTPS/JSON")
    Rel(download, resource_host, "HTTPS stream")
    Rel(lock, raw_store, "Reads/writes .updater.lock")
    Rel(manifest, raw_store, "Reads/writes sidecars + .manifest.json")
    Rel(symlink, raw_store, "Reads/writes latest")
    Rel(archive, raw_store, "Writes extracted CSVs into CSV staging (never the final location)")
    Rel(parquet_convert, raw_store, "Writes converted Parquet files into Parquet staging (never the final location)")
```

## Notes

* `pipeline` is the only module that depends on nearly everything else — it's the state machine described in the
  design doc's pipeline overview diagram, and every arrow leaving it corresponds to one box in that diagram.
* `domain` and `paths` have no I/O of their own; they're pure logic/convention modules that everything else
  depends on, which is why they sit at the bottom with no outgoing arrows to external systems.
* `archive` writes into the *CSV staging* extract directory, and `parquet_convert` writes into a *separate* Parquet
  staging directory — neither writes directly into the raw store's final location. The atomic rename that publishes
  a snapshot (of the Parquet staging directory, not the CSV one) happens in `pipeline`, not in either of these
  modules (design doc §5, §6.5).
* `parquet_convert` only ever runs after `archive`'s Tier 1 checks pass — a structurally-broken archive is never
  converted. Once conversion succeeds, `pipeline` deletes the CSV staging directory; it's never what gets renamed
  into the final snapshot path.
