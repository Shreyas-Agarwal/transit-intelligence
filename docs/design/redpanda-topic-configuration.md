# Redpanda Topic Configuration

## Overview

This document defines the Redpanda topic configuration for the Transit Intelligence platform, aligned with ADR 0008 (Adopt Redpanda as Immutable Temporal Snapshot Ledger).

Sprint 02 introduces the minimal Phase 1 topic topology. Additional topics will only be created when downstream consumption patterns validate the need.

---

## Phase 1 Topic Topology

| Topic                          | Purpose                                                  | Sprint Introduced |
| ------------------------------ | -------------------------------------------------------- | ----------------- |
| `transit.snapshots.raw`        | Decoded GTFS-RT entities (canonical JSON)                | Sprint 02         |
| `transit.snapshots.normalized` | Validated, deduplicated operational snapshots            | Future            |
| `transit.state.deltas`         | Computed state transitions between consecutive snapshots | Future            |
| `transit.metrics.operational`  | Derived observability and resilience metrics             | Future            |

---

## Topic Configuration

### `transit.snapshots.raw`

```
Partitions:     1
Replication:    1 (single-node local dev)
Retention:      7 days (604800000 ms)
Cleanup policy: delete
```

**Justification**: 1 partition is appropriate for Phase 1. ADR 0008 explicitly defers partitioning specialisation until replay patterns are validated. A single partition ensures strict ordering of all snapshot messages during the exploration phase.

**Retention**: 7 days gives a one-week replay window — sufficient for exploratory analysis and debugging without local storage exhaustion.

---

### `transit.snapshots.normalized`

```
Partitions:     1
Replication:    1
Retention:      7 days
Cleanup policy: delete
```

_Not yet populated. Reserved for future normalisation consumers._

---

### `transit.state.deltas`

```
Partitions:     1
Replication:    1
Retention:      7 days
Cleanup policy: delete
```

_Not yet populated. Reserved for future delta computation consumers._

---

### `transit.metrics.operational`

```
Partitions:     1
Replication:    1
Retention:      30 days (2592000000 ms)
Cleanup policy: delete
```

_Not yet populated. Longer retention for operational metrics (analytics workloads)._

---

## Message Format — `transit.snapshots.raw`

All messages in `transit.snapshots.raw` are UTF-8 encoded JSON strings (`SnapshotRawMessage` envelope).

### Message Key

```
vehicle.{vehicle_id}   → VehiclePosition entities
trip.{trip_id}         → TripUpdate entities
alert.{sha256[0:12]}   → Alert entities
```

### Message Value (JSON schema)

```json
{
  "entity_type": "VEHICLE_POSITION | TRIP_UPDATE | ALERT | UNKNOWN",
  "entity_id": "string — GTFS-RT entity id from the feed",
  "feed_timestamp": "ISO 8601 — when the feed provider generated this snapshot",
  "ingestion_timestamp": "ISO 8601 — when the ingestion worker processed this message",
  "feed_version": "string — gtfs_realtime_version from feed header",
  "payload": {
    // VehiclePosition | TripUpdate | Alert fields (decoded from protobuf, unchanged)
  }
}
```

### Example VehiclePosition Message

```json
{
  "entity_type": "VEHICLE_POSITION",
  "entity_id": "vehicle-entity-001",
  "feed_timestamp": "2026-06-12T09:00:00.000Z",
  "ingestion_timestamp": "2026-06-12T09:00:01.234Z",
  "feed_version": "2.0",
  "payload": {
    "trip": {
      "trip_id": "trip:sbb:8001",
      "route_id": "route:vbz:7",
      "schedule_relationship": "SCHEDULED"
    },
    "vehicle": {
      "id": "ch:vbz:tram:3001",
      "label": "3001"
    },
    "position": {
      "latitude": 47.3769,
      "longitude": 8.5417,
      "bearing": 270,
      "speed": 8.3
    },
    "current_status": "IN_TRANSIT_TO",
    "stop_id": "stop:zurich:central",
    "timestamp": 1749722400
  }
}
```

---

## rpk Commands

```bash
# List all topics
rpk topic list

# Create all Phase 1 topics manually (also done automatically by ensureTopics())
rpk topic create transit.snapshots.raw         --partitions 1 --replicas 1
rpk topic create transit.snapshots.normalized  --partitions 1 --replicas 1
rpk topic create transit.state.deltas          --partitions 1 --replicas 1
rpk topic create transit.metrics.operational   --partitions 1 --replicas 1

# Set retention on transit.snapshots.raw (7 days)
rpk topic alter-config transit.snapshots.raw --set retention.ms=604800000

# Set retention on transit.metrics.operational (30 days)
rpk topic alter-config transit.metrics.operational --set retention.ms=2592000000

# Inspect a topic's configuration
rpk topic describe transit.snapshots.raw --print-configs

# Consume messages (human-readable JSON — all fields visible)
rpk topic consume transit.snapshots.raw

# Consume from the beginning
rpk topic consume transit.snapshots.raw --offset start

# Consume with key display
rpk topic consume transit.snapshots.raw --format '%k: %v\n'
```

---

## Partitioning Evolution Path

Phase 1 uses 1 partition per topic intentionally. When the platform matures:

| Trigger                               | Partition Strategy                                     |
| ------------------------------------- | ------------------------------------------------------ |
| Multiple vehicle trajectory consumers | Partition `transit.snapshots.raw` by `vehicle_id` hash |
| Route-level parallel analytics        | Partition by `route_id`                                |
| Region-level scaling                  | Partition by geographic zone                           |

Repartitioning requires topic recreation and consumer offset reset — plan accordingly.

---

## References

- [ADR 0008 — Redpanda as Immutable Temporal Snapshot Ledger](../adr/008-redpanda-as-immutable-snapshot-ledger.md)
- [GTFS-RT Domain Mapping](./gtfs-rt-domain-mapping.md)
- [Local Redpanda Setup](../runbooks/local-redpanda-setup.md)
