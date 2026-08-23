//! Orchestrates the overall update run: recovery/consistency checks,
//! discovery, reconciliation, bounded concurrent processing, and the
//! post-processing bookkeeping (manifest rebuild, `latest` advancement).
//!
//! # Architecture (implementation plan Phases 1-5)
//!
//! ```text
//! CKAN discovery
//!     |
//!     v
//! reconcile()            control-plane state (crate::reconcile, crate::work_state)
//!     |                  vs. filesystem-installed snapshots (crate::manifest)
//!     v
//! bounded queue          crate::queue — a fixed-size worker pool consuming
//!     |                  from a capacity-limited queue; producing more
//!     v                  eligible work never spawns more workers, it waits
//! process_snapshot()     crate::snapshot — claim, download, verify, extract,
//!                        validate, convert, publish, complete, per version
//!     |
//!     v
//! resource permits       crate::concurrency — Download draws from one limit,
//!                        Extract/Convert draw from a second, independent
//!                        limit, both independent of the worker-pool size above
//! ```
//!
//! Enqueuing eligible versions and draining their results run concurrently
//! (one background task feeds the queue while this function reads results as
//! they arrive) — never enqueue everything and only then start reading
//! results. With two independently-bounded channels (the work queue and its
//! result channel) doing that sequentially can deadlock: workers can get
//! stuck handing back results with nowhere to put them, which stops them
//! from freeing queue capacity, which stops the producer from finishing, which
//! is the only thing that would ever let result-draining start. Draining
//! concurrently is a correctness requirement, not a performance nicety.
//!
//! State mutation that must remain serialized — the manifest, `latest`, and
//! the `installed` map — is still applied only in this function, after
//! results are collected, exactly as before Phase 4. No shared mutable state
//! is accessed by two workers at once: each worker gets its own copy of the
//! version's control-plane record and returns the updated copy through the
//! result channel; nothing is mutated through a shared reference.
//!
//! # Correctness invariants preserved
//!
//! * Staging directories are version-isolated; workers never share paths.
//! * Atomic rename (staging → final) is per-worker and targets distinct
//!   paths — no two workers can rename to the same final dir.
//! * `latest` is advanced to the newest *version ID*, not the first worker to
//!   finish — completion order is irrelevant.
//! * Failed workers clean their own staging artifacts and never create a final
//!   snapshot directory.
//! * The updater lock prevents two independent `ckan` *invocations* from
//!   running concurrently; it does not serialize the independent workers inside
//!   a single invocation.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::archive::ArchiveError;
use crate::ckan_client::{CkanClient, CkanClientError};
use crate::concurrency::ResourcePermits;
use crate::domain::{UpstreamResource, VersionId};
use crate::download::DownloadError;
use crate::lock::{LockError, UpdaterLock};
use crate::manifest::{self, Manifest, SidecarStatus, SnapshotMeta};
use crate::parquet_convert::ParquetError;
use crate::paths::RawLayout;
use crate::queue::{self, QueueConfig};
use crate::reconcile;
use crate::snapshot::{self, ProcessOutcome};
use crate::symlink::{self, SymlinkError};
use crate::work_state::{self, VersionWork};

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

