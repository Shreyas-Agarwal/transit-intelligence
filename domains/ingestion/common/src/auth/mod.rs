//! Token-based auth for opentransportdata.swiss APIs.
//!
//! Confirmed against the live CKAN API (`package_show`): a bearer token alone
//! is sufficient. See `docs/design/gtfs-static-auto-downloader.md` §1 for the
//! full picture, including the token hash's status.

use reqwest::RequestBuilder;

/// A token + token-hash credential pair, as issued by opentransportdata.swiss.
/// The token hash is retained on the struct (and required at config load time,
/// since it's issued alongside the token) but isn't sent by [`Self::apply`] —
/// the CKAN API didn't require it.
#[derive(Clone)]
pub struct TokenCredentials {
    token: String,
    #[allow(dead_code)]
    token_hash: String,
}

impl TokenCredentials {
    pub fn new(token: impl Into<String>, token_hash: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            token_hash: token_hash.into(),
        }
    }

    /// Attaches the bearer token to an outgoing request.
    pub fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        request.bearer_auth(&self.token)
    }
}
