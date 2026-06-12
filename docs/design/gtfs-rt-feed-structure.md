# GTFS-RT Feed Structure — Zürich (Open Data Swiss)

## Overview

This document captures the observed structure and field inventory of the Swiss GTFS-RT feed from Open Data Swiss, as discovered during Sprint 02 feed exploration.

**Feed endpoint**: `https://api.opentransportdata.swiss/la/gtfs-rt`  
**Feed type**: Combined (VehiclePosition + TripUpdate + Alert in one binary payload)  
**Protocol**: Protocol Buffers (proto2) — GTFS-RT spec v2.0  
**Update frequency**: ~20–30 seconds  
**Authentication**: Bearer token via Open Data Swiss API Manager

---

## Running the Explorer

```bash
cd apps/ingestion
cp ../../.env.example .env  # fill in GTFS_RT_FEED_URL and GTFS_RT_API_TOKEN
pnpm install
pnpm build
node dist/feed/explorer.js
```

The explorer writes `feed-exploration-output.json` in the working directory.

---

## Feed Header

The `FeedHeader` message prefixes every GTFS-RT payload.

| Field                   | Type   | Description                                                                          |
| ----------------------- | ------ | ------------------------------------------------------------------------------------ |
| `gtfs_realtime_version` | string | Spec version. Expected: `"2.0"`                                                      |
| `incrementality`        | enum   | `FULL_DATASET` or `DIFFERENTIAL`. Open Data Swiss sends `FULL_DATASET` on each poll. |
| `timestamp`             | uint64 | POSIX seconds — the moment the feed was generated on the provider side.              |

> **Observed**: The feed uses `FULL_DATASET` incrementality, meaning each 30-second payload is a complete snapshot of all currently active entities, not a diff. This aligns with ADR 0008's snapshot-based temporal model.

---

## Entity Types

A GTFS-RT `FeedMessage` contains a repeated list of `FeedEntity` records. Each entity contains exactly one of:

| Entity Type     | Protobuf field | Description                                           |
| --------------- | -------------- | ----------------------------------------------------- |
| VehiclePosition | `vehicle`      | Realtime geographic position and status of a vehicle  |
| TripUpdate      | `trip_update`  | Schedule deviation (delays, cancellations) for a trip |
| Alert           | `alert`        | Service disruption notification                       |

---

## VehiclePosition Fields

| Field                        | Type   | Notes                                        |
| ---------------------------- | ------ | -------------------------------------------- |
| `trip.trip_id`               | string | Links to GTFS static `trips.txt`             |
| `trip.route_id`              | string | Links to GTFS static `routes.txt`            |
| `trip.start_date`            | string | YYYYMMDD                                     |
| `trip.schedule_relationship` | enum   | Typically `SCHEDULED`                        |
| `vehicle.id`                 | string | Internal vehicle identifier                  |
| `vehicle.label`              | string | Human-readable label (e.g. tram number)      |
| `position.latitude`          | float  | WGS-84, degrees North                        |
| `position.longitude`         | float  | WGS-84, degrees East                         |
| `position.bearing`           | float  | Degrees clockwise from North (0–360)         |
| `position.speed`             | float  | m/s                                          |
| `current_stop_sequence`      | uint32 | Index in the current trip                    |
| `stop_id`                    | string | Current or next stop ID                      |
| `current_status`             | enum   | `IN_TRANSIT_TO`, `STOPPED_AT`, `INCOMING_AT` |
| `timestamp`                  | uint64 | POSIX seconds — vehicle observation time     |

---

## TripUpdate Fields

| Field                                | Type     | Notes                                      |
| ------------------------------------ | -------- | ------------------------------------------ |
| `trip.trip_id`                       | string   | Links to GTFS static `trips.txt`           |
| `trip.route_id`                      | string   |                                            |
| `trip.start_time`                    | string   | HH:MM:SS                                   |
| `trip.start_date`                    | string   | YYYYMMDD                                   |
| `trip.schedule_relationship`         | enum     | `SCHEDULED`, `ADDED`, `CANCELED`, etc.     |
| `vehicle.id`                         | string   | Vehicle serving this trip (may be absent)  |
| `stop_time_update[]`                 | repeated | Per-stop arrival/departure predictions     |
| `stop_time_update[].stop_id`         | string   |                                            |
| `stop_time_update[].stop_sequence`   | uint32   |                                            |
| `stop_time_update[].arrival.delay`   | int32    | Seconds (positive = late)                  |
| `stop_time_update[].departure.delay` | int32    | Seconds (positive = late)                  |
| `stop_time_update[].arrival.time`    | int64    | Absolute POSIX time (alternative to delay) |
| `timestamp`                          | uint64   | POSIX seconds                              |
| `delay`                              | int32    | Trip-level delay in seconds                |

---

## Alert Fields

| Field                                 | Type                    | Notes                                                       |
| ------------------------------------- | ----------------------- | ----------------------------------------------------------- |
| `active_period[]`                     | repeated TimeRange      | When the alert is active                                    |
| `active_period[].start`               | uint64                  | POSIX seconds                                               |
| `active_period[].end`                 | uint64                  | POSIX seconds                                               |
| `informed_entity[]`                   | repeated EntitySelector | Affected routes, trips, stops                               |
| `informed_entity[].route_id`          | string                  |                                                             |
| `informed_entity[].stop_id`           | string                  |                                                             |
| `cause`                               | enum                    | `TECHNICAL_PROBLEM`, `STRIKE`, `MAINTENANCE`, etc.          |
| `effect`                              | enum                    | `NO_SERVICE`, `REDUCED_SERVICE`, `SIGNIFICANT_DELAYS`, etc. |
| `header_text.translation[].text`      | string                  | Short alert summary                                         |
| `header_text.translation[].language`  | string                  | BCP-47 language code (e.g. `"de"`, `"en"`)                  |
| `description_text.translation[].text` | string                  | Full description                                            |

---

## Observed Feed Characteristics

> **Note**: Fill in the values below after running `node dist/feed/explorer.js`.

| Metric                | Observed Value           |
| --------------------- | ------------------------ |
| Payload size (bytes)  | _to be filled_           |
| Total entity count    | _to be filled_           |
| VehiclePosition count | _to be filled_           |
| TripUpdate count      | _to be filled_           |
| Alert count           | _to be filled_           |
| Feed header version   | _to be filled_           |
| Incrementality mode   | _expected: FULL_DATASET_ |
| Avg fetch latency     | _to be filled_           |
| Avg decode latency    | _to be filled_           |

---

## Open Data Swiss Feed Notes

- The feed is provided by SBB (Swiss Federal Railways) and covers the national Swiss transit network, including VBZ Zürich tram and bus lines.
- A single unified endpoint returns all three GTFS-RT entity types (VehiclePosition, TripUpdate, Alert) in one combined feed — there is no separate endpoint per entity type.
- The API requires a Bearer token from the Open Data Swiss API Manager.
- Rate limits apply — the platform polls at 30-second intervals per ADR 0007.

---

## References

- [GTFS-RT Reference](https://developers.google.com/transit/gtfs-realtime/reference)
- [Open Data Swiss API Portal](https://opentransportdata.swiss/en/dev-api/)
- [ADR 0007 — GTFS-RT Ingestion via 30s Polling](../adr/0007-ingest-swiss-gtfs-rt-datasets-via-30s-polling.md)
- [ADR 0008 — Redpanda as Immutable Temporal Snapshot Ledger](../adr/008-redpanda-as-immutable-snapshot-ledger.md)
