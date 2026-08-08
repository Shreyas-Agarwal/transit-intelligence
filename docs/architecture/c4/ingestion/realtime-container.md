# C4 — Container: GTFS-RT Realtime Worker

Inside the "GTFS-RT Realtime Worker" system boundary: the binary itself, the shared library it's built on, and
the Redpanda broker it produces to. `domains/ingestion` is a Cargo workspace shared with `ckan` and
`service-alerts` (see the static downloader's container diagram for the workspace-sharing rationale); this
diagram shows only the members involved in realtime ingestion.

```mermaid
C4Container
    title Container — GTFS-RT Realtime Worker

    System_Ext(feed_host, "opentransportdata.swiss GTFS-RT feed", "combined VehiclePosition/TripUpdate/Alert protobuf feed")
    System_Ext(future_consumers, "Future analytical consumers", "not implemented yet")

    Container_Boundary(realtime_worker, "GTFS-RT Realtime Worker") {
        Container(realtime_bin, "realtime", "Rust binary (Tokio async runtime)", "`run`: infinite poll loop, fetch/decode/publish, sleeps poll_interval, logs and continues past per-cycle errors. `explore`: one-shot fetch+decode diagnostic, writes feed-exploration-output.json.")
        Container(ti_common, "ti-common", "Rust library", "Shared across all domains/ingestion crates: env config loading, HTTP client construction, bearer-token auth, retry/backoff, tracing setup.")
    }

    ContainerDb(redpanda, "Redpanda broker", "Kafka-protocol event ledger", "transit.snapshots.raw (populated) + transit.snapshots.normalized / transit.state.deltas / transit.metrics.operational (reserved, unpopulated). 1 partition, 1 replica each — Phase 1 (ADR 0008).")

    Rel(realtime_bin, ti_common, "Uses for config loading, HTTP client construction, logging")
    Rel(realtime_bin, feed_host, "GET combined feed", "HTTPS, Bearer token")
    Rel(realtime_bin, redpanda, "ensure_topics() on startup; publish() SnapshotRawMessage batches keyed by vehicle/trip/alert id", "Kafka protocol (rskafka)")
    Rel(future_consumers, redpanda, "Will subscribe (not implemented yet)", "Kafka protocol")
```

## Notes

* `realtime_bin` talks to Redpanda directly via `rskafka` — a pure-Rust Kafka client chosen specifically so the
  workspace avoids a `librdkafka`/`cmake` native toolchain dependency (`producer.rs`). No separate
  broker-client container exists; the producer logic lives inside the binary's library crate.
* `ensure_topics()` runs once at startup and is idempotent: it creates whichever of the four Phase 1 topics are
  missing, but only sets partition count and replication factor at creation time — retention (`retention.ms`)
  is still set out-of-band via `rpk topic alter-config` per the runbook, since `rskafka`'s `create_topic` doesn't
  accept broker-side config entries.
* The poll loop treats a failed cycle (feed timeout, decode error, publish failure) as non-fatal: it logs and
  sleeps to the next cycle rather than crashing the process (`main.rs::run`) — there is deliberately no
  circuit-breaker or backoff-on-failure here yet.
