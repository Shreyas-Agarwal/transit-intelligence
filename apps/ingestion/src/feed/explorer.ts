/**
 * GTFS-RT Feed Explorer — Sprint 02 Exploration Utility
 *
 * Run via:  node dist/feed/explorer.js
 *
 * This is a one-shot tool (no polling loop). It fetches the GTFS-RT feed
 * once, decodes the protobuf payload, and logs a structured summary of what
 * the feed contains. Output is intended for documentation and domain mapping.
 *
 * The summary is also written to feed-exploration-output.json in the working
 * directory for offline reference.
 */

import fs from 'node:fs';
import { Logger } from '@transit-intelligence/shared-logger';
import { config } from '../config.js';
import { fetchFeedBuffer } from './fetcher.js';
import { decodeFeedBuffer } from './decoder.js';
import type { FeedEntity } from '../types/gtfs-rt.js';

const logger = new Logger('GTFS-RT-Explorer');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function posixToIso(seconds: number | undefined): string {
  if (!seconds) return '—';
  return new Date(seconds * 1000).toISOString();
}

function pickFirst<T>(arr: T[], count = 3): T[] {
  return arr.slice(0, count);
}

function summariseEntity(entity: FeedEntity): Record<string, unknown> {
  if (entity.vehicle) {
    const v = entity.vehicle;
    return {
      entity_id: entity.id,
      type: 'VEHICLE_POSITION',
      vehicle_id: v.vehicle?.id ?? '—',
      vehicle_label: v.vehicle?.label ?? '—',
      trip_id: v.trip?.trip_id ?? '—',
      route_id: v.trip?.route_id ?? '—',
      latitude: v.position?.latitude ?? '—',
      longitude: v.position?.longitude ?? '—',
      bearing: v.position?.bearing ?? '—',
      speed_ms: v.position?.speed ?? '—',
      status: v.current_status ?? '—',
      stop_id: v.stop_id ?? '—',
      timestamp_utc: posixToIso(v.timestamp),
      congestion: v.congestion_level ?? '—',
    };
  }

  if (entity.trip_update) {
    const t = entity.trip_update;
    const firstStop = t.stop_time_update[0];
    return {
      entity_id: entity.id,
      type: 'TRIP_UPDATE',
      trip_id: t.trip?.trip_id ?? '—',
      route_id: t.trip?.route_id ?? '—',
      vehicle_id: t.vehicle?.id ?? '—',
      stop_time_update_count: t.stop_time_update.length,
      first_stop_id: firstStop?.stop_id ?? '—',
      first_arrival_delay_s: firstStop?.arrival?.delay ?? '—',
      first_departure_delay_s: firstStop?.departure?.delay ?? '—',
      trip_level_delay_s: t.delay ?? '—',
      timestamp_utc: posixToIso(t.timestamp),
    };
  }

  if (entity.alert) {
    const a = entity.alert;
    const headerEn =
      a.header_text?.translation?.find((t) => t.language === 'en')?.text ??
      a.header_text?.translation?.[0]?.text ??
      '—';
    return {
      entity_id: entity.id,
      type: 'ALERT',
      cause: a.cause ?? '—',
      effect: a.effect ?? '—',
      header: headerEn,
      active_period_count: a.active_period.length,
      informed_entity_count: a.informed_entity.length,
    };
  }

  return { entity_id: entity.id, type: 'UNKNOWN', is_deleted: entity.is_deleted };
}

// ---------------------------------------------------------------------------
// Main exploration entry point
// ---------------------------------------------------------------------------

async function explore(): Promise<void> {
  logger.info('Starting GTFS-RT feed exploration', { feedUrl: config.feedUrl });

  // ── Fetch ────────────────────────────────────────────────────────────────
  const fetchStart = Date.now();
  const buffer = await fetchFeedBuffer(config.feedUrl, config.feedApiToken);
  const fetchMs = Date.now() - fetchStart;

  logger.info('Feed fetched', {
    bytes: buffer.length,
    fetch_latency_ms: fetchMs,
  });

  // ── Decode ───────────────────────────────────────────────────────────────
  const decodeStart = Date.now();
  const feed = await decodeFeedBuffer(buffer);
  const decodeMs = Date.now() - decodeStart;

  logger.info('Feed decoded', { decode_latency_ms: decodeMs });

  // ── Header ───────────────────────────────────────────────────────────────
  const header = feed.header;
  logger.info('Feed header', {
    gtfs_realtime_version: header.gtfs_realtime_version,
    incrementality: header.incrementality,
    timestamp_utc: posixToIso(header.timestamp),
    timestamp_posix: header.timestamp,
  });

  // ── Entity counts ─────────────────────────────────────────────────────────
  const entities = feed.entity ?? [];
  const vehicleEntities = entities.filter((e) => !!e.vehicle);
  const tripUpdateEntities = entities.filter((e) => !!e.trip_update);
  const alertEntities = entities.filter((e) => !!e.alert);
  const unknownEntities = entities.filter((e) => !e.vehicle && !e.trip_update && !e.alert);

  logger.info('Entity counts', {
    total: entities.length,
    vehicle_positions: vehicleEntities.length,
    trip_updates: tripUpdateEntities.length,
    alerts: alertEntities.length,
    unknown: unknownEntities.length,
  });

  // ── Sample entities ───────────────────────────────────────────────────────
  if (vehicleEntities.length > 0) {
    logger.info('Sample VehiclePosition entities (first 3)', {
      samples: pickFirst(vehicleEntities).map(summariseEntity),
    });
  }

  if (tripUpdateEntities.length > 0) {
    logger.info('Sample TripUpdate entities (first 3)', {
      samples: pickFirst(tripUpdateEntities).map(summariseEntity),
    });
  }

  if (alertEntities.length > 0) {
    logger.info('Sample Alert entities (first 3)', {
      samples: pickFirst(alertEntities).map(summariseEntity),
    });
  }

  // ── Write output file ─────────────────────────────────────────────────────
  const output = {
    explored_at: new Date().toISOString(),
    feed_url: config.feedUrl,
    fetch_latency_ms: fetchMs,
    decode_latency_ms: decodeMs,
    payload_bytes: buffer.length,
    header: {
      gtfs_realtime_version: header.gtfs_realtime_version,
      incrementality: header.incrementality,
      timestamp_posix: header.timestamp,
      timestamp_utc: posixToIso(header.timestamp),
    },
    entity_counts: {
      total: entities.length,
      vehicle_positions: vehicleEntities.length,
      trip_updates: tripUpdateEntities.length,
      alerts: alertEntities.length,
      unknown: unknownEntities.length,
    },
    sample_vehicle_positions: pickFirst(vehicleEntities).map(summariseEntity),
    sample_trip_updates: pickFirst(tripUpdateEntities).map(summariseEntity),
    sample_alerts: pickFirst(alertEntities).map(summariseEntity),
  };

  const outputPath = 'feed-exploration-output.json';
  fs.writeFileSync(outputPath, JSON.stringify(output, null, 2), 'utf-8');
  logger.info(`Exploration output written`, { path: outputPath });
}

// Run
explore().catch((err) => {
  const logger = new Logger('GTFS-RT-Explorer');
  logger.error('Feed exploration failed', err);
  process.exit(1);
});
