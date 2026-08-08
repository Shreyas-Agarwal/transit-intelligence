//! Tracing setup shared by every ingestion binary.

/// Initializes a `tracing` subscriber that logs to stdout, honoring `RUST_LOG`
/// (defaulting to `info`) via `tracing_subscriber`'s env filter.
///
/// Safe to call once at the start of `main`. Calling it a second time (e.g. in
/// tests that also exercise `main`-like code) is a logic error in the caller, not
/// something this function tries to guard against — `tracing`'s global subscriber
/// can only be set once per process.
pub fn init() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