/// Every concurrency knob `run` needs, bundled so the function signature
/// doesn't grow a new standalone parameter each time a phase adds another
/// (Phase 3's queue capacity, Phase 5's resource permits, ...). Constructed
/// directly from the matching `CkanConfig` fields by the caller (`main.rs`).
#[derive(Debug, Clone, Copy)]
pub struct ConcurrencyConfig {
    /// How many versions may be active at once, in any stage (Phase 3/4).
    pub max_concurrent_versions: usize,
    /// How many eligible versions may sit queued, waiting for a worker,
    /// before the producer blocks (Phase 3).
    pub max_queued_versions: usize,
    /// How many versions may be downloading at once (Phase 5), independent
    /// of `max_concurrent_versions`.
    pub max_concurrent_downloads: usize,
    /// How many versions may be extracting or converting at once (Phase 5;
    /// one shared limit for both stages), independent of
    /// `max_concurrent_versions`.
    pub max_concurrent_processing: usize,
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
/// discover → reconcile → bounded-queue processing for every eligible
/// version, then advance `latest`.
///
/// Up to `max_concurrent` versions are processed simultaneously by a fixed
/// worker pool, fed through a queue holding at most `max_queued` versions
/// waiting for a worker (implementation plan Phases 3-4). Manifest, `latest`,
/// and the installed-versions map are mutated only after results are
/// collected here — never from a worker.
///
/// Safe to run repeatedly on a schedule (design doc §10) and safe to have been
/// killed mid-run last time (§12) — every run starts by re-establishing a
/// clean, self-consistent state before touching the network.
pub async fn run(
    layout: &RawLayout,
    ckan_client: &CkanClient,
    download_http: &reqwest::Client,
    cutoff_version: Option<&VersionId>,
    concurrency: ConcurrencyConfig,
) -> Result<RunSummary, PipelineError> {
    let ConcurrencyConfig {
        max_concurrent_versions: max_concurrent,
        max_queued_versions: max_queued,
        max_concurrent_downloads,
        max_concurrent_processing,
    } = concurrency;
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

    let work_states = work_state::scan_work_states(layout);

    let resources = ckan_client.list_gtfs_zip_resources().await?;
    let discovered = resources.len();
    tracing::info!(discovered, "discovered upstream GTFS-S resources");

    let already_present = resources
        .iter()
        .filter(|r| cutoff_version.is_none_or(|cutoff| &r.version >= cutoff))
        .filter(|r| installed.contains_key(&r.version))
        .count();

    let reconciled = reconcile::reconcile(
        &resources,
        cutoff_version,
        &installed,
        work_states,
        Utc::now(),
    );
    for work in reconciled.states.values() {
        if let Err(e) = work_state::write_work_state(layout, work) {
            tracing::warn!(version = %work.version, error = %e, "failed to persist reconciled work state");
        }
    }
    if !reconciled.recovered_from_stale_running.is_empty() {
        tracing::warn!(
            versions = ?reconciled.recovered_from_stale_running,
            "recovered stale RUNNING work left behind by a previous crashed invocation"
        );
    }
    if !reconciled.diverged_published_without_filesystem.is_empty() {
        tracing::error!(
            versions = ?reconciled.diverged_published_without_filesystem,
            "control plane believes these versions are published but the filesystem \
             disagrees; left untouched pending investigation, not auto-corrected"
        );
    }
    tracing::info!(
        pending = reconciled.eligible.len(),
        already_present,
        ignored_below_cutoff = reconciled.ignored_below_cutoff.len(),
        max_concurrent,
        max_queued,
        cutoff_version = ?cutoff_version,
        "versions pending download"
    );

    let attempted = reconciled.eligible.len();
    let mut states = reconciled.states;
    let mut failed_this_run: BTreeMap<VersionId, ()> = BTreeMap::new();

    // Snapshot the installed set size before the concurrent phase so we can
    // compute which versions were newly downloaded this run.
    let pre_run_installed_count = installed.len();

    // -- Concurrent processing phase ------------------------------------------
    //
    // Every eligible version flows through a bounded queue (crate::queue)
    // consumed by a fixed pool of `max_concurrent` worker tasks, each running
    // the full explicit per-version pipeline (crate::snapshot): claim,
    // download, verify, extract, validate, convert, publish, complete.
    //
    // A worker needs the discovered `UpstreamResource` (download URL, hash,
    // etc.) and the version's current control-plane record. Neither is
    // carried through the queue itself — only the `VersionId` is — so both
    // are looked up here from this run's own discovery result and
    // reconciliation output, never re-fetched from CKAN.
    let resources_by_version: Arc<HashMap<VersionId, UpstreamResource>> = Arc::new(
        resources
            .iter()
            .map(|r| (r.version.clone(), r.clone()))
            .collect(),
    );
    let states_snapshot: Arc<BTreeMap<VersionId, VersionWork>> = Arc::new(states.clone());
    let worker_id = format!("{}:{}", crate::lock::hostname(), std::process::id());
    // Independent of the worker-pool size above: Download draws from one
    // limit, Extract/Convert share a second — see crate::concurrency.
    let permits = ResourcePermits::new(max_concurrent_downloads, max_concurrent_processing);

    let (work_queue, mut results_rx, mut workers) = {
        let http = download_http.clone();
        let layout = layout.clone();
        let resources_by_version = Arc::clone(&resources_by_version);
        let states_snapshot = Arc::clone(&states_snapshot);
        let permits = permits.clone();
        queue::spawn(
            QueueConfig {
                max_queued: max_queued.max(1),
                max_active: max_concurrent.max(1),
            },
            move |version: VersionId| {
                let http = http.clone();
                let layout = layout.clone();
                let permits = permits.clone();
                let resource = resources_by_version
                    .get(&version)
                    .cloned()
                    .expect("a queued version must be present in this run's discovery result");
                let work = states_snapshot
                    .get(&version)
                    .cloned()
                    .expect("a queued version must have a control-plane record from reconcile()");
                let worker_id = Some(worker_id.clone());
                async move {
                    snapshot::process_snapshot(&http, &layout, &resource, &permits, work, worker_id)
                        .await
                }
            },
        )
    };

    // The producer (enqueuing eligible work) and this function (draining
    // results) run concurrently — see the module doc comment for why that's
    // a correctness requirement, not just a throughput optimization.
    let eligible = reconciled.eligible;
    let producer = tokio::spawn(async move {
        for version in eligible {
            if work_queue.enqueue(version).await.is_err() {
                break; // every worker has already exited; nothing left to feed
            }
        }
        work_queue.close();
    });

    while let Some((_version, outcome)) = results_rx.recv().await {
        let ProcessOutcome { work, meta } = outcome;
        let version = work.version.clone();
        states.insert(version.clone(), work);
        match meta {
            Ok(snapshot_meta) => {
                tracing::info!(%version, "version verified and published");
                installed.insert(version, snapshot_meta);
            }
            Err(e) => {
                tracing::error!(%version, error = %e, "version failed; will retry next run");
                failed_this_run.insert(version, ());
            }
        }
    }

    producer.await?; // propagates a panic in the producer task itself
    while let Some(res) = workers.join_next().await {
        res?; // propagates a worker task panic
    }
    // -------------------------------------------------------------------------

    advance_latest_if_needed(layout, &installed)?;

    let final_manifest = manifest::rebuild_manifest(&installed, &failed_this_run, Utc::now());
    manifest::write_manifest(layout, &final_manifest)?;

    let succeeded = attempted - failed_this_run.len();
    // Sum archive_size_bytes over versions newly installed this run: every
    // entry in `installed` beyond what was already there before the
    // concurrent phase. Correct because `installed` sorts by VersionId (its
    // key) and cutoff-bounded discovery means anything installed this run is
    // chronologically newer than everything installed in a previous run —
    // not because BTreeMap preserves insertion order (it doesn't; it's
    // always key-ordered).
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

/// Ensures `.staging/` exists and sweeps every `*.zip.part` file left behind
/// under it — an interrupted download's partial bytes, never resumable in
/// this design (no HTTP range support), so always safe to discard
/// unconditionally regardless of which version it belongs to or whether that
/// version is even still eligible this run.
///
/// Before Phase 6 this function wiped `.staging/` wholesale every run,
/// unconditionally discarding anything an interrupted prior run had left
/// behind. It no longer does: a completed `.zip`, a validated extraction, or
/// a completed conversion are all left in place for `crate::snapshot`'s
/// per-version resume logic (`find_resume_point`) to inspect and validate on
/// its own when it actually processes that version — this function runs
/// once at startup, before any version's resource metadata or control-plane
/// record is even available yet, so it isn't in a position to make that
/// per-version judgment itself.
fn clean_staging(layout: &RawLayout) -> Result<(), PipelineError> {
    let staging = layout.staging_dir();
    std::fs::create_dir_all(&staging)?;
    for entry in std::fs::read_dir(&staging)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("part") {
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
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
