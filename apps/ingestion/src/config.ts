import fs from 'node:fs';
import path from 'node:path';
import dotenv from 'dotenv';

// 1. Load from the current working directory (.env)
dotenv.config();

// 2. Fallback to the monorepo root (.env) if present
const rootEnv = path.resolve(__dirname, '../../../.env');
if (fs.existsSync(rootEnv)) {
  dotenv.config({ path: rootEnv });
}

/**
 * Ingestion worker configuration.
 *
 * All configuration is read from environment variables at startup.
 * Required variables will throw on missing values so the process fails fast
 * rather than producing silent bad behaviour.
 */

function requireEnv(key: string): string {
  const value = process.env[key];
  if (!value) {
    throw new Error(
      `Missing required environment variable: ${key}. ` +
        `Ensure your .env file is populated — see .env.example for reference.`,
    );
  }
  return value;
}

function optionalEnv(key: string, fallback: string): string {
  return process.env[key] ?? fallback;
}

export const config = {
  /** Combined GTFS-RT feed URL from Open Data Swiss */
  feedUrl: requireEnv('GTFS_RT_FEED_URL'),

  /** Bearer token for the Open Data Swiss API Manager */
  feedApiToken: requireEnv('GTFS_RT_API_TOKEN'),

  /** Poll interval in milliseconds (default: 30 000ms = 30 seconds) */
  pollIntervalMs: parseInt(optionalEnv('GTFS_RT_POLL_INTERVAL_MS', '30000'), 10),

  /** Redpanda / Kafka broker address list (comma-separated) */
  redpandaBrokers: optionalEnv('REDPANDA_BROKERS', 'localhost:9092').split(','),

  /** Kafka client identifier */
  kafkaClientId: optionalEnv('KAFKA_CLIENT_ID', 'transit-ingestion-worker'),
} as const;
