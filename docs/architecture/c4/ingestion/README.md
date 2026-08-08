# C4 Diagrams — Static (GTFS-S) Ingestion

C4 model diagrams for the GTFS Static Auto-Downloader (the `ckan` crate in `domains/ingestion/`), as designed in
[`docs/design/gtfs-static-auto-downloader.md`](../../../design/gtfs-static-auto-downloader.md) and implemented per
that document's §§1–7.

Scope: **static** ingestion only (nationwide GTFS-S "Fahrplan" snapshots via the CKAN catalog). Real-time ingestion
(`domains/ingestion/realtime`) and service alerts (`domains/ingestion/service-alerts`) are separate systems with
their own diagrams, not covered here.

| Level | File | Shows |
| --- | --- | --- |
| 1. System Context | [`context.md`](./context.md) | The downloader's place among opentransportdata.swiss, the scheduler that triggers it, and the downstream subset pipeline that consumes its output. |
| 2. Container | [`container.md`](./container.md) | The `ckan` binary, the shared `ti-common` library, and the raw snapshot store on disk. |
| 3. Component | [`component.md`](./component.md) | The Rust modules inside the `ckan` crate and how they collaborate. |
| 4. Dynamic | [`dynamic.md`](./dynamic.md) | Sequenced view of one check-and-update run, including the recovery steps that run before any network call. |

## Reading these alongside the design doc

These diagrams are a visual index into the design, not a replacement for it — section references (`§1`, `§6`, etc.)
throughout point back to `docs/design/gtfs-static-auto-downloader.md`, which remains the source of truth for
behavior, failure handling, and the reasoning behind each decision.
