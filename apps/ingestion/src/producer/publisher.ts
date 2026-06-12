/**
 * GTFS-RT → Redpanda Publisher — Sprint 02 Producer Prototype
 *
 * Pipeline:
 *   GTFS-RT feed (HTTP binary)
 *   → fetchFeedBuffer()
 *   → decodeFeedBuffer()        (protobuf → plain JS object)
 *   → buildMessages()           (wrap each entity in SnapshotRawMessage envelope)
 *   → producer.send()           (publish JSON strings to transit.snapshots.raw)
 *
 * What this does NOT do (by design — Sprint 02 is purely exploratory):
 *   - Aggregate entities
 *   - Transform fields or rename keys
 *   - Calculate metrics or delays
 *   - Filter by zone, route, or agency
 *   - Modify payload semantics in any way
 *
 * Message format is canonical JSON so that:
 *   rpk topic consume transit.snapshots.raw
 * produces immediately human-readable output.
 *
 * Pure message-building logic lives in message-builder.ts for unit testability.
 */

import { Logger } from '@transit-intelligence/shared-logger';
import { config } from '../config.js';
import { fetchFeedBuffer } from '../feed/fetcher.js';
import { decodeFeedBuffer } from '../feed/decoder.js';
import { createKafkaClient, createProducer, ensureTopics } from './client.js';
import { TOPICS } from './topics.js';
import { buildMessages } from './message-builder.js';

const logger = new Logger('GTFS-RT-Publisher');

// ---------------------------------------------------------------------------
// Poll and publish loop
// ---------------------------------------------------------------------------

/**
 * Runs a single fetch-decode-publish cycle.
 *
 * @param producer Connected KafkaJS producer.
 * @returns        Per-cycle metrics for structured logging.
 */
async function pollAndPublish(producer: ReturnType<typeof createProducer>): Promise<{
  entityCount: number;
  vehicleCount: number;
  tripUpdateCount: number;
  alertCount: number;
  publishedCount: number;
  fetchMs: number;
  decodeMs: number;
  publishMs: number;
}> {
  // ── Fetch ─────────────────────────────────────────────────────────────────
  const fetchStart = Date.now();
  const buffer = await fetchFeedBuffer(config.feedUrl, config.feedApiToken);
  logger.info('Feed buffer received', {
    payloadBytes: buffer.length,
    payloadMB: Number((buffer.length / 1024 / 1024).toFixed(2)),
  });
  const fetchMs = Date.now() - fetchStart;

  // ── Decode ────────────────────────────────────────────────────────────────
  const decodeStart = Date.now();
  const feed = await decodeFeedBuffer(buffer);
  const decodeMs = Date.now() - decodeStart;

  const entities = feed.entity ?? [];
  const feedTimestampIso = feed.header.timestamp
    ? new Date(feed.header.timestamp * 1000).toISOString()
    : new Date().toISOString();

  const vehicleCount = entities.filter((e) => !!e.vehicle).length;
  const tripUpdateCount = entities.filter((e) => !!e.trip_update).length;
  const alertCount = entities.filter((e) => !!e.alert).length;

  // ── Build messages ────────────────────────────────────────────────────────
  const messages = buildMessages(entities, feedTimestampIso, feed.header.gtfs_realtime_version);

  const firstMessageSize =
    messages.length > 0
      ? Buffer.byteLength(String(messages[0].value))
      : 0;

  const largestMessageSize = messages.reduce((max, msg) => {
    const size = Buffer.byteLength(String(msg.value));
    return Math.max(max, size);
  }, 0);

  logger.info('Message batch built', {
    entityCount: entities.length,
    messageCount: messages.length,
    firstMessageBytes: firstMessageSize,
    largestMessageBytes: largestMessageSize,
  });

  // ── Publish ───────────────────────────────────────────────────────────────
  const publishStart = Date.now();
  const BATCH_SIZE = 500;
  for (let i = 0; i < messages.length; i += BATCH_SIZE) {
    const batch = messages.slice(i, i + BATCH_SIZE);

    const batchBytes = batch.reduce(
      (sum, message) => sum + Buffer.byteLength(String(message.value)),
      0,
    );

    logger.info('Publishing batch', {
      batchSize: batch.length,
      batchBytes,
      batchMB: Number((batchBytes / 1024 / 1024).toFixed(2)),
    });


    await producer.send({
      topic: TOPICS.SNAPSHOTS_RAW,
      messages: batch,
    });
  }
  const publishMs = Date.now() - publishStart;

  return {
    entityCount: entities.length,
    vehicleCount,
    tripUpdateCount,
    alertCount,
    publishedCount: messages.length,
    fetchMs,
    decodeMs,
    publishMs,
  };
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  logger.info('GTFS-RT publisher starting', {
    feedUrl: config.feedUrl,
    pollIntervalMs: config.pollIntervalMs,
    brokers: config.redpandaBrokers,
  });

  const kafka = createKafkaClient();

  // Ensure all ADR 0008 topics exist before producing
  await ensureTopics(kafka);

  const producer = createProducer(kafka);
  await producer.connect();
  logger.info('Redpanda producer connected', { topic: TOPICS.SNAPSHOTS_RAW });

  // Graceful shutdown
  let running = true;
  const shutdown = async () => {
    logger.info('Shutting down producer...');
    running = false;
    await producer.disconnect();
    process.exit(0);
  };
  process.on('SIGINT', () => void shutdown());
  process.on('SIGTERM', () => void shutdown());

  // Poll loop
  let cycleCount = 0;
  while (running) {
    cycleCount++;
    logger.info(`Poll cycle ${cycleCount} starting`);

    try {
      const metrics = await pollAndPublish(producer);

      logger.info(`Poll cycle ${cycleCount} complete`, {
        cycle: cycleCount,
        entity_count: metrics.entityCount,
        vehicle_positions: metrics.vehicleCount,
        trip_updates: metrics.tripUpdateCount,
        alerts: metrics.alertCount,
        published_messages: metrics.publishedCount,
        fetch_ms: metrics.fetchMs,
        decode_ms: metrics.decodeMs,
        publish_ms: metrics.publishMs,
        total_ms: metrics.fetchMs + metrics.decodeMs + metrics.publishMs,
        topic: TOPICS.SNAPSHOTS_RAW,
      });
    } catch (err) {
      logger.error(`Poll cycle ${cycleCount} failed`, err);
      // Continue polling on error — do not crash the loop
    }

    logger.info(`Waiting ${config.pollIntervalMs}ms until next poll...`);
    await new Promise((resolve) => setTimeout(resolve, config.pollIntervalMs));
  }
}

main().catch((err) => {
  logger.error('Publisher failed to start', err);
  process.exit(1);
});
