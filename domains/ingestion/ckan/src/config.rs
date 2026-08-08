//! Runtime configuration for the `ckan` downloader, loaded from the environment
//! (see `.env.example` for the full list of variables this reads).

use std::path::{Path, PathBuf};
use std::time::Duration;

use ti_common::auth::TokenCredentials;
use ti_common::config::{env_or, env_parsed_or, load_dotenv, require_env};

use crate::domain::VersionId;

pub struct CkanConfig {
    pub ckan_api_url: String,
    pub dataset_id: String,
    pub credentials: TokenCredentials,
    /// `<repo_root>/data/bronze/static` — where snapshots are extracted to.
    /// Overridable via `GTFS_S_RAW_DIR` (tests point it at a temp directory).
    /// The default is computed from `CARGO_MANIFEST_DIR` rather than the
    /// process's current working directory, specifically so it resolves to the
    /// same place regardless of where `ckan` is invoked from (`cargo run` from
    /// the repo root vs. from `domains/ingestion` vs. a packaged binary run
    /// from anywhere) — a relative `./data/bronze/static` default would silently
    /// land in the wrong place depending on the caller's cwd.
    pub raw_dir: PathBuf,
    /// Versions older than this are ignored during discovery, as if upstream
    /// never published them (design doc Non-goals: "we start tracking from
    /// whatever we adopt as our first automated pull, plus whatever's already
    /// on disk" — this is what encodes that starting point, rather than
    /// backfilling every historical snapshot the CKAN catalog still lists).
    /// Overridable via `GTFS_S_CUTOFF_VERSION` (`YYYYMMDD`); set it to an empty
    /// string to disable the cutoff and consider every version upstream lists.
    pub cutoff_version: Option<VersionId>,
    pub api_connect_timeout: Duration,
    pub api_request_timeout: Duration,
    pub download_connect_timeout: Duration,
    pub download_request_timeout: Duration,
}

impl CkanConfig {
    pub fn from_env() -> Result<Self, ti_common::ConfigError> {
        load_dotenv();

        Ok(Self {
            ckan_api_url: env_or(
                "GTFS_S_CKAN_API_URL",
                "https://api.opentransportdata.swiss/ckan-api",
            ),
            dataset_id: require_env("GTFS_S_CKAN_DATASET_ID")?,
            credentials: TokenCredentials::new(
                require_env("GTFS_S_CKAN_API_TOKEN")?,
                require_env("GTFS_S_CKAN_API_TOKEN_HASH")?,
            ),
            raw_dir: std::env::var("GTFS_S_RAW_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| default_raw_dir()),
            cutoff_version: parse_cutoff_version(&env_or("GTFS_S_CUTOFF_VERSION", "20260101"))?,
            api_connect_timeout: Duration::from_secs(env_parsed_or(
                "GTFS_S_CKAN_API_CONNECT_TIMEOUT_SECS",
                10,
            )?),
            api_request_timeout: Duration::from_secs(env_parsed_or(
                "GTFS_S_CKAN_API_REQUEST_TIMEOUT_SECS",
                30,
            )?),
            download_connect_timeout: Duration::from_secs(env_parsed_or(
                "GTFS_S_DOWNLOAD_CONNECT_TIMEOUT_SECS",
                10,
            )?),
            // GTFS-S archives are hundreds of MB; a generous ceiling avoids
            // spuriously killing a slow-but-healthy transfer.
            download_request_timeout: Duration::from_secs(env_parsed_or(
                "GTFS_S_DOWNLOAD_REQUEST_TIMEOUT_SECS",
                1800,
            )?),
        })
    }
}

/// `CARGO_MANIFEST_DIR` for this crate is `<repo_root>/domains/ingestion/ckan`;
/// three ancestors up is `<repo_root>`. This is a compile-time constant, so the
/// result is independent of the process's working directory at runtime.
fn default_raw_dir() -> PathBuf {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("CARGO_MANIFEST_DIR is domains/ingestion/ckan under the repo root");
    repo_root.join("data/bronze/static")
}

fn parse_cutoff_version(raw: &str) -> Result<Option<VersionId>, ti_common::ConfigError> {
    if raw.is_empty() {
        return Ok(None);
    }
    VersionId::parse(raw)
        .map(Some)
        .map_err(|e| ti_common::ConfigError::InvalidVar {
            name: "GTFS_S_CUTOFF_VERSION".to_string(),
            value: raw.to_string(),
            reason: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_raw_dir_resolves_to_repo_root_data_bronze_static() {
        // CARGO_MANIFEST_DIR is domains/ingestion/ckan; the repo root is the
        // directory containing `domains/`. This must hold regardless of the
        // test runner's own working directory.
        let dir = default_raw_dir();
        assert!(dir.ends_with("data/bronze/static"), "{}", dir.display());
        assert!(
            dir.to_string_lossy().contains("transit-intelligence"),
            "expected the repo root, got {}",
            dir.display()
        );
        assert!(
            !dir.to_string_lossy().contains("domains/ingestion"),
            "must not still be under domains/ingestion: {}",
            dir.display()
        );
    }

    #[test]
    fn cutoff_version_empty_string_disables_it() {
        assert!(parse_cutoff_version("").unwrap().is_none());
    }

    #[test]
    fn cutoff_version_parses_a_valid_date() {
        assert_eq!(
            parse_cutoff_version("20260101").unwrap(),
            Some(VersionId::parse("20260101").unwrap())
        );
    }

    #[test]
    fn cutoff_version_rejects_garbage() {
        assert!(parse_cutoff_version("not-a-date").is_err());
    }
}
