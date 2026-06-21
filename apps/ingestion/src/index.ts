/**
 * @transit-intelligence/ingestion
 *
 * Entry point for the GTFS-RT ingestion worker.
 *
 * In Sprint 02 (exploratory), this module is not the primary execution target.
 * Use the individual scripts instead:
 *
 *   Feed exploration:  node dist/feed/explorer.js
 *   Producer loop:     node dist/producer/publisher.js
 *
 * This file exists to satisfy the monorepo build system and to provide a
 * future composition point when the ingestion service is promoted to a
 * long-running process with proper lifecycle management.
 */

export { fetchFeedBuffer } from './feed/fetcher.js';
export { decodeFeedBuffer } from './feed/decoder.js';
export { TOPICS, TOPIC_CONFIGS } from './producer/topics.js';
export { createKafkaClient, createProducer, ensureTopics } from './producer/client.js';
export { deriveKey, entityType, buildMessages } from './producer/message-builder.js';
export { config } from './config.js';
export type * from './types/gtfs-rt.js';
