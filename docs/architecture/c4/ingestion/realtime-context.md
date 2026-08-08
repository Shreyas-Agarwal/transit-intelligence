# C4 — System Context: GTFS-RT Realtime Worker

Where the realtime worker sits relative to the opentransportdata.swiss GTFS-RT feed and the Redpanda ledger it
publishes into. Unlike the static downloader, there is no external scheduler here — `realtime run` is a
long-lived process that drives its own poll loop (fetch → decode → publish → sleep) at a fixed interval
(ADR 0007: 20–30s upstream update cadence; ADR 0008: snapshot-ledger model).

```mermaid
C4Context
    title System Context — GTFS-RT Realtime Worker

    System(realtime_worker, "GTFS-RT Realtime Worker", "Rust (realtime crate). Self-driven poll loop: fetches, decodes, and publishes GTFS-RT snapshots every 30s.")

    System_Ext(feed_host, "opentransportdata.swiss GTFS-RT feed", "Combined VehiclePosition/TripUpdate/Alert protobuf feed (FULL_DATASET), refreshed upstream every 20-30s.")
    System_Ext(redpanda, "Redpanda", "Immutable snapshot ledger (ADR 0008). Broker the worker publishes into and ensures topics on.")

    System_Ext(future_consumers, "Future analytical consumers", "Delay propagation, temporal routing, replay, visualization — not yet built (ADR 0008, Consumer Separation). Nothing subscribes to the ledger today.")

    Rel(realtime_worker, feed_host, "Polls combined feed every 30s", "HTTPS/Protobuf, Bearer token")
    Rel(realtime_worker, redpanda, "Publishes decoded entities as SnapshotRawMessage envelopes; ensures topics exist on startup", "Kafka protocol (rskafka)")
    Rel(future_consumers, redpanda, "Will subscribe to transit.snapshots.raw (not implemented yet)", "Kafka protocol")
```

## Notes

* Exactly one external upstream host appears here (`feed_host`), unlike the static downloader's two — GTFS-RT has
  no separate catalog/listing API; the feed URL itself is the resource.
* The bearer token is only attached when the request target is an `opentransportdata.swiss` host
  (`fetcher.rs`) — the same auth-scoping precaution as the static downloader, defending against token leakage
  through a redirect to a third party.
* `future_consumers` is drawn to make an easy mistake visible: the ledger being populated does **not** mean
  anything downstream exists yet. Only `transit.snapshots.raw` has a producer; `transit.snapshots.normalized`,
  `transit.state.deltas`, and `transit.metrics.operational` are reserved topic names with no writer or reader
  (`docs/design/redpanda-topic-configuration.md`).
