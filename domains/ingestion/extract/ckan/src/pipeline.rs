//! Orchestrates the per-version state machine and the recovery/consistency
//! checks that must run before it (design doc: pipeline overview diagram, §12).
//!
//! # Concurrency model
//!
//! Each discovered GTFS version is processed as an independent Tokio task.
//! An `Arc<Semaphore>` bounds the number of versions concurrently in-flight
//! (configurable via `GTFS_S_MAX_CONCURRENT_VERSIONS`, default `min(4, CPUs)`).
//!
//! Within each task:
//! * Download is fully async (`reqwest` streaming), never blocking the runtime.
//! * Archive extraction and CSV→Parquet conversion are CPU/disk-heavy
//!   synchronous work, so each is wrapped in `tokio::task::spawn_blocking` to
//!   offload them to Tokio's dedicated blocking-thread pool. This frees the
//!   async executor thread to drive other tasks (e.g. a concurrent download)
//!   while the blocking work runs.
//!
//! State mutation that must remain serialized — manifest, `latest`, the
//! `installed` map — is handled only after all tasks complete, in the main
//! task. No shared mutable state is accessed from concurrent workers.
//!
//! # Correctness invariants preserved
//!
//! * Staging directories are version-isolated; workers never share paths.
//! * Atomic rename (staging → final) is per-task and targets distinct paths —
//!   no two workers can rename to the same final dir.
//! * `latest` is advanced to the newest *version ID*, not the first worker to
//!   finish — completion order is irrelevant.
//! * Failed workers clean their own staging artifacts and never create a final
//!   snapshot directory.
//! * The updater lock prevents two independent `ckan` *invocations* from
//!   running concurrently; it does not serialize the independent workers inside
//!   a single invocation.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::archive::{self, ArchiveError};
use crate::ckan_client::{CkanClient, CkanClientError};
use crate::domain::{UpstreamResource, VersionId};
use crate::download::{self, DownloadError};
use crate::lock::{LockError, UpdaterLock};
use crate::manifest::{self, Manifest, SidecarStatus, SnapshotMeta};
use crate::parquet_convert::{self, ParquetError};
use crate::paths::RawLayout;
use crate::symlink::{self, SymlinkError};

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("could not acquire updater lock: {0}")]
    Lock(#[from] LockError),
    #[error("CKAN API error: {0}")]
    Ckan(#[from] CkanClientError),
    #[error("download failed: {0}")]
    Download(#[from] DownloadError),
    #[error("archive validation failed: {0}")]
    Archive(#[from] ArchiveError),
    #[error("CSV -> Parquet conversion failed: {0}")]
    Parquet(#[from] ParquetError),
    #[error("symlink update failed: {0}")]
    Symlink(#[from] SymlinkError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloaded archive fails upstream checksum verification: {0}")]
    HashMismatch(String),
    #[error(
        "invariant violated: `latest` symlink and manifest disagree on the current version ({0}); \
         this indicates a bug, not a recoverable runtime state — refusing to guess which side is right"
    )]
    LatestMismatch(String),
    #[error("concurrent task panicked: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Counts and totals from one run, for the end-of-run summary (invaluable in
/// CI/cron output and monitoring — see [`RunSummary::log`]).
#[derive(Debug, Clone, Copy)]
pub struct RunSummary {
    /// GTFS zip resources CKAN listed, before the cutoff-version filter.
    pub discovered: usize,
    /// Of those at or after the cutoff, how many were already installed.
    pub already_present: usize,
    /// How many versions this run attempted to download.
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    /// Sum of `archive_size_bytes` over successfully published versions only —
    /// bandwidth spent on an attempt that failed after the download step
    /// (e.g. archive validation) isn't included, to keep this a simple sum
    /// over sidecar data rather than needing a separate byte-counter threaded
    /// through every failure path.
    pub bytes_downloaded: u64,
    pub elapsed: Duration,
}

impl RunSummary {
    /// Emits both a structured tracing event (for log aggregators) and a
    /// human-readable block on stdout (for a person watching cron/CI output).
    pub fn log(&self) {
        tracing::info!(
            discovered = self.discovered,
            already_present = self.already_present,
            attempted = self.attempted,
            succeeded = self.succeeded,
            failed = self.failed,
            bytes_downloaded = self.bytes_downloaded,
            elapsed_secs = self.elapsed.as_secs_f64(),
            "updater run complete"
        );

        println!("Updater completed");
        println!("  Discovered:       {}", self.discovered);
        println!("  Already present:  {}", self.already_present);
        println!("  Attempted:        {}", self.attempted);
        println!("  Succeeded:        {}", self.succeeded);
        println!("  Failed:           {}", self.failed);
        println!(
            "  Bytes downloaded: {}",
            format_bytes(self.bytes_downloaded)
        );
        println!("  Elapsed:          {}", format_duration(self.elapsed));
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = UNITS[0];
    for candidate in &UNITS[1..] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = candidate;
    }
    if unit == UNITS[0] {
        format!("{bytes} {unit}")
    } else {
        format!("{value:.1} {unit}")
    }
}

fn format_duration(elapsed: Duration) -> String {
    let total_secs = elapsed.as_secs();
    let (hours, rem) = (total_secs / 3600, total_secs % 3600);
    let (minutes, seconds) = (rem / 60, rem % 60);
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// Runs one full check-and-update pass: recovery/consistency checks, then
/// discover → download → verify → extract → validate → publish for every
/// upstream version not yet installed, then advance `latest`.
///
/// Up to `max_concurrent` versions are processed simultaneously. Each version
/// is an independent Tokio task; CPU/disk-heavy stages use `spawn_blocking`.
/// Manifest, `latest`, and the installed-versions map are mutated only after
/// all tasks complete — never from concurrent workers.
///
/// Safe to run repeatedly on a schedule (design doc §10) and safe to have been
/// killed mid-run last time (§12) — every run starts by re-establishing a
/// clean, self-consistent state before touching the network.
pub async fn run(
    layout: &RawLayout,
    ckan_client: &CkanClient,
    download_http: &reqwest::Client,
    cutoff_version: Option<&VersionId>,
    max_concurrent: usize,
) -> Result<RunSummary, PipelineError> {
    let started_at = Instant::now();
    std::fs::create_dir_all(layout.root())?;

    let lock = UpdaterLock::acquire(layout.lock_path())?;
    tracing::info!(lock_path = %lock.path().display(), "acquired updater lock");

    clean_staging(layout)?;

    let mut installed = manifest::scan_sidecars(layout);
    tracing::info!(
        installed = installed.len(),
        "reconciled installed snapshots from sidecars"
    );

    let startup_manifest = manifest::rebuild_manifest(&installed, &BTreeMap::new(), Utc::now());
    manifest::write_manifest(layout, &startup_manifest)?;
    verify_latest_consistency(layout, &startup_manifest, &installed)?;

    let resources = ckan_client.list_gtfs_zip_resources().await?;
    let discovered = resources.len();
    tracing::info!(discovered, "discovered upstream GTFS-S resources");

    let eligible: Vec<UpstreamResource> = resources
        .into_iter()
        .filter(|r| cutoff_version.is_none_or(|cutoff| &r.version >= cutoff))
        .collect();
    let already_present = eligible
        .iter()
        .filter(|r| installed.contains_key(&r.version))
        .count();

    let mut pending: Vec<UpstreamResource> = eligible
        .into_iter()
        .filter(|r| !installed.contains_key(&r.version))
        .collect();
    pending.sort_by(|a, b| a.version.cmp(&b.version));
    tracing::info!(
        pending = pending.len(),
        already_present,
        max_concurrent,
        cutoff_version = ?cutoff_version,
        "versions pending download"
    );

    let attempted = pending.len();
    let mut failed_this_run: BTreeMap<VersionId, ()> = BTreeMap::new();

    // -- Concurrent processing phase ------------------------------------------
    //
    // Each version gets its own Tokio task. The semaphore caps the number of
    // tasks simultaneously in-flight. A task acquires the permit before spawning
    // and holds it for the full version pipeline (download + extract + convert),
    // so the cap applies to total resource consumption at once.
    //
    // JoinSet::join_next processes results as they arrive (real-time per-version
    // log output, lower peak memory than join_all).
    let semaphore = Arc::new(Semaphore::new(max_concurrent.max(1)));
    let mut set: JoinSet<(VersionId, Result<SnapshotMeta, PipelineError>)> = JoinSet::new();

    // Snapshot the installed set size before the concurrent phase so we can
    // compute which versions were newly downloaded this run.
    let pre_run_installed_count = installed.len();

    for resource in pending {
        // acquire_owned gives an OwnedSemaphorePermit that can be moved into
        // the spawned task. Dropping it releases the slot for the next version.
        let permit = Arc::clone(&semaphore)
            .acquire_owned()
            .await
            // Semaphore::acquire_owned only errors if closed; we never close it.
            .expect("semaphore is never closed");

        let http = download_http.clone();
        let layout = layout.clone();
        let version = resource.version.clone();

        tracing::info!(
            version = %resource.version,
            filename = %resource.original_filename,
            url = %resource.download_url,
            "queuing version for processing"
        );

        set.spawn(async move {
            let result = process_version(&http, &layout, &resource).await;
            // Drop permit after the full pipeline so the slot reflects actual
            // resource consumption (staging on disk, network, CPU).
            drop(permit);
            (version, result)
        });
    }

    // Collect results as tasks complete.
    while let Some(task_result) = set.join_next().await {
        let (version, outcome) = task_result?; // propagates task panics
        match outcome {
            Ok(meta) => {
                tracing::info!(%version, "version verified and published");
                installed.insert(version, meta);
            }
            Err(e) => {
                tracing::error!(%version, error = %e, "version failed; will retry next run");
                failed_this_run.insert(version, ());
            }
        }
    }
    // -------------------------------------------------------------------------

    advance_latest_if_needed(layout, &installed)?;

    let final_manifest = manifest::rebuild_manifest(&installed, &failed_this_run, Utc::now());
    manifest::write_manifest(layout, &final_manifest)?;

    let succeeded = attempted - failed_this_run.len();
    // Sum archive_size_bytes over versions that were newly installed this run:
    // everything in `installed` beyond what was already there before the
    // concurrent phase. BTreeMap preserves insertion-order within a run when
    // combined with the skip offset.
    let bytes_downloaded: u64 = installed
        .values()
        .skip(pre_run_installed_count)
        .map(|m| m.archive_size_bytes)
        .sum();

    let summary = RunSummary {
        discovered,
        already_present,
        attempted,
        succeeded,
        failed: failed_this_run.len(),
        bytes_downloaded,
        elapsed: started_at.elapsed(),
    };
    summary.log();

    Ok(summary)
}

/// Staging is always disposable (design doc §12): wipe it unconditionally at
/// the start of every run before touching the network, regardless of whether
/// the previous run crashed or exited cleanly.
fn clean_staging(layout: &RawLayout) -> Result<(), PipelineError> {
    let staging = layout.staging_dir();
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;
    Ok(())
}

/// Runs one version through download → verify size → extract → validate →
/// convert to Parquet → atomic rename → sidecar. Any failure here is reported
/// to the caller, which records it as `failed` for this run and moves on — it
/// never becomes `raw/<version>/` and never risks becoming `latest`.
///
/// CPU/disk-heavy stages (archive extraction, Parquet conversion) are wrapped
/// in [`tokio::task::spawn_blocking`] so they execute on Tokio's blocking
/// thread pool rather than occupying an async executor thread. This keeps the
/// runtime responsive to concurrent tasks (e.g. another version's download)
/// while a CPU-heavy conversion is running.
async fn process_version(
    http: &reqwest::Client,
    layout: &RawLayout,
    resource: &UpstreamResource,
) -> Result<SnapshotMeta, PipelineError> {
    let dir_name = resource.snapshot_dir_name();
    let part_path = layout.staging_part_path(&dir_name);
    let zip_path = layout.staging_zip_path(&dir_name);
    let extract_staging = layout.staging_extract_dir(&dir_name);
    let parquet_staging = layout.staging_parquet_dir(&dir_name);

    if extract_staging.exists() {
        std::fs::remove_dir_all(&extract_staging)?;
    }
    if parquet_staging.exists() {
        std::fs::remove_dir_all(&parquet_staging)?;
    }

    let downloaded_at = Utc::now();
    let outcome =
        download::download_to_staging(http, &resource.download_url, &part_path, &zip_path).await?;
    tracing::info!(
        version = %resource.version,
        bytes = outcome.bytes,
        content_length_header = ?outcome.content_length_header,
        sha256 = %outcome.sha256,
        "download verified"
    );

    if let Err(reason) =
        crate::domain::verify_upstream_hash(resource.upstream_hash.as_deref(), &outcome.sha256)
    {
        let _ = std::fs::remove_file(&zip_path);
        return Err(PipelineError::HashMismatch(reason));
    }

    std::fs::create_dir_all(&extract_staging)?;

    // Archive extraction: synchronous, CPU/disk-heavy — offload to the blocking
    // thread pool so the async executor remains free for concurrent downloads.
    {
        let zip = zip_path.clone();
        let extract = extract_staging.clone();
        if let Err(e) =
            tokio::task::spawn_blocking(move || archive::validate_and_extract(&zip, &extract))
                .await?
        {
            let _ = std::fs::remove_file(&zip_path);
            let _ = std::fs::remove_dir_all(&extract_staging);
            return Err(e.into());
        }
    }
    tracing::info!(version = %resource.version, "archive-level validation passed (Tier 1)");

    // Parquet is the canonical, permanently-persisted storage format (design
    // doc §8) — the CSVs extracted above and the zip itself are both scratch
    // once conversion succeeds. Highly compressible GTFS tables (stop_times,
    // calendar_dates) shrink dramatically vs. raw CSV, which is the point.
    std::fs::create_dir_all(&parquet_staging)?;

    // Parquet conversion: synchronous, CPU-heavy — same spawn_blocking
    // treatment as archive extraction above.
    {
        let csv_dir = extract_staging.clone();
        let pq_dir = parquet_staging.clone();
        if let Err(e) = tokio::task::spawn_blocking(move || {
            parquet_convert::convert_directory(&csv_dir, &pq_dir)
        })
        .await?
        {
            let _ = std::fs::remove_file(&zip_path);
            let _ = std::fs::remove_dir_all(&extract_staging);
            let _ = std::fs::remove_dir_all(&parquet_staging);
            return Err(e.into());
        }
    }
    std::fs::remove_dir_all(&extract_staging)?;
    tracing::info!(version = %resource.version, "converted to parquet");

    let final_dir = layout.final_dir(&dir_name);
    if final_dir.exists() {
        // Only reachable for a directory with no valid sidecar (an "already
        // installed?" version would have been filtered out before we got
        // here) — e.g. a pre-existing baseline snapshot that predates this
        // pipeline, or manual filesystem tampering (design doc §12, row 3).
        // We've now got a freshly downloaded, Tier-1-validated, and
        // Parquet-converted copy ready to go, so there's no data-loss window:
        // replace it.
        tracing::warn!(
            dir = %final_dir.display(),
            "overwriting pre-existing directory with no sidecar using freshly validated snapshot"
        );
        std::fs::remove_dir_all(&final_dir)?;
    }
    std::fs::rename(&parquet_staging, &final_dir)?;
    // Neither the zip nor the intermediate CSVs are part of what this design
    // retains (§8 retention is about the persisted Parquet at extract_path);
    // staging is disposable once its contents are published.
    let _ = std::fs::remove_file(&zip_path);

    let meta = SnapshotMeta {
        version: resource.version.clone(),
        source_url: resource.download_url.clone(),
        downloaded_at,
        archive_size_bytes: outcome.bytes,
        archive_sha256: outcome.sha256,
        publisher_last_modified: resource
            .publisher_last_modified
            .clone()
            .or(outcome.last_modified),
        etag: outcome.etag,
        extract_path: final_dir.to_string_lossy().to_string(),
        status: SidecarStatus::Verified,
    };
    manifest::write_sidecar(layout, &dir_name, &meta)?;

    Ok(meta)
}

/// Advances `latest` to the newest verified version, if it isn't already
/// there. Never moves it backwards, and never moves it at all if nothing
/// verified exists yet (design doc §7).
fn advance_latest_if_needed(
    layout: &RawLayout,
    installed: &BTreeMap<VersionId, SnapshotMeta>,
) -> Result<(), PipelineError> {
    let Some((newest_version, newest_meta)) = installed
        .iter()
        .filter(|(_, meta)| meta.status == SidecarStatus::Verified)
        .max_by_key(|(version, _)| (*version).clone())
    else {
        return Ok(());
    };

    let target_dir_name = dir_name_from_extract_path(&newest_meta.extract_path);
    let current = symlink::read_latest(layout)?;

    if current.as_deref() == Some(target_dir_name.as_str()) {
        return Ok(());
    }

    tracing::info!(version = %newest_version, target = %target_dir_name, "advancing latest symlink");
    symlink::advance_latest(layout, &target_dir_name)?;
    Ok(())
}

/// Asserts the manifest's `latest` field and the actual symlink target agree
/// (design doc §4, §12): a mismatch is a bug to fail loudly on, not a state to
/// silently reconcile by guessing which side is right.
fn verify_latest_consistency(
    layout: &RawLayout,
    manifest: &Manifest,
    installed: &BTreeMap<VersionId, SnapshotMeta>,
) -> Result<(), PipelineError> {
    let symlink_target = symlink::read_latest(layout)?;

    match (&manifest.latest, &symlink_target) {
        (None, None) => Ok(()),
        (None, Some(target)) => Err(PipelineError::LatestMismatch(format!(
            "manifest has no verified versions but `latest` points at {target:?}"
        ))),
        (Some(version), None) => Err(PipelineError::LatestMismatch(format!(
            "manifest latest is {version} but no `latest` symlink exists"
        ))),
        (Some(version), Some(target)) => {
            let meta = installed.get(version).ok_or_else(|| {
                PipelineError::LatestMismatch(format!(
                    "manifest latest {version} has no installed sidecar"
                ))
            })?;
            let expected_dir_name = dir_name_from_extract_path(&meta.extract_path);
            if &expected_dir_name != target {
                return Err(PipelineError::LatestMismatch(format!(
                    "manifest latest is {version} (dir {expected_dir_name:?}) but `latest` points at {target:?}"
                )));
            }
            Ok(())
        }
    }
}

fn dir_name_from_extract_path(extract_path: &str) -> String {
    Path::new(extract_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| extract_path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_picks_the_largest_sensible_unit() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(
            format_bytes(10 * 1024 * 1024 * 1024 + 300 * 1024 * 1024),
            "10.3 GiB"
        );
    }

    #[test]
    fn format_duration_scales_with_magnitude() {
        assert_eq!(format_duration(Duration::from_secs(42)), "42s");
        assert_eq!(format_duration(Duration::from_secs(62)), "1m 2s");
        assert_eq!(
            format_duration(Duration::from_secs(3600 + 60 + 5)),
            "1h 1m 5s"
        );
    }
}
