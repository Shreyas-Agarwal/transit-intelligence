import path from 'node:path';
import { describe, it, expect, beforeAll } from 'vitest';
import protobuf from 'protobufjs';
import { decodeFeedBuffer } from '../feed/decoder.js';

/**
 * Unit tests for the protobuf decode pipeline.
 *
 * Strategy: use protobufjs to encode a known FeedMessage into a binary buffer,
 * then pass that buffer through decodeFeedBuffer() and assert the round-trip
 * correctness. This tests the complete decode path — proto loading, type
 * lookup, decode(), toObject() — without requiring a live feed endpoint.
 *
 * The test proto file is the same canonical gtfs-realtime.proto used in
 * production, ensuring the test exercises real decode behaviour.
 */

const PROTO_PATH = path.resolve(__dirname, '../../proto/gtfs-realtime.proto');

// ---------------------------------------------------------------------------
// Fixtures — minimal protobuf payloads for each entity type
// ---------------------------------------------------------------------------

let root: protobuf.Root;
let FeedMessage: protobuf.Type;

beforeAll(async () => {
  root = new protobuf.Root();
  await root.load(PROTO_PATH, { keepCase: true });
  FeedMessage = root.lookupType('transit_realtime.FeedMessage');
});

/**
 * Helper: encode a plain object as a FeedMessage binary buffer.
 * Mirrors what the Open Data Swiss API returns as its HTTP response body.
 */
function encodeFeedMessage(payload: Record<string, unknown>): Buffer {
  const message = FeedMessage.create(payload);
  const bytes = FeedMessage.encode(message).finish();
  return Buffer.from(bytes);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('decodeFeedBuffer()', () => {
  describe('feed header', () => {
    it('decodes the gtfs_realtime_version from the feed header', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0', incrementality: 0, timestamp: 1749722400 },
        entity: [],
      });

      const feed = await decodeFeedBuffer(buffer);
      expect(feed.header.gtfs_realtime_version).toBe('2.0');
    });

    it('decodes the incrementality enum as a string (FULL_DATASET)', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0', incrementality: 0 },
        entity: [],
      });

      const feed = await decodeFeedBuffer(buffer);
      // toObject({ enums: String }) should convert 0 → 'FULL_DATASET'
      expect(feed.header.incrementality).toBe('FULL_DATASET');
    });

    it('decodes the header timestamp as a number (POSIX seconds)', async () => {
      const posixTs = 1749722400;
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0', timestamp: posixTs },
        entity: [],
      });

      const feed = await decodeFeedBuffer(buffer);
      expect(feed.header.timestamp).toBe(posixTs);
    });
  });

  describe('entity array', () => {
    it('decodes an empty entity list', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0', incrementality: 0 },
        entity: [],
      });

      const feed = await decodeFeedBuffer(buffer);
      expect(Array.isArray(feed.entity)).toBe(true);
      expect(feed.entity).toHaveLength(0);
    });

    it('decodes entity count correctly', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0' },
        entity: [
          {
            id: 'entity-1',
            vehicle: {
              vehicle: { id: 'vehicle-1' },
              position: { latitude: 47.37, longitude: 8.54 },
            },
          },
          {
            id: 'entity-2',
            vehicle: {
              vehicle: { id: 'vehicle-2' },
              position: { latitude: 47.38, longitude: 8.55 },
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      expect(feed.entity).toHaveLength(2);
    });
  });

  describe('VehiclePosition decoding', () => {
    it('decodes VehiclePosition entity id', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0' },
        entity: [
          {
            id: 'vp-entity-001',
            vehicle: {
              vehicle: { id: 'ch:vbz:tram:3001', label: '3001' },
              position: { latitude: 47.3769, longitude: 8.5417, bearing: 270.0 },
              timestamp: 1749722400,
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      const entity = feed.entity[0];
      expect(entity.id).toBe('vp-entity-001');
    });

    it('decodes vehicle descriptor fields', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0' },
        entity: [
          {
            id: 'vp-001',
            vehicle: {
              vehicle: { id: 'ch:vbz:tram:3001', label: '3001' },
              position: { latitude: 47.3769, longitude: 8.5417 },
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      const vehicle = feed.entity[0].vehicle;
      expect(vehicle?.vehicle?.id).toBe('ch:vbz:tram:3001');
      expect(vehicle?.vehicle?.label).toBe('3001');
    });

    it('decodes position latitude and longitude', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0' },
        entity: [
          {
            id: 'vp-001',
            vehicle: {
              position: { latitude: 47.3769, longitude: 8.5417 },
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      const pos = feed.entity[0].vehicle?.position;
      expect(pos?.latitude).toBeCloseTo(47.3769, 3);
      expect(pos?.longitude).toBeCloseTo(8.5417, 3);
    });

    it('decodes current_status as a string enum', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0' },
        entity: [
          {
            id: 'vp-001',
            vehicle: {
              position: { latitude: 47.3, longitude: 8.5 },
              current_status: 2, // IN_TRANSIT_TO
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      // toObject({ enums: String }) converts 2 → 'IN_TRANSIT_TO'
      expect(feed.entity[0].vehicle?.current_status).toBe('IN_TRANSIT_TO');
    });
  });

  describe('TripUpdate decoding', () => {
    it('decodes trip_id from TripDescriptor', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0' },
        entity: [
          {
            id: 'tu-001',
            trip_update: {
              trip: { trip_id: 'trip-sbb-8001', route_id: 'route-7' },
              stop_time_update: [],
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      expect(feed.entity[0].trip_update?.trip?.trip_id).toBe('trip-sbb-8001');
    });

    it('decodes stop_time_update array with arrival delay', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0' },
        entity: [
          {
            id: 'tu-001',
            trip_update: {
              trip: { trip_id: 'trip-001' },
              stop_time_update: [
                {
                  stop_id: 'stop-A',
                  stop_sequence: 1,
                  arrival: { delay: 120 },
                  departure: { delay: 120 },
                },
              ],
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      const updates = feed.entity[0].trip_update?.stop_time_update ?? [];
      expect(updates).toHaveLength(1);
      expect(updates[0].stop_id).toBe('stop-A');
      expect(updates[0].arrival?.delay).toBe(120);
    });
  });

  describe('decoded output is JSON-serialisable', () => {
    it('the decoded feed message can be JSON.stringify()d without error', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0', timestamp: 1749722400 },
        entity: [
          {
            id: 'vp-001',
            vehicle: {
              vehicle: { id: 'vehicle-001' },
              position: { latitude: 47.37, longitude: 8.54 },
              timestamp: 1749722400,
            },
          },
        ],
      });

      const feed = await decodeFeedBuffer(buffer);
      expect(() => JSON.stringify(feed)).not.toThrow();
    });

    it('does not contain Long objects after decoding (longs are plain numbers)', async () => {
      const buffer = encodeFeedMessage({
        header: { gtfs_realtime_version: '2.0', timestamp: 1749722400 },
        entity: [],
      });

      const feed = await decodeFeedBuffer(buffer);
      // If Long objects were present, timestamp would be { low, high, unsigned }
      // toObject({ longs: Number }) should return a plain JavaScript number.
      expect(typeof feed.header.timestamp).toBe('number');
    });
  });
});
