//! Where a run's spans and metrics actually go. This is the one place that
//! would need a new match arm to add a different destination (e.g. an OTLP
//! collector) later — every span, event, and metric instrument elsewhere in
//! the codebase is written against `tracing`/`opentelemetry`'s own APIs, not
//! against a specific exporter, so none of it would need to change.
//!
//! Only `Stdout` exists today. It's what local development and Phase 8's
//! benchmark runs actually need; a real remote backend is a deliberately
//! deferred decision, not an oversight — see the implementation log.

use std::time::Duration;

use opentelemetry_sdk::metrics::PeriodicReader;

/// Long enough that the periodic export timer never fires on its own during
/// a normal invocation of this short-lived CLI — final export happens once,
/// deterministically, at shutdown (`ObservabilityGuard::drop`), not in a race
/// against process exit.
const METRIC_EXPORT_INTERVAL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExporterKind {
    Stdout,
}

impl ExporterKind {
    /// Only `Stdout` exists today, so this always resolves to it. Kept as a
    /// named constructor (rather than every call site writing
    /// `ExporterKind::Stdout` directly) so that when a second destination is
    /// added and needs to be selected somehow — an env var, most likely —
    /// this one function is where that selection logic goes.
    pub fn from_env() -> Self {
        ExporterKind::Stdout
    }
}

pub(super) fn span_exporter(kind: ExporterKind) -> opentelemetry_stdout::SpanExporter {
    match kind {
        ExporterKind::Stdout => opentelemetry_stdout::SpanExporter::default(),
    }
}

pub(super) fn metric_reader(
    kind: ExporterKind,
) -> PeriodicReader<opentelemetry_stdout::MetricExporter> {
    match kind {
        ExporterKind::Stdout => {
            PeriodicReader::builder(opentelemetry_stdout::MetricExporter::default())
                .with_interval(METRIC_EXPORT_INTERVAL)
                .build()
        }
    }
}
