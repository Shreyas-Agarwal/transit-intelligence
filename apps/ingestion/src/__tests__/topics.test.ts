import { describe, it, expect } from 'vitest';
import { TOPICS, TOPIC_CONFIGS } from '../producer/topics.js';

/**
 * Unit tests for Redpanda topic constants and configuration.
 *
 * These tests encode the ADR 0008 topic topology as machine-verifiable
 * assertions. If the topic names or configuration values ever drift from
 * the ADR, these tests fail — providing an early warning before any
 * message routing breaks.
 */

describe('TOPICS constants', () => {
  it('defines the four ADR 0008 topic names', () => {
    // These exact strings are what gets created in Redpanda and consumed by
    // downstream services. Changing them is a breaking change.
    expect(TOPICS.SNAPSHOTS_RAW).toBe('transit.snapshots.raw');
    expect(TOPICS.SNAPSHOTS_NORMALIZED).toBe('transit.snapshots.normalized');
    expect(TOPICS.STATE_DELTAS).toBe('transit.state.deltas');
    expect(TOPICS.METRICS_OPERATIONAL).toBe('transit.metrics.operational');
  });

  it('contains exactly four topics (no undocumented additions)', () => {
    expect(Object.keys(TOPICS)).toHaveLength(4);
  });

  it('all topic names follow the transit.* namespace convention', () => {
    Object.values(TOPICS).forEach((name) => {
      expect(name).toMatch(/^transit\./);
    });
  });

  it('all topic names use dot-separated lowercase segments', () => {
    Object.values(TOPICS).forEach((name) => {
      // Must be lowercase letters and dots only — no hyphens or underscores
      expect(name).toMatch(/^[a-z.]+$/);
    });
  });
});

describe('TOPIC_CONFIGS', () => {
  it('provides a configuration entry for every defined topic', () => {
    Object.values(TOPICS).forEach((topicName) => {
      expect(TOPIC_CONFIGS).toHaveProperty(topicName);
    });
  });

  it('sets 1 partition per topic for Phase 1 (ADR 0008: defer partitioning specialisation)', () => {
    Object.values(TOPICS).forEach((topicName) => {
      expect(TOPIC_CONFIGS[topicName].numPartitions).toBe(1);
    });
  });

  it('sets replication factor of 1 (single-node local dev)', () => {
    Object.values(TOPICS).forEach((topicName) => {
      expect(TOPIC_CONFIGS[topicName].replicationFactor).toBe(1);
    });
  });

  it('retains transit.snapshots.raw for 7 days', () => {
    const sevenDaysMs = 7 * 24 * 60 * 60 * 1000;
    expect(TOPIC_CONFIGS[TOPICS.SNAPSHOTS_RAW].retentionMs).toBe(sevenDaysMs);
  });

  it('retains transit.metrics.operational for 30 days (longer than raw snapshots)', () => {
    const thirtyDaysMs = 30 * 24 * 60 * 60 * 1000;
    expect(TOPIC_CONFIGS[TOPICS.METRICS_OPERATIONAL].retentionMs).toBe(thirtyDaysMs);
    expect(TOPIC_CONFIGS[TOPICS.METRICS_OPERATIONAL].retentionMs).toBeGreaterThan(
      TOPIC_CONFIGS[TOPICS.SNAPSHOTS_RAW].retentionMs,
    );
  });

  it('all retention values are positive integers', () => {
    Object.values(TOPICS).forEach((topicName) => {
      const retention = TOPIC_CONFIGS[topicName].retentionMs;
      expect(retention).toBeGreaterThan(0);
      expect(Number.isInteger(retention)).toBe(true);
    });
  });
});
