//! GTFS-RT realtime ingestion worker. Implements the pipeline described in
//! ADR 0007 (30s polling) and ADR 0008 (Redpanda as immutable snapshot
//! ledger): fetch the combined GTFS-RT feed, decode it, and publish each
//! entity to `transit.snapshots.raw`.
//!
//! Split into a library + thin binary (`main.rs`) so the pipeline stages are
//! independently testable without a live feed endpoint or Redpanda broker.

pub mod config;
pub mod fetcher;
pub mod proto;
