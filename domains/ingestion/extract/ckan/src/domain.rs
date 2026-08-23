//! Core domain types: version identity and upstream resource descriptions.

use std::fmt;

/// A normalized snapshot version identifier: an 8-digit `YYYYMMDD` date string.
///
/// Per the design doc §1, this is normalized at parse time so downstream code
/// never has to deal with the two upstream filename conventions
/// (`GTFS_FP2026_20260805.zip` vs the legacy `GTFS_FP2026_2025-09-22.zip`).
/// `YYYYMMDD` strings sort lexicographically in the same order as chronologically,
/// so `VersionId`'s derived `Ord` is exactly "chronological order" — this is what
/// lets the manifest and pipeline pick "newest" with a plain `max()`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct VersionId(String);

impl VersionId {
    /// Parses a `YYYYMMDD` string, rejecting anything that isn't exactly 8 ASCII digits.
    pub fn parse(raw: &str) -> Result<Self, VersionIdError> {
        if raw.len() == 8 && raw.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Self(raw.to_string()))
        } else {
            Err(VersionIdError::NotEightDigits(raw.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VersionIdError {
    #[error("version id {0:?} is not an 8-digit YYYYMMDD string")]
    NotEightDigits(String),
}

/// One GTFS-S zip resource as discovered from the CKAN catalog, before anything
/// has been downloaded.
#[derive(Debug, Clone)]
pub struct UpstreamResource {
    pub version: VersionId,
    /// The lowercased filename stem prefix before the date component, e.g.
    /// `"gtfs_fp2026"` for `GTFS_FP2026_20260805.zip`. Used to derive the final
    /// snapshot directory name (`gtfs_fp2026_20260805`), matching the naming the
    /// design doc's examples use, without hardcoding a Fahrplan year that will
    /// change in future dataset revisions.
    pub name_prefix: String,
    pub download_url: String,
    /// The original filename as published, kept for diagnostics/logging.
    pub original_filename: String,
    /// From the CKAN resource's own `last_modified` field (catalog metadata,
    /// not an HTTP response header).
    pub publisher_last_modified: Option<String>,
    /// From the CKAN resource's `hash` field, if the publisher set one.
    /// Algorithm is unspecified by the API; only compared against our own
    /// `archive_sha256` when it's plausibly a hex SHA-256 (64 hex chars) — see
    /// [`verify_upstream_hash`].
    pub upstream_hash: Option<String>,
}

impl UpstreamResource {
    /// The directory name this resource will be extracted under: `<prefix>_<version>`.
    pub fn snapshot_dir_name(&self) -> String {
        format!("{}_{}", self.name_prefix, self.version.as_str())
    }
}

/// Parses a GTFS-S resource filename into (name_prefix, `VersionId`).
///
/// Handles both naming conventions documented in the runbook:
/// * `GTFS_FP2026_20260805.zip` (current)
/// * `GTFS_FP2026_2025-09-22.zip` (legacy, hyphenated date)
///
/// The date is always the last underscore-separated segment before the extension;
/// everything before it, lowercased, becomes the name prefix.
pub fn parse_resource_filename(filename: &str) -> Result<(String, VersionId), VersionIdError> {
    let stem = filename
        .strip_suffix(".zip")
        .or_else(|| filename.strip_suffix(".ZIP"))
        .unwrap_or(filename);

    let (prefix, date_part) = stem
        .rsplit_once('_')
        .ok_or_else(|| VersionIdError::NotEightDigits(filename.to_string()))?;

    let normalized_date = date_part.replace('-', "");
    let version = VersionId::parse(&normalized_date)
        .map_err(|_| VersionIdError::NotEightDigits(filename.to_string()))?;

    Ok((prefix.to_lowercase(), version))
}

/// Compares a downloaded archive's computed SHA-256 against the CKAN
/// resource's `hash` field, when that field looks like a hex SHA-256 (64 hex
/// chars). Other lengths/algorithms are logged and otherwise ignored — we
/// can't verify a checksum we don't know the algorithm for.
pub fn verify_upstream_hash(
    upstream_hash: Option<&str>,
    computed_sha256: &str,
) -> Result<(), String> {
    let Some(hash) = upstream_hash else {
        return Ok(());
    };
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        tracing::debug!(
            hash,
            "upstream resource hash is not a recognizable SHA-256; skipping comparison"
        );
        return Ok(());
    }
    if hash.eq_ignore_ascii_case(computed_sha256) {
        Ok(())
    } else {
        Err(format!(
            "upstream hash {hash} does not match computed sha256 {computed_sha256}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_naming_convention() {
        let (prefix, version) = parse_resource_filename("GTFS_FP2026_20260805.zip").unwrap();
        assert_eq!(prefix, "gtfs_fp2026");
        assert_eq!(version.as_str(), "20260805");
    }

    #[test]
    fn parses_legacy_hyphenated_naming_convention() {
        let (prefix, version) = parse_resource_filename("GTFS_FP2026_2025-09-22.zip").unwrap();
        assert_eq!(prefix, "gtfs_fp2026");
        assert_eq!(version.as_str(), "2025-09-22".replace('-', ""));
        assert_eq!(version.as_str(), "20250922");
    }

    #[test]
    fn rejects_non_date_suffix() {
        assert!(parse_resource_filename("GTFS_FP2026_README.zip").is_err());
    }

    #[test]
    fn version_ids_sort_chronologically() {
        let older = VersionId::parse("20260729").unwrap();
        let newer = VersionId::parse("20260805").unwrap();
        assert!(older < newer);
    }

    #[test]
    fn verify_upstream_hash_passes_when_no_hash_was_published() {
        assert!(verify_upstream_hash(None, "anything").is_ok());
    }

    #[test]
    fn verify_upstream_hash_passes_when_hashes_match_case_insensitively() {
        let lower = "a".repeat(64);
        let upper = lower.to_uppercase();
        assert!(verify_upstream_hash(Some(&upper), &lower).is_ok());
    }

    #[test]
    fn verify_upstream_hash_fails_when_hashes_differ() {
        let published = "a".repeat(64);
        let computed = "b".repeat(64);
        let err = verify_upstream_hash(Some(&published), &computed).unwrap_err();
        assert!(err.contains(&published) && err.contains(&computed));
    }

    #[test]
    fn verify_upstream_hash_ignores_a_value_not_shaped_like_sha256() {
        // Wrong length: not a SHA-256 at all (maybe a different algorithm).
        assert!(verify_upstream_hash(Some("deadbeef"), "anything").is_ok());
        // Right length, but not hex.
        assert!(verify_upstream_hash(Some(&"z".repeat(64)), "anything").is_ok());
    }
}
