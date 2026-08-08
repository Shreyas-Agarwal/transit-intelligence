# GTFS-RT Domain Mapping

## Overview

This document defines how GTFS-RT feed entities map to internal domain concepts, Redpanda topics, and message key strategies.

This is a Sprint 02 design document. No production implementation is required at this stage.

---

## The Fundamental Question

The GTFS-RT feed emits three entity types in every poll cycle:

| GTFS-RT Entity    | Nature                        |
| ----------------- | ----------------------------- |
| `VehiclePosition` | Where is a vehicle right now? |
| `TripUpdate`      | Is a trip running on time?    |
| `Alert`           | Is there a disruption?        |

The platform must decide:

1. What Redpanda topic does each entity type belong to?
2. What key should each message use?
3. Where does the authoritative timestamp come from?

---

## Sprint 02 Topic Assignment

Per ADR 0008, Sprint 02 publishes **all entity types** to a single topic:

```text
transit.snapshots.raw
```

This is the appropriate choice during an exploratory sprint because:

- It keeps the implementation minimal.
- All entities come from the same feed poll cycle and represent the same temporal snapshot.
- Downstream consumers that care about specific entity types can filter by the `entity_type` header.
- Topic specialisation (e.g. separate vehicle vs. trip topics) is deferred until consumption patterns validate the need.

---

## Event Ownership

| Entity Type       | Domain Concept              | Owner System                  | Redpanda Topic          |
| ----------------- | --------------------------- | ----------------------------- | ----------------------- |
| `VehiclePosition` | Spatial vehicle observation | Ingestion Worker (TypeScript) | `transit.snapshots.raw` |
| `TripUpdate`      | Schedule deviation record   | Ingestion Worker (TypeScript) | `transit.snapshots.raw` |
| `Alert`           | Network disruption event    | Ingestion Worker (TypeScript) | `transit.snapshots.raw` |

Future sprints may split these into specialised topics as downstream analytical consumers are built.

---

## Message Key Strategy

Message keys determine Redpanda partition assignment. Per ADR 0008, keys must preserve spatial-temporal ordering for trajectory reconstruction.

| Entity Type       | Key Format                        | Example                    |
| ----------------- | --------------------------------- | -------------------------- |
| `VehiclePosition` | `vehicle.{vehicle_id}`            | `vehicle.ch:vbz:tram:3001` |
| `TripUpdate`      | `trip.{trip_id}`                  | `trip.8001.20260612`       |
| `Alert`           | `alert.{sha256(entity_id)[0:12]}` | `alert.3f9a2b7c1d4e`       |

**Rationale:**

- `vehicle.*` keys ensure all observations for a given vehicle land in the same partition → required for trajectory reconstruction.
- `trip.*` keys ensure all stop-time updates for a trip land in the same partition → required for delay propagation analysis.
- `alert.*` uses a hash because GTFS-RT alert `id` fields are provider-defined and not guaranteed unique across feeds.

---

## Timestamp Strategy

GTFS-RT provides multiple timestamp signals. The platform must be explicit about which timestamp is used and for what purpose.

| Timestamp               | Source                        | Field         | Meaning                                   |
| ----------------------- | ----------------------------- | ------------- | ----------------------------------------- |
| **Feed timestamp**      | `FeedHeader.timestamp`        | POSIX seconds | When the provider generated this snapshot |
| **Entity timestamp**    | `VehiclePosition.timestamp`   | POSIX seconds | When the vehicle observation was recorded |
| **Trip update time**    | `StopTimeUpdate.arrival.time` | POSIX seconds | Predicted absolute arrival time           |
| **Ingestion timestamp** | Wall clock at publish         | ISO 8601      | When this platform ingested the message   |

### Published Message Timestamp Fields

Each `SnapshotRawMessage` envelope contains:

```json
{
  "feed_timestamp": "2026-06-12T09:00:00.000Z",
  "ingestion_timestamp": "2026-06-12T09:00:01.234Z",
  ...
}
```

