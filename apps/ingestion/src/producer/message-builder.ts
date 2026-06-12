/**
 * Message builder — pure functions for constructing Redpanda messages from GTFS-RT entities.
 *
 * Extracted from publisher.ts to enable unit testing without requiring a live
 * Redpanda broker, network access, or protobuf I/O.
 *
 * All functions here are pure (no side effects, no I/O, no module-level state).
 */

import crypto from 'node:crypto';
import type { FeedEntity, SnapshotRawMessage, EntityType } from '../types/gtfs-rt.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface KafkaMessage {
  key: string;
  /** JSON string — human-readable via `rpk topic consume` */
  value: string;
}

// ---------------------------------------------------------------------------
// Entity type discriminator
// ---------------------------------------------------------------------------

/**
 * Returns the entity type discriminator string for a FeedEntity.
 *
 * Priority order matches the GTFS-RT proto field ordering:
 *   trip_update → vehicle → alert
 * A FeedEntity carries at most one payload field, so the order only matters
 * for the UNKNOWN fallback.
 */
export function entityType(entity: FeedEntity): EntityType {
  if (entity.vehicle) return 'VEHICLE_POSITION';
  if (entity.trip_update) return 'TRIP_UPDATE';
  if (entity.alert) return 'ALERT';
  return 'UNKNOWN';
}

// ---------------------------------------------------------------------------
// Message key derivation
// ---------------------------------------------------------------------------

/**
 * Derives a stable Redpanda partition key for a given GTFS-RT entity.
 *
 * Key strategy (per docs/design/gtfs-rt-domain-mapping.md):
 *
 *   VehiclePosition → `vehicle.{vehicle_id}`
 *     Uses VehicleDescriptor.id as the natural key.
 *     Falls back to entity.id if the vehicle descriptor is absent.
 *
 *   TripUpdate → `trip.{trip_id}`
 *     Uses TripDescriptor.trip_id as the natural key.
 *     Falls back to entity.id if the trip descriptor is absent.
 *
 *   Alert → `alert.{sha256(entity_id)[0:12]}`
 *     Alert entity ids are provider-defined strings without guaranteed stability.
 *     A deterministic sha256 prefix keeps keys short and uniformly distributed.
 *
 *   Unknown → `unknown.{entity_id}`
 *     Explicit fallback — should not appear in practice with a FULL_DATASET feed.
 *
 * Keys are used by Redpanda to assign messages to partitions. Consistent keys
 * ensure that all updates for the same logical entity land in the same partition,
 * preserving chronological ordering for temporal reconstruction (ADR 0008).
 */
export function deriveKey(entity: FeedEntity): string {
  if (entity.vehicle) {
    const vehicleId = entity.vehicle.vehicle?.id ?? entity.id;
    return `vehicle.${vehicleId}`;
  }
  if (entity.trip_update) {
    const tripId = entity.trip_update.trip?.trip_id ?? entity.id;
    return `trip.${tripId}`;
  }
  if (entity.alert) {
    const hash = crypto.createHash('sha256').update(entity.id).digest('hex').slice(0, 12);
    return `alert.${hash}`;
  }
  return `unknown.${entity.id}`;
}

// ---------------------------------------------------------------------------
// Message envelope builder
// ---------------------------------------------------------------------------

/**
 * Wraps a list of FeedEntity records in SnapshotRawMessage envelopes and
 * serialises each to a JSON string ready for Redpanda.
 *
 * Design decisions:
 *   - `is_deleted` entities are silently dropped. They appear only in
 *     DIFFERENTIAL mode feeds; the Swiss feed uses FULL_DATASET.
 *   - `ingestion_timestamp` is evaluated once per batch (not per entity) so
 *     all messages from the same poll cycle share the same processing timestamp.
 *   - Payload is the decoded entity object, unchanged — no field transformation.
 *
 * @param entities         Decoded GTFS-RT entities from one feed poll cycle.
 * @param feedTimestampIso ISO 8601 string of the FeedHeader.timestamp.
 * @param feedVersion      GTFS-RT spec version from FeedHeader.
 * @param ingestionTs      Optional override for ingestion_timestamp (for tests).
 *                         Defaults to `new Date().toISOString()` when omitted.
 */
export function buildMessages(
  entities: FeedEntity[],
  feedTimestampIso: string,
  feedVersion: string,
  ingestionTs?: string,
): KafkaMessage[] {
  const ingestionTimestamp = ingestionTs ?? new Date().toISOString();

  return entities
    .filter((e) => !e.is_deleted)
    .map((entity) => {
      const type = entityType(entity);
      const payload = entity.vehicle ?? entity.trip_update ?? entity.alert ?? {};

      const envelope: SnapshotRawMessage = {
        entity_type: type,
        entity_id: entity.id,
        feed_timestamp: feedTimestampIso,
        ingestion_timestamp: ingestionTimestamp,
        feed_version: feedVersion,
        payload,
      };

      return {
        key: deriveKey(entity),
        value: JSON.stringify(envelope),
      };
    });
}
