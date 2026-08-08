# C4 — System Context: GTFS-S Auto-Downloader

Where the downloader sits relative to opentransportdata.swiss, whatever triggers it, and the pipeline that
consumes what it publishes. Scheduling mechanism is intentionally undecided by the design (§10: cron, GitHub
Actions, or a script hook are all candidates) — shown here as a generic external trigger.

```mermaid
C4Context
    title System Context — GTFS-S Auto-Downloader

    System_Ext(scheduler, "Scheduler", "cron / GitHub Actions / script hook (mechanism left open, design §10). Fires roughly daily.")

    System(downloader, "GTFS-S Auto-Downloader", "Rust (ckan crate). Detects, downloads, verifies, and publishes new GTFS-S snapshots.")

    System_Ext(ckan_api, "opentransportdata.swiss CKAN API", "api.opentransportdata.swiss/ckan-api — package_show endpoint lists available GTFS-S resources.")
    System_Ext(resource_host, "opentransportdata.swiss resource host", "data.opentransportdata.swiss — serves the actual GTFS-S zip archives.")

    System_Ext(subset_pipeline, "GTFS-S Subset Pipeline", "Python + DuckDB/SQLMesh (domains/gtfs_s/scripts). Consumes raw/latest; not triggered by the downloader (non-goal).")

    Rel(scheduler, downloader, "Triggers one check-and-update run")
    Rel(downloader, ckan_api, "Lists GTFS-S resources", "HTTPS/JSON, Bearer token")
    Rel(downloader, resource_host, "Downloads GTFS-S zip archives", "HTTPS")
    Rel(subset_pipeline, downloader, "Reads data/bronze/static/latest whenever it next runs (manually, for now)", "Filesystem")
```

## Notes

* The downloader talks to **two distinct external hosts** under the same publisher, not one — this distinction
  matters because only the CKAN API call is authenticated (§1); the zip download itself is a plain, unauthenticated
  HTTPS GET (§3 field notes).
* The relationship to the subset pipeline is deliberately one-directional and pull-based: the downloader's job
  ends at "raw data is on disk and `latest` points at it" (design doc, Non-goals). It never calls the subset
  pipeline; the subset pipeline reads `raw/latest` whenever it happens to run next.
