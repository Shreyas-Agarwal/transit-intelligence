/**
 * Redpanda topic name constants.
 *
 * These are the canonical topic names established by ADR 0008
 * (Adopt Redpanda as Immutable Temporal Snapshot Ledger).
 *
 * Sprint 02 publishes only to SNAPSHOTS_RAW.
 * The remaining topics are defined here for completeness and to avoid
 * string literals being scattered across the codebase.
 */
export const TOPICS = {
  /** Raw decoded GTFS-RT snapshot payloads — primary target for Sprint 02 */
  SNAPSHOTS_RAW: 'transit.snapshots.raw',

  /**
   * Cleaned and structurally validated operational state snapshots.
   * Populated by downstream normalisation consumers in later sprints.
   */
  SNAPSHOTS_NORMALIZED: 'transit.snapshots.normalized',

  /**
   * Computed state transitions derived between consecutive snapshots.
   * Populated by downstream delta consumers in later sprints.
   */
  STATE_DELTAS: 'transit.state.deltas',

  /**
   * Derived observability and resilience metrics.
   * Populated by downstream metric consumers in later sprints.
   */
  METRICS_OPERATIONAL: 'transit.metrics.operational',
} as const;

export type TopicName = (typeof TOPICS)[keyof typeof TOPICS];

/**
 * Topic configuration for admin creation.
 *
 * Sprint 02 uses 1 partition per topic — ADR 0008 explicitly defers
 * partitioning specialisation until operational replay patterns are
 * validated empirically.
 *
 * Retention: 7 days for raw snapshots (168 hours × 3600 × 1000 ms).
 */
export const TOPIC_CONFIGS: Record<
  TopicName,
  { numPartitions: number; replicationFactor: number; retentionMs: number }
> = {
  [TOPICS.SNAPSHOTS_RAW]: {
    numPartitions: 1,
    replicationFactor: 1,
    retentionMs: 7 * 24 * 60 * 60 * 1000, // 7 days
  },
  [TOPICS.SNAPSHOTS_NORMALIZED]: {
    numPartitions: 1,
    replicationFactor: 1,
    retentionMs: 7 * 24 * 60 * 60 * 1000,
  },
  [TOPICS.STATE_DELTAS]: {
    numPartitions: 1,
    replicationFactor: 1,
    retentionMs: 7 * 24 * 60 * 60 * 1000,
  },
  [TOPICS.METRICS_OPERATIONAL]: {
    numPartitions: 1,
    replicationFactor: 1,
    retentionMs: 30 * 24 * 60 * 60 * 1000, // 30 days for metrics
  },
};
