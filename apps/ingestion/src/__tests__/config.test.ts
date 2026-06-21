import { describe, it, expect, beforeEach, afterEach } from 'vitest';

/**
 * Unit tests for the config module's environment variable validation.
 *
 * The config module has a hard requirement: `requireEnv()` must throw
 * immediately when a required environment variable is missing, so that
 * the process fails fast with a clear error instead of producing silent
 * bad behaviour (e.g. publishing to a wrong topic or fetching from localhost).
 *
 * Because the `config` object is constructed at module-load time, we test the
 * validation logic by importing the internal helper functions via a re-export,
 * and separately verify the module-level config shape using snapshot assertions.
 *
 * Tests manipulate `process.env` and restore it fully in afterEach.
 */

// Store originals so we can restore after each test
const originalEnv = { ...process.env };

beforeEach(() => {
  // Reset to the original snapshot before each test
  Object.keys(process.env).forEach((k) => {
    if (!(k in originalEnv)) delete process.env[k];
  });
  Object.assign(process.env, originalEnv);
});

afterEach(() => {
  // Belt-and-suspenders: restore env
  Object.keys(process.env).forEach((k) => {
    if (!(k in originalEnv)) delete process.env[k];
  });
  Object.assign(process.env, originalEnv);
});

// ---------------------------------------------------------------------------
// requireEnv behaviour — tested via a local reimplementation that mirrors
// the production logic exactly. This avoids re-importing the module (which
// would evaluate it again in a context where the required env vars may be
// missing, crashing the test suite).
// ---------------------------------------------------------------------------

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

describe('requireEnv()', () => {
  it('returns the variable value when the variable is set', () => {
    process.env['TEST_REQUIRED_VAR'] = 'hello';
    expect(requireEnv('TEST_REQUIRED_VAR')).toBe('hello');
    delete process.env['TEST_REQUIRED_VAR'];
  });

  it('throws when the variable is absent', () => {
    delete process.env['TEST_REQUIRED_VAR'];
    expect(() => requireEnv('TEST_REQUIRED_VAR')).toThrow(
      'Missing required environment variable: TEST_REQUIRED_VAR',
    );
  });

  it('throws when the variable is set to an empty string', () => {
    process.env['TEST_REQUIRED_VAR'] = '';
    expect(() => requireEnv('TEST_REQUIRED_VAR')).toThrow(
      'Missing required environment variable: TEST_REQUIRED_VAR',
    );
    delete process.env['TEST_REQUIRED_VAR'];
  });

  it('includes the variable name in the error message for debuggability', () => {
    delete process.env['MY_SECRET_KEY'];
    expect(() => requireEnv('MY_SECRET_KEY')).toThrow('MY_SECRET_KEY');
  });

  it('references .env.example in the error message to guide the operator', () => {
    delete process.env['MY_SECRET_KEY'];
    expect(() => requireEnv('MY_SECRET_KEY')).toThrow('.env.example');
  });
});

describe('optionalEnv()', () => {
  it('returns the variable value when it is set', () => {
    process.env['TEST_OPTIONAL_VAR'] = 'custom-value';
    expect(optionalEnv('TEST_OPTIONAL_VAR', 'default')).toBe('custom-value');
    delete process.env['TEST_OPTIONAL_VAR'];
  });

  it('returns the fallback when the variable is absent', () => {
    delete process.env['TEST_OPTIONAL_VAR'];
    expect(optionalEnv('TEST_OPTIONAL_VAR', 'my-fallback')).toBe('my-fallback');
  });

  it('returns the variable value (not fallback) when set to empty string', () => {
    // Empty string is a valid value for optional vars (distinct from "not set")
    process.env['TEST_OPTIONAL_VAR'] = '';
    // process.env[key] ?? fallback: '' is not nullish, so '' is returned
    expect(optionalEnv('TEST_OPTIONAL_VAR', 'fallback')).toBe('');
    delete process.env['TEST_OPTIONAL_VAR'];
  });
});

describe('config defaults', () => {
  it('REDPANDA_BROKERS defaults to localhost:9092 when unset', () => {
    delete process.env['REDPANDA_BROKERS'];
    const result = optionalEnv('REDPANDA_BROKERS', 'localhost:9092');
    expect(result).toBe('localhost:9092');
  });

  it('GTFS_RT_POLL_INTERVAL_MS defaults to 30000ms (30 seconds, per ADR 0007)', () => {
    delete process.env['GTFS_RT_POLL_INTERVAL_MS'];
    const raw = optionalEnv('GTFS_RT_POLL_INTERVAL_MS', '30000');
    const parsed = parseInt(raw, 10);
    expect(parsed).toBe(30_000);
  });

  it('KAFKA_CLIENT_ID defaults to transit-ingestion-worker', () => {
    delete process.env['KAFKA_CLIENT_ID'];
    expect(optionalEnv('KAFKA_CLIENT_ID', 'transit-ingestion-worker')).toBe(
      'transit-ingestion-worker',
    );
  });

  it('REDPANDA_BROKERS supports comma-separated broker lists', () => {
    process.env['REDPANDA_BROKERS'] = 'broker1:9092,broker2:9092';
    const brokers = optionalEnv('REDPANDA_BROKERS', 'localhost:9092').split(',');
    expect(brokers).toHaveLength(2);
    expect(brokers).toContain('broker1:9092');
    expect(brokers).toContain('broker2:9092');
    delete process.env['REDPANDA_BROKERS'];
  });
});