- `feed_timestamp` = event time (per ADR 0008's temporal semantics — when the data was true)
- `ingestion_timestamp` = processing time (when the system observed the event)

Downstream analytical consumers must use `feed_timestamp` for all temporal reconstruction and should treat `ingestion_timestamp` as metadata only.

---

## Candidate Topic Topology — Future Sprints

When downstream consumption patterns are established, the topic topology may evolve:

| Topic                            | Content                                                  | Sprint                 |
| -------------------------------- | -------------------------------------------------------- | ---------------------- |
| `transit.snapshots.raw`          | All decoded entities (current)                           | Sprint 02              |
| `transit.snapshots.normalized`   | Validated, deduplicated snapshots                        | Sprint 03+             |
| `transit.state.deltas`           | Computed state transitions between consecutive snapshots | Sprint 03+             |
| `transit.metrics.operational`    | Derived delay, congestion, and resilience metrics        | Sprint 04+             |
| `transit.vehicles` _(candidate)_ | VehiclePosition stream only                              | If needed by consumers |
| `transit.trips` _(candidate)_    | TripUpdate stream only                                   | If needed by consumers |
| `transit.alerts` _(candidate)_   | Alert stream only                                        | If needed by consumers |

The specialised entity-level topics (`transit.vehicles`, etc.) should **not** be created until a consumer exists that needs them. ADR 0008 explicitly warns against premature topic proliferation.

---

## FeedEntity Field Mapping

### VehiclePosition → Internal Concept

| GTFS-RT Field        | Internal Concept  | Notes                                            |
| -------------------- | ----------------- | ------------------------------------------------ |
| `vehicle.id`         | `vehicle_id`      | Primary entity identifier                        |
| `vehicle.label`      | `vehicle_label`   | Human-readable (tram/bus number)                 |
| `trip.trip_id`       | `trip_id`         | Links to GTFS static schedule                    |
| `trip.route_id`      | `route_id`        |                                                  |
| `position.latitude`  | `latitude`        | WGS-84                                           |
| `position.longitude` | `longitude`       | WGS-84                                           |
| `position.bearing`   | `bearing`         | Degrees from North                               |
| `position.speed`     | `speed_ms`        | m/s — convert to km/h for display                |
| `current_status`     | `vehicle_status`  | `IN_TRANSIT_TO` \| `STOPPED_AT` \| `INCOMING_AT` |
| `stop_id`            | `current_stop_id` |                                                  |
| `timestamp`          | `observed_at`     | Event time (POSIX → ISO)                         |

### TripUpdate → Internal Concept

| GTFS-RT Field                        | Internal Concept       | Notes                       |
| ------------------------------------ | ---------------------- | --------------------------- |
| `trip.trip_id`                       | `trip_id`              | Primary entity identifier   |
| `trip.route_id`                      | `route_id`             |                             |
| `delay`                              | `trip_delay_s`         | Trip-level delay in seconds |
| `stop_time_update[].stop_id`         | `stop_id`              | Per-stop updates            |
| `stop_time_update[].arrival.delay`   | `arrival_delay_s`      | Positive = late             |
| `stop_time_update[].departure.delay` | `departure_delay_s`    |                             |
| `stop_time_update[].arrival.time`    | `predicted_arrival_at` | Absolute POSIX → ISO        |
| `timestamp`                          | `observed_at`          |                             |

### Alert → Internal Concept

| GTFS-RT Field       | Internal Concept                     | Notes                                   |
| ------------------- | ------------------------------------ | --------------------------------------- |
| `cause`             | `cause`                              | Enum string from proto                  |
| `effect`            | `effect`                             | Enum string from proto                  |
| `header_text`       | `summary`                            | Prefer `language=en`, fallback to first |
| `description_text`  | `description`                        |                                         |
| `active_period[]`   | `active_periods`                     |                                         |
| `informed_entity[]` | `affected_routes` / `affected_stops` |                                         |

---

## References

- [ADR 0008 — Redpanda as Immutable Temporal Snapshot Ledger](../adr/008-redpanda-as-immutable-snapshot-ledger.md)
- [ADR 0010 — Polyglot Runtime Architecture](../adr/0010-adopt-polyglot-runtime-architecture.md)
- [GTFS-RT Feed Structure](./gtfs-rt-feed-structure.md)
- [Redpanda Topic Configuration](./redpanda-topic-configuration.md)
