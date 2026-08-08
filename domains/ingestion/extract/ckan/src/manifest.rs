//! Per-snapshot metadata sidecars and the rollup manifest (design doc §2, §3, §4).
//!
//! The filesystem — specifically, the sidecars — is authoritative. The manifest
//! is always rebuilt from the sidecars at the start of every run (design doc
//! §12: "not just when it's detected missing, so drift can't accumulate
//! silently"); nothing here ever trusts a manifest read from disk as ground truth.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::domain::VersionId;
use crate::paths::RawLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidecarStatus {
    /// Archive-level structural checks passed (Tier 1 in the design doc §6).
    /// Does *not* imply GTFS content has been validated.
    Verified,
    /// Recorded here aspirationally for schema completeness — in practice a
    /// failed version never gets a final directory or sidecar (design doc §4:
    /// "the directory under its final name is never created for a failed
    /// version"), so this variant is never actually serialized to disk. It only
    /// shows up transiently in a single run's in-memory manifest rollup (see
    /// [`rebuild_manifest`]).
    Failed,
}

/// The durable, per-snapshot record: `raw/<version>/.snapshot-meta.json`.
///
/// Written exactly once, immediately after the atomic rename that publishes a
/// snapshot, and never touched again afterward (design doc §3, §5 step 6).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotMeta {
    pub version: VersionId,
    pub source_url: String,
    pub downloaded_at: DateTime<Utc>,
    pub archive_size_bytes: u64,
    pub archive_sha256: String,
    pub publisher_last_modified: Option<String>,
    pub etag: Option<String>,
    /// Directory containing this snapshot's Parquet files (one per GTFS
    /// member, e.g. `stops.parquet`) — the canonical, permanently-persisted
    /// storage format. The raw CSVs extracted from the archive are scratch:
    /// deleted right after conversion, never present at this path.
    pub extract_path: String,
    pub status: SidecarStatus,
}

pub fn write_sidecar(
    layout: &RawLayout,
    snapshot_dir_name: &str,
    meta: &SnapshotMeta,
) -> std::io::Result<()> {
    let path = layout.sidecar_path_for_dir(snapshot_dir_name);
    let json =
        serde_json::to_string_pretty(meta).expect("SnapshotMeta serialization is infallible");
    std::fs::write(path, json)
}

fn read_sidecar_at(path: &Path) -> Option<SnapshotMeta> {
    let contents = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&contents) {
        Ok(meta) => Some(meta),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "sidecar file is corrupt, ignoring");
            None
        }
    }
}

/// Scans `raw/*/.snapshot-meta.json`, returning every version with a valid
/// sidecar. Per design doc §2, a snapshot directory that exists but lacks a
/// valid sidecar is **not** considered installed — it's logged and left for the
/// pipeline/recovery step to treat as if it doesn't exist yet.
pub fn scan_sidecars(layout: &RawLayout) -> BTreeMap<VersionId, SnapshotMeta> {
    let mut found = BTreeMap::new();

    let raw_dir = layout.root();
    if !raw_dir.exists() {
        return found;
    }

    for entry in WalkDir::new(raw_dir).min_depth(1).max_depth(1) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !RawLayout::looks_like_snapshot_dir(&name) {
            continue;
        }

        let sidecar_path = entry.path().join(".snapshot-meta.json");
        match read_sidecar_at(&sidecar_path) {
            Some(meta) => {
                found.insert(meta.version.clone(), meta);
            }
            None => {
                tracing::warn!(
                    dir = %entry.path().display(),
                    "snapshot directory has no valid sidecar; treating as not installed \
                     (this indicates something outside the pipeline touched raw/)"
                );
            }
        }
    }

    found
}

/// Status as it appears in the manifest rollup — a superset of [`SidecarStatus`]
/// because the manifest additionally distinguishes "verified and current" from
/// "verified but no longer `latest`" (`superseded`). This distinction is
/// computed at rebuild time by comparing each version to `latest`; it is never
/// persisted on the sidecar itself (design doc §4: "There is deliberately no
/// persisted latest/active status on the version itself").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestStatus {
    Verified,
    Superseded,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestVersionEntry {
    pub status: ManifestStatus,
    #[serde(rename = "extract_path")]
    pub extract_path: Option<String>,
}

/// The rollup index at `raw/.manifest.json`. Purely a derived cache — see the
/// module doc comment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub generated_at: DateTime<Utc>,
    pub latest: Option<VersionId>,
    pub versions: BTreeMap<VersionId, ManifestVersionEntry>,
}

/// Rebuilds the manifest from durable sidecar data plus this run's in-memory
/// record of any versions that failed during the current invocation.
///
/// `failed_this_run` exists only to make the current run's manifest.json
/// informative (so a human inspecting it right after a run can see what just
/// failed) — it is **not** persisted across runs in any other way, since failed
/// versions never get a sidecar. A manifest rebuilt on a later run, without that
/// in-memory context, will simply omit versions that failed on a previous run
/// and were never retried successfully; that's consistent with the design
/// ("the manifest is a derived cache... never the only place a fact is
/// recorded") — nothing is lost, because a failed version with no directory
/// will just be re-attempted on the next run regardless of what the manifest says.
pub fn rebuild_manifest(
    installed: &BTreeMap<VersionId, SnapshotMeta>,
    failed_this_run: &BTreeMap<VersionId, ()>,
    now: DateTime<Utc>,
) -> Manifest {
    let latest = installed
        .iter()
        .filter(|(_, meta)| meta.status == SidecarStatus::Verified)
        .map(|(version, _)| version.clone())
        .max();

    let mut versions = BTreeMap::new();

    for (version, meta) in installed {
        let status = if Some(version) == latest.as_ref() {
            ManifestStatus::Verified
        } else {
            ManifestStatus::Superseded
        };
        versions.insert(
            version.clone(),
            ManifestVersionEntry {
                status,
                extract_path: Some(meta.extract_path.clone()),
            },
        );
    }

    for version in failed_this_run.keys() {
        versions.insert(
            version.clone(),
            ManifestVersionEntry {
                status: ManifestStatus::Failed,
                extract_path: None,
            },
        );
    }

    Manifest {
        generated_at: now,
        latest,
        versions,
    }
}

pub fn write_manifest(layout: &RawLayout, manifest: &Manifest) -> std::io::Result<()> {
    let json =
        serde_json::to_string_pretty(manifest).expect("Manifest serialization is infallible");
    std::fs::write(layout.manifest_path(), json)
}
