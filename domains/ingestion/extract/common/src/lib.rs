//! `ti-common` — shared building blocks for the `domains/ingestion` Rust workspace.
//!
//! Each ingestion crate (`ckan`, `realtime`, `service-alerts`) acquires feeds from
//! opentransportdata.swiss and needs the same handful of concerns: env-based config,
//! an HTTP client with sane defaults, CKAN-style token auth, retry-with-backoff, and
//! tracing setup. Those live here so they're implemented once.

pub mod auth;
pub mod config;
pub mod http;
pub mod logging;
pub mod observability;
pub mod retry;

pub use config::ConfigError;
