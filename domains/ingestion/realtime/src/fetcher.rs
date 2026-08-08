//! Fetches the binary GTFS-RT protobuf payload from the feed URL.
//!
//! Open Data Swiss requires a Bearer token in the `Authorization` header.
//! The token is only ever attached to requests against
//! `opentransportdata.swiss` hosts — `reqwest`'s default redirect policy
//! already strips `Authorization` when a redirect crosses to a different
//! host, so a redirect to a third party never leaks the token.

use anyhow::{Context, Result, bail};

/// Fetches the raw protobuf body from `feed_url`, attaching the bearer token
/// only when the target host is `opentransportdata.swiss` (or a subdomain).
pub async fn fetch_feed_buffer(
    client: &reqwest::Client,
    feed_url: &str,
    api_token: &str,
) -> Result<Vec<u8>> {
    let url = feed_url
        .parse::<reqwest::Url>()
        .with_context(|| format!("invalid feed URL: {feed_url}"))?;

    let mut request = client
        .get(url.clone())
        .header("Accept", "application/x-protobuf");

    if url
        .host_str()
        .is_some_and(|host| host.ends_with("opentransportdata.swiss"))
    {
        request = request.bearer_auth(api_token);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("feed request failed for {feed_url}"))?;

    let status = response.status();
    if !status.is_success() {
        bail!("feed HTTP error: {status} from {feed_url}");
    }

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("failed to read feed response body from {feed_url}"))?;

    Ok(bytes.to_vec())
}
