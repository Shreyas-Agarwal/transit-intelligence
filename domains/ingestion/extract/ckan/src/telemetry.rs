//! Domain-specific OpenTelemetry metrics for the GTFS-S pipeline
//! (implementation plan Phase 7). Bootstrap — exporters, resource
//! attributes, the `tracing`-to-OpenTelemetry bridge — lives in
//! `ti_common::observability`; this module only defines what *this*
//! pipeline specifically wants to count, independent of where those numbers
//! end up going.
//!
//! Complements, rather than duplicates, the spans created directly with
//! `tracing::info_span!` elsewhere in this crate (`pipeline::run`,
//! `snapshot::run_stages`): a span answers "how long did this one
//! version/stage take, right now, in this run" and is naturally
//! hierarchical; a metric answers "how does this number behave in
//! aggregate, across many runs" and is what a dashboard alerts on. Per-stage
//! (download/extract/convert/publish) timing is recorded as spans only, not
//! also as histograms — one signal per fact, not two disagreeing ones.
//!
//! [`Metrics::new`] reads whatever `MeterProvider` is currently registered
//! globally (via `opentelemetry::global`), matching the same pattern
//! `ti_common::observability::init` itself uses to install that provider in
//! the first place. Everything downstream — `pipeline::run` and the
//! functions it calls — receives the resulting counters/histograms as an
//! explicit value, the same way `crate::concurrency::ResourcePermits` is
//! constructed once and threaded through by reference, rather than reaching
//! back into global state themselves.

use opentelemetry::metrics::{Counter, Histogram, UpDownCounter};

/// Cheap to clone: every field is itself a reference-counted handle to the
/// same underlying instrument (see each `opentelemetry::metrics` type's own
/// docs) — cloning `Metrics` to hand a copy to a closure is the intended way
/// to share it, not a workaround.
#[derive(Clone)]
pub struct Metrics {
    pub versions_discovered: Counter<u64>,
    pub versions_queued: Counter<u64>,
    pub versions_published: Counter<u64>,
    pub versions_failed: Counter<u64>,
    pub bytes_downloaded: Counter<u64>,
    pub stale_running_recovered: Counter<u64>,
    pub queue_wait_seconds: Histogram<f64>,
    pub version_duration_seconds: Histogram<f64>,
    pub active_workers: UpDownCounter<i64>,
}

impl Metrics {
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("ckan");
        Self {
            versions_discovered: meter
                .u64_counter("gtfs_s.versions.discovered")
                .with_description("GTFS-S resources CKAN listed, before the cutoff-version filter")
                .build(),
            versions_queued: meter
                .u64_counter("gtfs_s.versions.queued")
                .with_description("versions reconciliation determined actually need work this run")
                .build(),
            versions_published: meter
                .u64_counter("gtfs_s.versions.published")
                .with_description("versions successfully downloaded, converted, and published")
                .build(),
            versions_failed: meter
                .u64_counter("gtfs_s.versions.failed")
                .with_description("versions that failed at some stage; retried next run")
                .build(),
            bytes_downloaded: meter
                .u64_counter("gtfs_s.bytes_downloaded")
                .with_unit("By")
                .with_description("archive bytes actually transferred (excludes resumed downloads)")
                .build(),
            stale_running_recovered: meter
                .u64_counter("gtfs_s.recovery.stale_running_recovered")
                .with_description("versions found RUNNING at startup with no live owner, recovered")
                .build(),
            queue_wait_seconds: meter
                .f64_histogram("gtfs_s.queue.wait_seconds")
                .with_unit("s")
                .with_description("time a version spent enqueued before a worker picked it up")
                .build(),
            version_duration_seconds: meter
                .f64_histogram("gtfs_s.version.duration_seconds")
                .with_unit("s")
                .with_description("total processing time for one version, claim through complete")
                .build(),
            active_workers: meter
                .i64_up_down_counter("gtfs_s.workers.active")
                .with_description("versions currently being processed by a worker")
                .build(),
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
