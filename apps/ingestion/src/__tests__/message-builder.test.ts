import { describe, it, expect } from 'vitest';
import crypto from 'node:crypto';
import { deriveKey, entityType, buildMessages } from '../producer/message-builder.js';
import type { FeedEntity, SnapshotRawMessage } from '../types/gtfs-rt.js';

/**
 * Unit tests for the pure message-building functions.
 *
 * These are the most critical unit tests in the ingestion package because
 * buildMessages() is the only pure transformation that happens between the
 * raw feed and Redpanda. Getting the envelope shape, key strategy, and
 * filtering logic right is essential for downstream consumer correctness.
 *
 * All tests are pure — no network, no file I/O, no Redpanda connection.
 */

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const FEED_TS = '2026-06-12T09:00:00.000Z';
const FEED_VERSION = '2.0';
const INGESTION_TS = '2026-06-12T09:00:01.234Z';

const vehicleEntity: FeedEntity = {
  id: 'entity-v-001',
  is_deleted: false,
  vehicle: {
    vehicle: { id: 'ch:vbz:tram:3001', label: '3001' },
    trip: { trip_id: 'trip-8001', route_id: 'route-7' },
    position: { latitude: 47.3769, longitude: 8.5417, bearing: 270, speed: 8.3 },
    current_status: 'IN_TRANSIT_TO',
    stop_id: 'stop:zurich:central',
    timestamp: 1749722400,
  },
};

const vehicleEntityNoId: FeedEntity = {
  id: 'entity-v-002',
  is_deleted: false,
  vehicle: {
    // No VehicleDescriptor.id — fallback to entity.id
    trip: { trip_id: 'trip-8002' },
    position: { latitude: 47.38, longitude: 8.54 },
  },
};

const tripEntity: FeedEntity = {
  id: 'entity-t-001',
  is_deleted: false,
  trip_update: {
    trip: { trip_id: 'trip-9001', route_id: 'route-10' },
    stop_time_update: [
      { stop_id: 'stop-A', arrival: { delay: 120 }, departure: { delay: 120 } },
      { stop_id: 'stop-B', arrival: { delay: 60 } },
    ],
    timestamp: 1749722400,
    delay: 120,
  },
};

const tripEntityNoTripId: FeedEntity = {
  id: 'entity-t-002',
  is_deleted: false,
  trip_update: {
    trip: {
      // No trip_id — fallback to entity.id
    },
    stop_time_update: [],
  },
};

const alertEntity: FeedEntity = {
  id: 'alert-entity-xyz-001',
  is_deleted: false,
  alert: {
    active_period: [{ start: 1749722400, end: 1749808800 }],
    informed_entity: [{ route_id: 'route-7' }],
    cause: 'TECHNICAL_PROBLEM',
    effect: 'SIGNIFICANT_DELAYS',
    header_text: { translation: [{ text: 'Tram 7 delayed', language: 'en' }] },
  },
};

const deletedEntity: FeedEntity = {
  id: 'entity-deleted-001',
  is_deleted: true,
  vehicle: {
    vehicle: { id: 'ch:vbz:bus:4001' },
    position: { latitude: 47.4, longitude: 8.5 },
  },
};

const unknownEntity: FeedEntity = {
  id: 'entity-unknown-001',
  is_deleted: false,
  // No vehicle, trip_update, or alert
};

// ---------------------------------------------------------------------------
// entityType()
// ---------------------------------------------------------------------------

describe('entityType()', () => {
  it('returns VEHICLE_POSITION for entities with a vehicle field', () => {
    expect(entityType(vehicleEntity)).toBe('VEHICLE_POSITION');
  });

  it('returns TRIP_UPDATE for entities with a trip_update field', () => {
    expect(entityType(tripEntity)).toBe('TRIP_UPDATE');
  });

  it('returns ALERT for entities with an alert field', () => {
    expect(entityType(alertEntity)).toBe('ALERT');
  });

  it('returns UNKNOWN when no payload field is present', () => {
    expect(entityType(unknownEntity)).toBe('UNKNOWN');
  });
});

// ---------------------------------------------------------------------------
// deriveKey()
// ---------------------------------------------------------------------------

