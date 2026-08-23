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
    /// Maximum number of GTFS versions that may be concurrently in-flight
    /// (download → extract → Parquet). Defaults to `min(4, available_parallelism)`.
    /// Overridable via `GTFS_S_MAX_CONCURRENT_VERSIONS`.
    ///
    /// Caps both network connections and simultaneous staging directories.
    /// Four concurrent versions consume roughly 1–1.5 GB of staging disk space
    /// and up to four CPU cores during parallel Parquet conversion. Reduce if
    /// either resource is constrained on the host running this binary.
    pub max_concurrent_versions: usize,
    /// Maximum number of eligible versions that may sit queued, waiting for
    /// a worker, before the scheduler blocks rather than accepting more
    /// (implementation plan Phase 3/4: `MAX_QUEUED_VERSIONS`, a bound
    /// independent of `max_concurrent_versions`). Overridable via
    /// `GTFS_S_MAX_QUEUED_VERSIONS`. Defaults to `2 * max_concurrent_versions`
    /// (floor 4): generous enough that a normal run's eligible set — a
    /// handful of versions between twice-weekly publications — never blocks
    /// in practice, while still being a genuine fixed ceiling rather than
    /// "however many happen to be eligible this run."
    pub max_queued_versions: usize,
    /// Maximum number of versions that may be *downloading* at once
    /// (implementation plan Phase 5: `MAX_CONCURRENT_DOWNLOADS`), independent
    /// of `max_concurrent_versions` (which still bounds how many versions are
    /// active overall, in any stage). Overridable via
    /// `GTFS_S_MAX_CONCURRENT_DOWNLOADS`. Defaults to `max_concurrent_versions`
    /// — i.e. no additional restriction beyond the pre-Phase-5 behavior,
    /// since this is a new knob for operators who want it, not a value with
    /// a measured "correct" default yet (see Phase 11).
    pub max_concurrent_downloads: usize,
    /// Maximum number of versions that may be *extracting or converting* at
    /// once (implementation plan Phase 5: `MAX_CONCURRENT_PROCESSING`) — one
    /// shared pool for both stages, since both are CPU/disk-heavy rather than
    /// network-heavy. Overridable via `GTFS_S_MAX_CONCURRENT_PROCESSING`.
    /// Defaults to `max_concurrent_versions`, for the same reason as
    /// `max_concurrent_downloads` above.
    pub max_concurrent_processing: usize,
    pub api_connect_timeout: Duration,
    pub api_request_timeout: Duration,
    pub download_connect_timeout: Duration,
    pub download_request_timeout: Duration,
}

impl CkanConfig {
    pub fn from_env() -> Result<Self, ti_common::ConfigError> {
        load_dotenv();

        let max_concurrent_versions = parse_max_concurrent(env_parsed_or(
            "GTFS_S_MAX_CONCURRENT_VERSIONS",
            0usize, // 0 = sentinel: compute the default
        )?)?;
        let max_queued_versions = parse_max_queued(
            env_parsed_or("GTFS_S_MAX_QUEUED_VERSIONS", 0usize)?,
            max_concurrent_versions,
        );
        let max_concurrent_downloads = parse_resource_permit_count(
            env_parsed_or("GTFS_S_MAX_CONCURRENT_DOWNLOADS", 0usize)?,
            max_concurrent_versions,
        );
        let max_concurrent_processing = parse_resource_permit_count(
            env_parsed_or("GTFS_S_MAX_CONCURRENT_PROCESSING", 0usize)?,
            max_concurrent_versions,
        );

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
            max_concurrent_versions,
            max_queued_versions,
            max_concurrent_downloads,
            max_concurrent_processing,
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

/// `CARGO_MANIFEST_DIR` for this crate is
/// `<repo_root>/domains/ingestion/extract/ckan`; four ancestors up is
/// `<repo_root>`. This is a compile-time constant, so the result is
/// independent of the process's working directory at runtime.
fn default_raw_dir() -> PathBuf {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("CARGO_MANIFEST_DIR is domains/ingestion/extract/ckan under the repo root");
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

/// When `raw` is 0 (the sentinel for "use the default"), returns
/// `min(4, available_parallelism)`. An explicit positive value is used
/// directly. Zero is rejected if explicitly set via the env var because the
/// caller passes the already-parsed value; the sentinel is only reachable
/// through the `env_parsed_or(…, 0usize)` default path.
fn parse_max_concurrent(raw: usize) -> Result<usize, ti_common::ConfigError> {
    if raw == 0 {
        // Default: cap at 4 unless the machine has fewer logical CPUs.
        let parallelism = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Ok(4_usize.min(parallelism).max(1))
    } else {
        Ok(raw)
    }
}

/// When `raw` is 0 (the sentinel for "use the default"), returns
/// `2 * max_concurrent` with a floor of 4 — generous enough that a normal
/// run's eligible set never blocks in practice, while still a genuine fixed
/// ceiling. An explicit positive value is used directly.
fn parse_max_queued(raw: usize, max_concurrent: usize) -> usize {
    if raw == 0 {
        max_concurrent.saturating_mul(2).max(4)
    } else {
        raw
    }
}

/// Shared by `GTFS_S_MAX_CONCURRENT_DOWNLOADS` and
/// `GTFS_S_MAX_CONCURRENT_PROCESSING`: when `raw` is 0 (the sentinel),
/// defaults to `default_to` (in practice, `max_concurrent_versions`) — an
/// operator who never sets either variable sees no change in observed
/// concurrency from before Phase 5. An explicit positive value is used
/// directly.
fn parse_resource_permit_count(raw: usize, default_to: usize) -> usize {
    if raw == 0 { default_to } else { raw }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_raw_dir_resolves_to_repo_root_data_bronze_static() {
        // CARGO_MANIFEST_DIR is domains/ingestion/extract/ckan; the repo root
        // is the directory containing `domains/`. This must hold regardless
        // of the test runner's own working directory.
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

    #[test]
    fn max_concurrent_sentinel_zero_gives_positive_default() {
        let n = parse_max_concurrent(0).unwrap();
        assert!(n >= 1, "default must be at least 1, got {n}");
        assert!(n <= 4, "default is capped at 4, got {n}");
    }

    #[test]
    fn max_concurrent_explicit_value_passes_through() {
        assert_eq!(parse_max_concurrent(8).unwrap(), 8);
        assert_eq!(parse_max_concurrent(1).unwrap(), 1);
    }

    #[test]
    fn max_queued_sentinel_zero_defaults_to_twice_max_concurrent() {
        assert_eq!(parse_max_queued(0, 3), 6);
    }

    #[test]
    fn max_queued_sentinel_zero_has_a_floor_of_four() {
        assert_eq!(parse_max_queued(0, 1), 4, "2 * 1 must still floor to 4");
    }

    #[test]
    fn max_queued_explicit_value_passes_through() {
        assert_eq!(parse_max_queued(10, 3), 10);
    }

    #[test]
    fn resource_permit_count_sentinel_zero_defaults_to_max_concurrent() {
        assert_eq!(parse_resource_permit_count(0, 4), 4);
    }

    #[test]
    fn resource_permit_count_explicit_value_passes_through() {
        assert_eq!(parse_resource_permit_count(2, 4), 2);
    }
}
