//! Explicit per-version processing pipeline (implementation plan Phase 4):
//! `process_snapshot(version)` broken into its named stages.
//!
//! ```text
//! 1. Claim     QUEUED -> RUNNING, records worker ownership, persisted immediately
//! 2. Download  stream the archive to disposable staging
//! 3. Verify    byte count (during download) + SHA-256 vs. the publisher's hash
//! 4. Extract   \_ kept as one atomic step — see the note below
//! 5. Validate  /
//! 6. Convert   every extracted *.txt member -> a same-named *.parquet file
//! 7. Publish   atomic rename: staging -> final snapshot directory
//! 8. Complete  write the sidecar, then RUNNING -> PUBLISHED (or -> FAILED), persisted
//! ```
//!
//! Stages 4 (Extract) and 5 (Validate) are deliberately **not** split into
//! separate functions, even though the plan lists them as distinct stages.
//! `crate::archive::validate_and_extract` already interleaves three passes —
//! a pre-extraction CRC/required-member check, the extraction itself, and a
//! post-extraction re-check — specifically so a known-bad archive is never
//! partially extracted, and so filesystem-level truncation introduced by
//! extraction itself is still caught. Splitting that into two independent
//! functions would either duplicate the pre-extraction check or lose the
//! "don't extract a bad archive" guarantee; DD-001's Tier-1 validation
//! behavior is preserved unchanged here rather than restructured. This was
//! flagged as an open question in the Phase 0 reconnaissance and resolved
//! this way in Phase 4 — see the implementation log for the review question
//! inviting a different call if preferred.
//!
//! Stages 2–7 are unchanged from the pipeline's pre-Phase-4 behavior (moved
//! here from `pipeline::process_version` without altering their logic).
//! Stages 1 and 8 are new in Phase 4: they wire the durable control-plane
//! state introduced in Phase 1 (`crate::work_state`) into the actual
//! processing path for the first time. Before Phase 4, a `VersionWork`
//! record could be created, transitioned, and persisted in isolation
//! (Phases 1–2), but nothing in the real pipeline ever called `start()`,
//! `publish()`, or `fail()` — this module is where that stops being true.

use chrono::Utc;

use crate::archive;
use crate::domain::UpstreamResource;
use crate::download;
use crate::manifest::{self, SidecarStatus, SnapshotMeta};
use crate::parquet_convert;
use crate::paths::RawLayout;
use crate::pipeline::PipelineError;
use crate::work_state::{self, VersionWork};

/// The result of running one version through every stage. `work` is the
/// fully up-to-date control-plane record (already persisted to
/// `.work/<version>.json` by this function) and `meta` is the outcome: the
/// durable snapshot metadata on success, or a display-formatted error on
/// failure — matching how `VersionWork::last_error` stores failures (a
/// string, not a live error object, since the record must be serializable).
pub struct ProcessOutcome {
    pub work: VersionWork,
    pub meta: Result<SnapshotMeta, String>,
}

/// Runs one version through every stage, claiming it first and recording
/// PUBLISHED or FAILED at the end — the whole reason this function exists,
/// rather than callers driving `VersionWork` transitions themselves, is so
/// the Claim step and the Complete/Fail step are never accidentally skipped
/// or reordered relative to the actual processing work.
///
/// `work` must already be in the QUEUED state (i.e. what
/// `crate::reconcile::reconcile` hands out as eligible) — this is an
/// invariant of the caller, not re-validated here beyond the Claim
/// transition itself rejecting anything else.
pub async fn process_snapshot(
    http: &reqwest::Client,
    layout: &RawLayout,
    resource: &UpstreamResource,
    mut work: VersionWork,
    worker_id: Option<String>,
) -> ProcessOutcome {
    // -- Stage 1: Claim -----------------------------------------------------
    if let Err(e) = work.start(worker_id, Utc::now()) {
        tracing::error!(
            version = %resource.version,
            error = %e,
            "cannot claim a version that reconcile() did not hand out as QUEUED; this is a bug in the caller, not a runtime condition"
        );
        return ProcessOutcome {
            work,
            meta: Err(e.to_string()),
        };
    }
    persist(layout, &work);

    // -- Stages 2-7: Download, Verify, Extract+Validate, Convert, Publish ---
    let result = run_stages(http, layout, resource).await;

    // -- Stage 8: Complete ---------------------------------------------------
    let now = Utc::now();
    match result {
        Ok(meta) => {
            work.publish(now)
                .expect("RUNNING -> PUBLISHED is always valid after a successful run");
            persist(layout, &work);
            ProcessOutcome {
                work,
                meta: Ok(meta),
            }
        }
        Err(e) => {
            let message = e.to_string();
            work.fail(message.clone(), now)
                .expect("RUNNING -> FAILED is always valid after a failed run");
            persist(layout, &work);
            ProcessOutcome {
                work,
                meta: Err(message),
            }
        }
    }
}

/// Best-effort persistence: a write failure here is logged, not fatal. The
/// alternative — aborting the whole version because its control-plane
/// bookkeeping couldn't be written — would throw away real processing work
/// (a successfully published snapshot, or a legitimately diagnosed failure)
/// over a secondary durability concern. Filesystem sidecars remain the
/// authoritative record of what's actually published either way (DD-001
/// §2); a lost control-plane write is corrected on the next reconciliation
/// pass, not a data-loss event.
fn persist(layout: &RawLayout, work: &VersionWork) {
    if let Err(e) = work_state::write_work_state(layout, work) {
        tracing::warn!(
            version = %work.version,
            error = %e,
            "failed to persist control-plane work state; will be corrected on the next reconciliation pass"
        );
    }
}

/// Stages 2 through 7: download the archive to disposable staging, verify
/// it, extract and validate it, convert it to Parquet, and atomically
/// publish it. Identical logic to the pre-Phase-4 `pipeline::process_version`
/// — moved here, not rewritten, so DD-001's existing publication invariants
/// (no partial publication, staging cleaned up on every failure path) carry
/// over unchanged.
async fn run_stages(
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

    // -- Stage 2: Download ---------------------------------------------------
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

    // -- Stage 3: Verify ------------------------------------------------------
    // (byte count against Content-Length already checked inside `download_to_staging`)
    if let Err(reason) =
        crate::domain::verify_upstream_hash(resource.upstream_hash.as_deref(), &outcome.sha256)
    {
        let _ = std::fs::remove_file(&zip_path);
        return Err(PipelineError::HashMismatch(reason));
    }

    std::fs::create_dir_all(&extract_staging)?;

    // -- Stages 4-5: Extract + Validate (kept atomic; see module doc comment) --
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

    std::fs::create_dir_all(&parquet_staging)?;

    // -- Stage 6: Convert -----------------------------------------------------
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

    // -- Stage 7: Publish -------------------------------------------------------
    let final_dir = layout.final_dir(&dir_name);
    if final_dir.exists() {
        tracing::warn!(
            dir = %final_dir.display(),
            "overwriting pre-existing directory with no sidecar using freshly validated snapshot"
        );
        std::fs::remove_dir_all(&final_dir)?;
    }
    std::fs::rename(&parquet_staging, &final_dir)?;
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