describe('deriveKey()', () => {
  describe('VehiclePosition keys', () => {
    it('uses vehicle.{VehicleDescriptor.id} as the key', () => {
      expect(deriveKey(vehicleEntity)).toBe('vehicle.ch:vbz:tram:3001');
    });

    it('falls back to vehicle.{entity.id} when VehicleDescriptor has no id', () => {
      expect(deriveKey(vehicleEntityNoId)).toBe('vehicle.entity-v-002');
    });

    it('all VehiclePosition keys start with the vehicle. prefix', () => {
      expect(deriveKey(vehicleEntity)).toMatch(/^vehicle\./);
      expect(deriveKey(vehicleEntityNoId)).toMatch(/^vehicle\./);
    });
  });

  describe('TripUpdate keys', () => {
    it('uses trip.{TripDescriptor.trip_id} as the key', () => {
      expect(deriveKey(tripEntity)).toBe('trip.trip-9001');
    });

    it('falls back to trip.{entity.id} when TripDescriptor has no trip_id', () => {
      expect(deriveKey(tripEntityNoTripId)).toBe('trip.entity-t-002');
    });

    it('all TripUpdate keys start with the trip. prefix', () => {
      expect(deriveKey(tripEntity)).toMatch(/^trip\./);
      expect(deriveKey(tripEntityNoTripId)).toMatch(/^trip\./);
    });
  });

  describe('Alert keys', () => {
    it('produces an alert.{12-char-hash} key for alert entities', () => {
      const key = deriveKey(alertEntity);
      expect(key).toMatch(/^alert\.[0-9a-f]{12}$/);
    });

    it('is deterministic — same entity id always produces the same key', () => {
      expect(deriveKey(alertEntity)).toBe(deriveKey(alertEntity));
    });

    it('uses sha256 of entity.id with the first 12 hex chars', () => {
      const expectedHash = crypto
        .createHash('sha256')
        .update(alertEntity.id)
        .digest('hex')
        .slice(0, 12);
      expect(deriveKey(alertEntity)).toBe(`alert.${expectedHash}`);
    });

    it('produces different keys for different entity ids', () => {
      const other: FeedEntity = { ...alertEntity, id: 'different-alert-id' };
      expect(deriveKey(alertEntity)).not.toBe(deriveKey(other));
    });
  });

  describe('Unknown entity keys', () => {
    it('uses unknown.{entity.id} for entities with no payload', () => {
      expect(deriveKey(unknownEntity)).toBe('unknown.entity-unknown-001');
    });
  });
});

// ---------------------------------------------------------------------------
// buildMessages()
// ---------------------------------------------------------------------------

