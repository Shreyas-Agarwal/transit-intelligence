# C4 — Component: `realtime` crate

The Rust modules inside `domains/ingestion/realtime/src/` and how they collaborate. Like `ckan`, `main.rs` is a
thin CLI wrapper — the fetch/decode/model/publish stages live in the library so they're independently testable
without a live feed endpoint or Redpanda broker (`lib.rs`).

```mermaid
C4Component
    title Component — realtime crate

    Container_Boundary(realtime_bin, "realtime") {
        Component(main, "main", "main.rs", "Parses CLI (run/explore), loads config, owns the poll loop and per-cycle timing/logging.")
        Component(config, "config", "config.rs", "RealtimeConfig — loads GTFS_RT_*/REDPANDA_*/KAFKA_* environment variables, including poll_interval (ADR 0007).")
        Component(fetcher, "fetcher", "fetcher.rs", "Fetches the raw protobuf body; attaches the Bearer token only when the target host is opentransportdata.swiss.")
        Component(proto, "proto", "proto.rs + build.rs", "Generated GTFS-RT protobuf types (prost), compiled from the .proto schema in proto/ at build time.")
        Component(decoder, "decoder", "decoder.rs", "Decodes a raw byte buffer into the generated proto::FeedMessage.")
        Component(model, "model", "model.rs", "Converts proto types to a JSON-serializable model; derives entity_type and the Redpanda partition key; builds SnapshotRawMessage envelopes.")
        Component(producer, "producer", "producer.rs", "RedpandaProducer — rskafka client wrapper: ensure_topics(), per-topic partition clients, batched publish().")
        Component(topics, "topics", "topics.rs", "Topic name constants and TOPIC_CONFIGS (partitions/replication/retention) for all four Phase 1 topics.")
    }

    Container(ti_common, "ti-common", "Rust library", "config/http/auth/logging primitives")
    System_Ext(feed_host, "opentransportdata.swiss GTFS-RT feed", "")
    ContainerDb(redpanda, "Redpanda broker", "transit.snapshots.raw + reserved topics", "")

    Rel(main, config, "Loads config from")
    Rel(main, ti_common, "Builds HTTP client via")
    Rel(main, fetcher, "fetch_feed_buffer() each cycle")
    Rel(main, decoder, "decode_feed_buffer() each cycle")
    Rel(main, model, "build_messages() each cycle")
    Rel(main, producer, "connect() once, ensure_topics() once, publish() each cycle")

    Rel(fetcher, feed_host, "HTTPS GET, Bearer token")
    Rel(decoder, proto, "Decodes bytes into proto::FeedMessage")
    Rel(model, proto, "Converts proto::FeedEntity variants into JSON model + envelope")
    Rel(producer, topics, "Reads TOPIC_CONFIGS to create missing topics")
    Rel(producer, redpanda, "Kafka protocol (rskafka)")
```

## Notes

* `proto` has no hand-written logic of its own — it's the `prost`-generated output of `build.rs` compiling the
  GTFS-RT `.proto` schema, kept as its own component because `decoder` and `model` both depend on its generated
  types directly.
* `model` is the only component that makes ingestion-specific decisions: which field wins entity-type priority
  (`vehicle` → `trip_update` → `alert` → `UNKNOWN`), what the Redpanda partition key looks like per entity type,
  and that `is_deleted` entities are dropped before publishing (they only appear in DIFFERENTIAL-mode feeds; the
  Swiss feed is FULL_DATASET). See `docs/design/gtfs-rt-domain-mapping.md`.
* `producer` is the only component that talks to Redpanda — `main` never constructs Kafka records itself, it
  just hands `producer` the `KafkaMessage` list that `model` already built.
