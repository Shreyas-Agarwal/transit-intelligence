//! Shared HTTP client construction.

use std::time::Duration;

/// Builds a [`reqwest::Client`] with the defaults every ingestion crate wants:
/// a bounded connect timeout, a sane User-Agent (so opentransportdata.swiss can
/// identify our traffic in their logs), and gzip/brotli negotiation (enabled via
/// the `reqwest` feature flags at the workspace level).
///
/// `request_timeout` is deliberately caller-supplied rather than baked in here:
/// a CKAN metadata call and a multi-hundred-MB GTFS zip download have very
/// different reasonable timeouts.
pub fn build_client(
    connect_timeout: Duration,
    request_timeout: Duration,
) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .user_agent(concat!(
            "transit-intelligence-ingestion/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
}