describe('buildMessages()', () => {
  describe('output shape and JSON structure', () => {
    it('returns one message per non-deleted entity', () => {
      const messages = buildMessages(
        [vehicleEntity, tripEntity, alertEntity],
        FEED_TS,
        FEED_VERSION,
        INGESTION_TS,
      );
      expect(messages).toHaveLength(3);
    });

    it('message value is valid JSON', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      expect(() => JSON.parse(messages[0].value)).not.toThrow();
    });

    it('parsed envelope has the required SnapshotRawMessage fields', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;

      expect(envelope).toHaveProperty('entity_type');
      expect(envelope).toHaveProperty('entity_id');
      expect(envelope).toHaveProperty('feed_timestamp');
      expect(envelope).toHaveProperty('ingestion_timestamp');
      expect(envelope).toHaveProperty('feed_version');
      expect(envelope).toHaveProperty('payload');
    });

    it('entity_type discriminator matches the entity content', () => {
      const messages = buildMessages(
        [vehicleEntity, tripEntity, alertEntity, unknownEntity],
        FEED_TS,
        FEED_VERSION,
        INGESTION_TS,
      );

      const envelopes = messages.map((m) => JSON.parse(m.value) as SnapshotRawMessage);
      expect(envelopes[0].entity_type).toBe('VEHICLE_POSITION');
      expect(envelopes[1].entity_type).toBe('TRIP_UPDATE');
      expect(envelopes[2].entity_type).toBe('ALERT');
      expect(envelopes[3].entity_type).toBe('UNKNOWN');
    });

    it('entity_id is preserved from the FeedEntity.id', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      expect(envelope.entity_id).toBe('entity-v-001');
    });
  });

  describe('timestamp handling', () => {
    it('feed_timestamp is set to the provided feed timestamp ISO string', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      expect(envelope.feed_timestamp).toBe(FEED_TS);
    });

    it('ingestion_timestamp is set to the provided ingestion timestamp', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      expect(envelope.ingestion_timestamp).toBe(INGESTION_TS);
    });

    it('all messages in a batch share the same ingestion_timestamp', () => {
      const messages = buildMessages(
        [vehicleEntity, tripEntity, alertEntity],
        FEED_TS,
        FEED_VERSION,
        INGESTION_TS,
      );
      const timestamps = messages.map((m) => {
        const env = JSON.parse(m.value) as SnapshotRawMessage;
        return env.ingestion_timestamp;
      });
      expect(new Set(timestamps).size).toBe(1);
    });

    it('ingestion_timestamp defaults to a valid ISO string when not provided', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      // Should be a valid ISO 8601 date
      expect(() => new Date(envelope.ingestion_timestamp)).not.toThrow();
      expect(new Date(envelope.ingestion_timestamp).toISOString()).toBe(
        envelope.ingestion_timestamp,
      );
    });
  });

  describe('feed_version', () => {
    it('feed_version is set to the provided feed version string', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, '2.0', INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      expect(envelope.feed_version).toBe('2.0');
    });
  });

  describe('payload preservation', () => {
    it('VehiclePosition payload contains position data', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      const payload = envelope.payload as { position: { latitude: number } };
      expect(payload.position.latitude).toBe(47.3769);
    });

    it('TripUpdate payload contains stop_time_update array', () => {
      const messages = buildMessages([tripEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      const payload = envelope.payload as { stop_time_update: unknown[] };
      expect(Array.isArray(payload.stop_time_update)).toBe(true);
      expect(payload.stop_time_update).toHaveLength(2);
    });

    it('Alert payload contains cause and effect', () => {
      const messages = buildMessages([alertEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      const payload = envelope.payload as { cause: string; effect: string };
      expect(payload.cause).toBe('TECHNICAL_PROBLEM');
      expect(payload.effect).toBe('SIGNIFICANT_DELAYS');
    });
  });

  describe('deleted entity filtering', () => {
    it('silently drops entities with is_deleted=true', () => {
      const messages = buildMessages(
        [vehicleEntity, deletedEntity, tripEntity],
        FEED_TS,
        FEED_VERSION,
        INGESTION_TS,
      );
      // deletedEntity should be excluded
      expect(messages).toHaveLength(2);
    });

    it('returns an empty array when all entities are deleted', () => {
      const messages = buildMessages([deletedEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      expect(messages).toHaveLength(0);
    });

    it('returns an empty array for an empty entity list', () => {
      const messages = buildMessages([], FEED_TS, FEED_VERSION, INGESTION_TS);
      expect(messages).toHaveLength(0);
    });
  });

  describe('message keys', () => {
    it('VehiclePosition message key uses vehicle. prefix', () => {
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      expect(messages[0].key).toMatch(/^vehicle\./);
    });

    it('TripUpdate message key uses trip. prefix', () => {
      const messages = buildMessages([tripEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      expect(messages[0].key).toMatch(/^trip\./);
    });

    it('Alert message key uses alert. prefix', () => {
      const messages = buildMessages([alertEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      expect(messages[0].key).toMatch(/^alert\./);
    });

    it('entity_id in the envelope is independent of the Kafka message key', () => {
      // The Kafka key drives partitioning; entity_id preserves the original feed id
      const messages = buildMessages([vehicleEntity], FEED_TS, FEED_VERSION, INGESTION_TS);
      const envelope = JSON.parse(messages[0].value) as SnapshotRawMessage;
      expect(envelope.entity_id).toBe(vehicleEntity.id); // 'entity-v-001'
      expect(messages[0].key).toBe('vehicle.ch:vbz:tram:3001'); // from vehicle descriptor
      // The two are different — key is for partition routing, entity_id is for traceability
      expect(messages[0].key).not.toBe(envelope.entity_id);
    });
  });
});
