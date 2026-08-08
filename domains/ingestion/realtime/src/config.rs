//! Runtime configuration for the GTFS-RT realtime ingestion worker, loaded
//! from the environment (see `.env.example` for the full list of variables).

use std::time::Duration;

use ti_common::config::{env_or, env_parsed_or, load_dotenv, require_env};
use ti_common::ConfigError;

pub struct RealtimeConfig {
    /// Combined GTFS-RT feed URL from Open Data Swiss.
    pub feed_url: String,
    /// Bearer token for the Open Data Swiss API Manager.
    pub feed_api_token: String,
    /// Poll interval (per ADR 0007: feed updates every 20-30 seconds).
    pub poll_interval: Duration,
    /// Redpanda / Kafka broker address list.
    pub redpanda_brokers: Vec<String>,
    /// Client identifier reported to Redpanda.
    pub client_id: String,
    /// Connect/request timeout applied to the feed HTTP client.
    pub feed_connect_timeout: Duration,
    pub feed_request_timeout: Duration,
}

impl RealtimeConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        load_dotenv();

        Ok(Self {
            feed_url: require_env("GTFS_RT_FEED_URL")?,
            feed_api_token: require_env("GTFS_RT_API_TOKEN")?,
            poll_interval: Duration::from_millis(env_parsed_or(
                "GTFS_RT_POLL_INTERVAL_MS",
                30_000,
            )?),
            redpanda_brokers: env_or("REDPANDA_BROKERS", "localhost:9092")
                .split(',')
                .map(str::to_string)
                .collect(),
            client_id: env_or("KAFKA_CLIENT_ID", "transit-ingestion-worker"),
            feed_connect_timeout: Duration::from_secs(env_parsed_or(
                "GTFS_RT_CONNECT_TIMEOUT_SECS",
                10,
            )?),
            feed_request_timeout: Duration::from_secs(env_parsed_or(
                "GTFS_RT_REQUEST_TIMEOUT_SECS",
                15,
            )?),
        })
    }
}
