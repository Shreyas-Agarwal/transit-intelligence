# Zone 110 Filtering Strategy

## Status

**Recommendation:** Option B — Raw Feed → Redpanda → Downstream Filtering

---

## Context

Zone 110 is the Zürich city zone in the Swiss fare zone system. A recurring architectural question is whether the ingestion pipeline should filter feed entities by zone before publishing to Redpanda, or whether filtering should occur downstream as a consumer concern.

This document evaluates both options and provides a formal recommendation.

---

## The Two Options

### Option A — Pre-Ingest Filtering

```text
Raw Feed (HTTP) → Zone Filter → Redpanda (only Zone 110 entities)
```

The ingestion worker applies a zone predicate before publishing. Only entities associated with Zone 110 (by `stop_id`, `route_id`, or geographic bounding box) are published to Redpanda.

### Option B — Post-Ingest Filtering

```text
Raw Feed (HTTP) → Redpanda (all entities) → Zone Filter Consumer
```

The ingestion worker publishes all entities to Redpanda without any filtering. Downstream consumers (Python analytics workers, future route engines) apply zone filtering independently.

---

## Evaluation

### Option A — Pre-Ingest Filtering

**Advantages:**

- Smaller Redpanda storage footprint.
- Simpler downstream consumers (no zone logic required in each consumer).
- Reduced bandwidth between broker and consumers.

**Disadvantages:**

- **Irreversible information loss.** Once non-Zone-110 entities are filtered at the ingestion layer, they cannot be recovered without re-fetching the live feed. Historical replay of the unfiltered feed is permanently lost.

- **Couples ingestion to a business domain assumption.** The ingestion worker's job is to acquire and preserve data. Zone filtering is a _domain interpretation_, not an acquisition concern. Mixing these concerns violates single-responsibility.

- **Makes zone expansion expensive.** If the platform later needs to expand to Zone 120 (Dietikon), Zone 130 (Regensdorf), or a different region entirely, the ingestion worker must be modified and redeployed — and historical data for those zones will be absent.

- **Zone mapping may be ambiguous.** A vehicle traversing a zone boundary is simultaneously relevant to Zone 110 and an adjacent zone. Pre-filtering requires a policy decision about how to handle transitional entities.

- **Conflicts with ADR 0008's immutable temporal ledger philosophy.** ADR 0008 explicitly defines Redpanda as an _immutable_ and _replayable_ record. Filtering before ingestion undermines the completeness of this record.

---

### Option B — Post-Ingest Filtering (Recommended)

**Advantages:**

- **Full historical replayability.** The complete unfiltered feed is preserved in Redpanda. Any future zone, region, or entity selection can be derived from the stored history without re-polling the live feed.

- **Separation of concerns.** The ingestion worker does one thing: acquire and publish. Zone intelligence belongs to consumers.

- **Flexible consumer evolution.** Different consumers can apply different zone predicates independently. A tram delay consumer might filter to Zone 110. A network resilience consumer might process all zones to model cross-boundary propagation.

- **Consistent with ADR 0008.** The immutable temporal ledger principle explicitly encourages full-snapshot preservation and downstream consumer independence.

- **Aligns with ADR 0010 (polyglot architecture).** Python analytics consumers can apply sophisticated zone mapping (e.g. geographic bounding box using geopandas) that would be complex to implement in the TypeScript ingestion worker.

**Disadvantages:**

- **Higher Redpanda storage volume.** Storing all entities, not just Zone 110, uses more disk space. At the current feed size and 30-second polling intervals, this is manageable.

- **Filtering logic must be implemented in every consumer that cares.** This is mitigated by implementing a shared zone-filter utility in the Python analytics package.

---

## Storage and Replay Implications

### Storage Estimate

Assuming a typical Swiss GTFS-RT combined feed:

| Metric                    | Estimate                |
| ------------------------- | ----------------------- |
| Entity count per snapshot | ~1,000–5,000 entities   |
| JSON payload per entity   | ~0.5–2 KB               |
| Snapshot payload size     | ~1–10 MB per poll       |
| Poll frequency            | 30 seconds              |
| Messages per day          | ~2,880                  |
| Daily storage in Redpanda | ~3–30 GB (uncompressed) |
| With 7-day retention      | ~21–210 GB              |

Redpanda supports log compression (snappy, lz4, zstd) which typically reduces JSON payloads by 60–80%. Effective storage with 7-day retention is likely in the **5–40 GB** range for a local development setup.

If storage becomes a constraint, the first intervention should be **reducing retention** (e.g. to 3 days), not pre-filtering. This preserves the architectural flexibility of Option B while controlling cost.

### Replay Implications

Option B enables deterministic historical replay for:

- Debugging ingestion issues at a specific timestamp.
- Replaying historical snapshots through a new consumer without re-fetching the feed.
- Monte Carlo simulation seeding (ADR 0008 explicitly names this as a future use case).
- Zone expansion — processing historical data for a new zone without any re-polling.

Option A makes all of the above impossible for non-Zone-110 data.

---

## Recommendation

**Adopt Option B: Raw Feed → Redpanda → Downstream Filtering.**

The immutable, complete temporal ledger is the single most valuable architectural property of the Redpanda layer. Pre-filtering permanently destroys this property for a storage saving that is manageable by other means (compression, retention tuning).

Zone filtering belongs in consumers, not in the ingestion worker.

### Implementation Guidance

1. **Sprint 02**: Publish all entities to `transit.snapshots.raw` without filtering (current implementation).
2. **Future sprint**: Implement a shared zone-filter utility in the Python analytics package.
3. **Zone filter predicate**: Use `stop_id` lookups against the GTFS static `stops.txt` to determine zone membership, supplemented by geographic bounding box for vehicles between stops.

---

## Decision Record

This recommendation should be referenced in the next ADR that governs analytics consumer design. No formal ADR is raised for Sprint 02 (this is a recommendation, not a binding decision).

---

## References

- [ADR 0008 — Redpanda as Immutable Temporal Snapshot Ledger](../adr/008-redpanda-as-immutable-snapshot-ledger.md)
- [GTFS-RT Domain Mapping](./gtfs-rt-domain-mapping.md)
- [Redpanda Topic Configuration](./redpanda-topic-configuration.md)
