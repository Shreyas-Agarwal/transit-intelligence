//! Redpanda topic name constants — the canonical topic names established by
//! ADR 0008 (Adopt Redpanda as Immutable Temporal Snapshot Ledger). See
//! `docs/design/redpanda-topic-configuration.md` for the full topology.
//!
//! Only `SNAPSHOTS_RAW` is populated today; the remaining topics are defined
//! here for completeness and so string literals don't get scattered across
//! the codebase.

pub const SNAPSHOTS_RAW: &str = "transit.snapshots.raw";
pub const SNAPSHOTS_NORMALIZED: &str = "transit.snapshots.normalized";
pub const STATE_DELTAS: &str = "transit.state.deltas";
pub const METRICS_OPERATIONAL: &str = "transit.metrics.operational";

#[derive(Debug, Clone, Copy)]
pub struct TopicConfig {
    pub name: &'static str,
    pub num_partitions: i32,
    pub replication_factor: i16,
    /// Retention, in milliseconds.
    pub retention_ms: i64,
}

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Phase 1 uses 1 partition per topic intentionally — ADR 0008 explicitly
/// defers partitioning specialisation until replay patterns are validated
/// empirically.
pub const TOPIC_CONFIGS: &[TopicConfig] = &[
    TopicConfig {
        name: SNAPSHOTS_RAW,
        num_partitions: 1,
        replication_factor: 1,
        retention_ms: 7 * DAY_MS,
    },
    TopicConfig {
        name: SNAPSHOTS_NORMALIZED,
        num_partitions: 1,
        replication_factor: 1,
        retention_ms: 7 * DAY_MS,
    },
    TopicConfig {
        name: STATE_DELTAS,
        num_partitions: 1,
        replication_factor: 1,
        retention_ms: 7 * DAY_MS,
    },
    TopicConfig {
        name: METRICS_OPERATIONAL,
        num_partitions: 1,
        replication_factor: 1,
        // Longer retention for operational/analytics workloads.
        retention_ms: 30 * DAY_MS,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_raw_retains_seven_days() {
        let cfg = TOPIC_CONFIGS
            .iter()
            .find(|c| c.name == SNAPSHOTS_RAW)
            .unwrap();
        assert_eq!(cfg.retention_ms, 604_800_000);
        assert_eq!(cfg.num_partitions, 1);
        assert_eq!(cfg.replication_factor, 1);
    }

    #[test]
    fn metrics_operational_retains_thirty_days() {
        let cfg = TOPIC_CONFIGS
            .iter()
            .find(|c| c.name == METRICS_OPERATIONAL)
            .unwrap();
        assert_eq!(cfg.retention_ms, 2_592_000_000);
    }

    #[test]
    fn every_phase_1_topic_has_a_config() {
        let names: Vec<&str> = TOPIC_CONFIGS.iter().map(|c| c.name).collect();
        assert_eq!(
            names,
            vec![
                SNAPSHOTS_RAW,
                SNAPSHOTS_NORMALIZED,
                STATE_DELTAS,
                METRICS_OPERATIONAL
            ]
        );
    }
}
