# C4 — Dynamic: One Poll Cycle

Mermaid has no native C4 "Dynamic" diagram type, so this is a sequence diagram — the standard stand-in for a C4
dynamic view. It traces `realtime run`'s startup plus one iteration of its poll loop, in the order the code
actually executes (`main.rs::run` / `main.rs::poll_and_publish`).

```mermaid
sequenceDiagram
    participant CLI as realtime (main)
    participant Producer as RedpandaProducer
    participant Fetcher
    participant Decoder
    participant Model
    participant Feed as GTFS-RT feed
    participant Redpanda

    Note over CLI,Redpanda: Startup — once per process lifetime
    CLI->>Producer: connect(brokers, client_id)
    Producer->>Redpanda: list_topics()
    Redpanda-->>Producer: existing topic names
    loop for each of the 4 Phase 1 topics missing
        Producer->>Redpanda: create_topic(name, partitions, replication)
    end

    loop every poll_interval (default 30s), forever
        Note over CLI,Redpanda: One poll cycle (errors logged, loop continues — never crashes)
        CLI->>Fetcher: fetch_feed_buffer(feed_url, api_token)
        Fetcher->>Feed: GET (Bearer token attached iff host is opentransportdata.swiss)
        Feed-->>Fetcher: protobuf bytes
        Fetcher-->>CLI: buffer

        CLI->>Decoder: decode_feed_buffer(buffer)
        Decoder-->>CLI: FeedMessage{header, entity[]}

        CLI->>Model: build_messages(entities, feed_timestamp, feed_version, ingestion_timestamp)
        Model->>Model: drop entities where is_deleted == true
        Model->>Model: per entity — derive entity_type, derive_key, convert payload
        Model-->>CLI: Vec<KafkaMessage>{key, value}

        CLI->>Producer: publish(SNAPSHOTS_RAW, messages)
        loop per batch of 100
            Producer->>Redpanda: produce(records)
        end
        Producer-->>CLI: Ok

        CLI->>CLI: log fetch_ms / decode_ms / publish_ms / published_messages
        CLI->>CLI: sleep(poll_interval)
    end
```

## Notes

* The startup topic-creation block runs exactly once, before the loop starts — every subsequent cycle assumes
  the topics already exist and goes straight to fetch/decode/publish.
* A failure at any step inside `poll_and_publish` (feed timeout, malformed protobuf, broker unreachable) is
  logged and the loop proceeds to `sleep` and the next cycle — there's no retry-within-a-cycle and no
  backoff-on-repeated-failure; the next scheduled poll is the retry.
* `feed_timestamp` (from the feed provider's header) and `ingestion_timestamp` (sampled once per cycle, not once
  per entity) are both attached to every message in the batch — this is what lets a downstream consumer measure
  provider-to-ledger latency later, per `docs/design/redpanda-topic-configuration.md`.
* `derive_key` is what actually determines Redpanda partition-key locality per entity: `vehicle.<id>` for
  positions, `trip.<id>` for trip updates, `alert.<sha256[0:12]>` for alerts (whose upstream ids aren't
  guaranteed stable) — all moot for ordering today since every topic has exactly 1 partition (ADR 0008 defers
  partitioning until replay patterns are validated).
