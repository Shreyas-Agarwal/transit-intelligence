//! In-memory observability for tests that need to assert on what was
//! actually recorded — span names, parent/child nesting, metric values —
//! rather than only on the side effects those spans and metrics happen to
//! accompany.
//!
//! Unlike [`super::init`], this does *not* install a process-global
//! subscriber (`tracing`'s global subscriber can only be set once per
//! process, and many tests share one). Instead each test gets its own
//! `Subscriber` value to scope with `tracing::subscriber::set_default`, and
//! its own pair of in-memory exporters to read back from afterward.
//!
//! **This only captures spans created on the same OS thread that called
//! `set_default`.** `set_default` sets a thread-local, not a process-global,
//! default — a `tokio::spawn`ed task polled on a *different* thread creates
//! its spans against whatever the ambient default is on *that* thread
//! (typically none, so they're silently dropped, not misattributed). Under a
//! current-thread Tokio runtime (the default for `#[tokio::test]`, and the
//! only kind these crates' tests use) there is only one OS thread to poll
//! anything on, so this is never an issue; a genuinely multi-threaded
//! runtime would need every task's future to carry its own span context
//! explicitly (`tracing::Instrument`) all the way down, since the thread a
//! given poll happens to land on can't be relied on to have the right
//! thread-local set.

use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use tracing_subscriber::layer::SubscriberExt as _;

/// The exporters a test reads recorded spans/metrics back from, plus the
/// providers that must be flushed before that data is guaranteed visible.
pub struct InMemoryObservability {
    pub spans: InMemorySpanExporter,
    pub metrics: InMemoryMetricExporter,
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
}

impl InMemoryObservability {
    /// Forces every buffered span and metric to actually land in `spans` /
    /// `metrics`. Call this before asserting — otherwise a test is racing
    /// the exporter rather than observing what it recorded.
    pub fn flush(&self) {
        let _ = self.tracer_provider.force_flush();
        let _ = self.meter_provider.force_flush();
    }
}

/// Builds an in-memory tracing/metrics pipeline and the `Subscriber` that
/// feeds it. Typical use — `set_default`, not `with_default`, because the
/// exercised code is async and the subscriber must stay current across
/// `.await` points, not just for the synchronous instant a closure runs:
///
/// ```ignore
/// let (otel, subscriber) = ti_common::observability::testing::init("ckan-test");
/// let _guard = tracing::subscriber::set_default(subscriber);
/// // ... exercise instrumented async code here, including tasks spawned on
/// // a current-thread `#[tokio::test]` runtime — they run on this same
/// // thread, so this thread-local default reaches them too ...
/// drop(_guard);
/// otel.flush();
/// let spans = otel.spans.get_finished_spans().unwrap();
/// ```
pub fn init(service_name: &'static str) -> (InMemoryObservability, impl tracing::Subscriber) {
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();

    let metric_exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(metric_exporter.clone()).build();
    let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();

    let tracer = opentelemetry::trace::TracerProvider::tracer(&tracer_provider, service_name);
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let subscriber = tracing_subscriber::registry().with(otel_layer);

    (
        InMemoryObservability {
            spans: span_exporter,
            metrics: metric_exporter,
            tracer_provider,
            meter_provider,
        },
        subscriber,
    )
}
