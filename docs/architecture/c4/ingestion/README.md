# C4 Diagrams — Ingestion

C4 model diagrams for `domains/ingestion/`, a Cargo workspace with three independent Rust crates. Both crates
covered here publish onto the same Redpanda ledger (ADR 0008) but share nothing else at runtime — each gets its
own full C4 stack rather than one merged diagram set.

| Crate | Design doc | Scope |
| --- | --- | --- |
| `ckan` (static) | [`docs/design/gtfs-static-auto-downloader.md`](../../../design/gtfs-static-auto-downloader.md) | Nationwide GTFS-S "Fahrplan" snapshots via the CKAN catalog — one-shot, externally scheduled. |
| `realtime` | ADR [0007](../../adr/0007-ingest-swiss-gtfs-rt-datasets-via-30s-polling.md) / [0008](../../adr/008-redpanda-as-immutable-snapshot-ledger.md) | Combined GTFS-RT (VehiclePosition/TripUpdate/Alert) feed — long-lived, self-scheduled 30s poll loop. |

`service-alerts` is a separate system with its own diagrams, not covered here.

## Static (GTFS-S) Ingestion

| Level | File | Shows |
| --- | --- | --- |
| 1. System Context | [`static-context.md`](./static-context.md) | The downloader's place among opentransportdata.swiss, the scheduler that triggers it, and the downstream subset pipeline that consumes its output. |
| 2. Container | [`static-container.md`](./static-container.md) | The `ckan` binary, the shared `ti-common` library, and the raw snapshot store on disk. |
| 3. Component | [`static-component.md`](./static-component.md) | The Rust modules inside the `ckan` crate and how they collaborate. |
| 4. Dynamic | [`static-dynamic.md`](./static-dynamic.md) | Sequenced view of one check-and-update run, including the recovery steps that run before any network call. |

## Realtime (GTFS-RT) Ingestion

Now also Rust (the `realtime` crate) — no external scheduler; the binary drives its own poll loop.

| Level | File | Shows |
| --- | --- | --- |
| 1. System Context | [`realtime-context.md`](./realtime-context.md) | The worker's place between the GTFS-RT feed and Redpanda, and that nothing downstream consumes the ledger yet. |
| 2. Container | [`realtime-container.md`](./realtime-container.md) | The `realtime` binary, the shared `ti-common` library, and the Redpanda broker it produces to. |
| 3. Component | [`realtime-component.md`](./realtime-component.md) | The Rust modules inside the `realtime` crate — fetch, decode, model, produce — and how they collaborate. |
| 4. Dynamic | [`realtime-dynamic.md`](./realtime-dynamic.md) | Sequenced view of startup topic creation plus one poll cycle. |

## Reading these alongside the design docs

These diagrams are a visual index, not a replacement for the design doc / ADRs — section references (`§1`,
`§6`, ADR numbers, etc.) throughout point back to the source of truth for behavior, failure handling, and the
reasoning behind each decision.
