//! Shared OpenTelemetry-compatible observability bootstrap for ingestion
//! binaries.
//!
//! This module owns exactly three things: a `tracing` subscriber wired so
//! every span and event is also recorded as an OpenTelemetry trace, the
//! metrics pipeline domain crates publish numbers through, and the exporter
//! configuration and resource attributes both share. It does not know
//! anything about GTFS, CKAN, or any other domain — a crate that wants a
//! span for "downloading a snapshot" or a counter for "bytes downloaded"
//! creates it itself, using the ordinary `tracing` macros and the
//! `opentelemetry` metrics API respectively; this module only makes sure
//! that whatever it creates ends up somewhere.
//!
//! The application itself is not distributed, but the same trace model
//! OpenTelemetry uses for distributed systems is still useful for a single
//! process: nesting spans (an invocation contains a discovery step and a
//! processing step; processing contains one span per version; a version
//! contains a span per pipeline stage) gives a single navigable timeline of
//! one run, without inventing a bespoke format for that.
//!
//! # Why a guard, not just an init function
//!
//! This binary runs once per invocation and exits — it is not a long-lived
//! server. OpenTelemetry's exporters are built to batch and export on a
//! schedule, which assumes something is still running when that schedule
//! fires. Nothing here can assume that, so [`ObservabilityGuard`] flushes and
//! shuts both providers down itself, synchronously, when it is dropped —
//! which for a value held in `main` means "when the process is about to
//! exit," covering the success path and every early-return-via-`?` failure
//! path identically, because that's just how the local variable's scope
//! ends either way.

mod exporters;
mod resource;

#[cfg(feature = "observability-testing")]
pub mod testing;

pub use exporters::ExporterKind;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// Identifies the running binary in every span and metric this process
/// emits (OpenTelemetry's `service.name` / `service.version` resource
/// attributes).
pub struct ServiceInfo {
    pub name: &'static str,
    pub version: &'static str,
}

/// Owns both OpenTelemetry providers for the process's lifetime. See the
/// module doc comment for why dropping it (rather than a timer) is what
/// actually flushes telemetry to the configured exporter.
#[must_use = "dropping this immediately shuts observability back down; hold it for the process's lifetime (e.g. a `let` binding in `main`)"]
pub struct ObservabilityGuard {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
}

impl Drop for ObservabilityGuard {
    fn drop(&mut self) {
        if let Err(e) = self.tracer_provider.shutdown() {
            eprintln!("observability: failed to flush trace data on shutdown: {e}");
        }
        if let Err(e) = self.meter_provider.shutdown() {
            eprintln!("observability: failed to flush metric data on shutdown: {e}");
        }
    }
}

/// Initializes process-wide logging, tracing, and metrics: a human-readable
/// log on stdout (unchanged from what `logging::init` produced), plus an
/// OpenTelemetry layer that mirrors every `tracing` span into a trace and
/// exports it through `exporter`.
///
/// Calls `tracing_subscriber`'s global-subscriber init, so — like
/// `logging::init` — this may only be called once per process, and not
/// alongside `logging::init`; both attempt to install the same global.
pub fn init(service: ServiceInfo, exporter: ExporterKind) -> ObservabilityGuard {
    let resource = resource::build(&service);

    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_simple_exporter(exporters::span_exporter(exporter))
        .build();

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(exporters::metric_reader(exporter))
        .build();

    opentelemetry::global::set_tracer_provider(tracer_provider.clone());
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    let tracer = tracer_provider.tracer(service.name);
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(otel_layer)
        .init();

    ObservabilityGuard {
        tracer_provider,
        meter_provider,
    }
}

/// Marks `span` as having failed, with `message` as the reason — a shared
/// telemetry convention so every ingestion crate records failure on a span
/// the same way, through this one function, rather than each reaching for
/// `tracing_opentelemetry`'s span-status API (and its own idea of what
/// "failed" means) directly.
///
/// Takes an explicit `&tracing::Span` rather than using
/// `tracing::Span::current()`: a stage's span is often still current only
/// while its own future is being polled — by the time an `async` caller
/// observes the `Result` that future produced, `.instrument()` has already
/// exited that span. Marking failure needs a `Span` value the caller kept
/// alive across that boundary, not whatever happens to be ambient at the
/// call site.
pub fn mark_span_error(span: &tracing::Span, message: impl Into<String>) {
    use opentelemetry::trace::Status;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;
    span.set_status(Status::error(message.into()));
}
