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
//!
//! Phase 5 adds one more thing here: Download (stage 2) and Extract/Convert
//! (stages 4 and 6) each acquire a resource-specific permit
//! (`crate::concurrency::ResourcePermits`) before doing their work — a
//! network-work limit independent from a CPU/disk-work limit, both
//! independent from how many versions are active overall. See that module
//! for why they're split and why Extract and Convert share one pool.
//!
//! # Stage-aware crash recovery (Phase 6)
//!
//! Before this phase, an interrupted run's staging leftovers were always
//! discarded wholesale at startup (`pipeline::clean_staging`) and every
//! retried version redid every stage from Download onward, regardless of how
//! far it had actually gotten. This phase makes recovery deterministic and
//! proportional to what's actually missing, by inspecting the filesystem —
//! never by remembering what a previous process was doing (a fresh worker,
//! with no history of its own, must reach the same conclusion a crashed
//! one's replacement would).
//!
//! [`find_resume_point`] answers one question before any stage runs: *how
//! much of this version's work, if any, is already durably present and
//! trustworthy?* The recovery matrix, by where a crash could have happened:
//!
//! | Crash point | Durable evidence found | Recovery |
//! |---|---|---|
//! | During Download | only `<name>.zip.part`, never renamed to `.zip` | discarded unconditionally (never resumable — no partial-byte-range support); Download restarts |
//! | Right after Download, during/before Verify | a complete `<name>.zip` | re-verified by rehashing the file (no re-download); Verify re-runs, cheaply, from the file already on disk |
//! | During Extract | `extract_staging/` exists but fails the required-member check | discarded; Extract restarts from the (already-verified) `.zip` |
//! | Right after Extract, during/before Validate | `extract_staging/` exists and passes the required-member check | trusted as-is; Download, Extract, and Validate are all skipped — resume straight at Convert |
//! | During Convert | `parquet_staging/` exists but is missing a `.parquet` file for some extracted member | discarded; Convert restarts from the (already-valid) `extract_staging/` |
//! | After Convert (staging complete, not yet published) | `parquet_staging/` fully matches what `extract_staging/` should have produced — including the case where `extract_staging/` was already cleaned up by the crashed run's own successful conversion | trusted as-is; Download through Convert are all skipped — resume straight at Publish |
//! | After the atomic rename, before the sidecar is written | `final_dir` exists but has no sidecar | **not specially fast-pathed** — handled by Publish's pre-existing "no sidecar means not really installed, safe to overwrite" rule (unchanged since before Phase 4); redone from whatever stage `find_resume_point` identifies, then republished over the old copy. Safe because a directory without a sidecar was never considered "installed" by `crate::manifest::scan_sidecars` in the first place — DD-001's own installation invariant. See the Deviations note in the implementation log for why this wasn't also fast-pathed. |
//! | After the sidecar is written, before the control-plane record is marked PUBLISHED | sidecar valid, filesystem shows the version installed | **already handled — no Phase 6 code needed.** `crate::reconcile::reconcile`'s `reconcile_as_published` (Phase 2) forces the control-plane record to PUBLISHED from *any* prior state once the filesystem shows a version installed; already proven for every non-PUBLISHED state by Phase 1/2's own tests. |
//! | Before `latest` is advanced | version installed, `latest` not yet updated (or pointing at an older version) | **already handled — no Phase 6 code needed.** `pipeline::advance_latest_if_needed` recomputes the target from every installed sidecar on each run; already idempotent and re-run-safe since before Phase 4. |
//! | After `latest` is advanced | fully complete | **already handled.** Recognized as installed and never touched again by `reconcile`. |
//!
//! Only the first six rows needed new logic in this phase (`find_resume_point`
//! and the conditional stage execution in `run_stages` below); the last four
//! were already correct, by construction, from earlier phases — Phase 6's
//! job for those was to confirm and document that, not to add anything.

use std::path::Path;

use chrono::Utc;
use tracing::Instrument as _;

use crate::archive;
use crate::concurrency::ResourcePermits;
use crate::domain::UpstreamResource;
use crate::download::{self, DownloadOutcome};
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
    permits: &ResourcePermits,
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
        ti_common::observability::mark_span_error(&tracing::Span::current(), e.to_string());
        return ProcessOutcome {
            work,
            meta: Err(e.to_string()),
        };
    }
    persist(layout, &work);

    // -- Stages 2-7: Download, Verify, Extract+Validate, Convert, Publish ---
    let result = run_stages(http, layout, resource, permits).await;

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
            ti_common::observability::mark_span_error(&tracing::Span::current(), message.as_str());
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

/// How much of a version's pipeline is already durably present and
/// trustworthy, per the recovery matrix in the module doc comment. Always
/// carries the archive's size/hash — needed for the sidecar regardless of
/// which stages actually still need to run.
enum ResumePoint {
    /// Nothing usable found (or what was found didn't check out); run every
    /// stage starting at Download.
    FromScratch,
    /// A valid `.zip` exists; skip Download, run Extract onward.
    Downloaded(DownloadOutcome),
    /// A valid, complete extraction exists; skip Download and Extract, run
    /// Convert onward.
    Extracted(DownloadOutcome),
    /// A complete conversion exists; skip straight to Publish.
    Converted(DownloadOutcome),
}

/// Inspects whatever staging artifacts an interrupted prior run may have
/// left behind for this exact version, and decides how much can be safely
/// skipped. Pure filesystem inspection: this function is handed only paths,
/// never anything from a previous process's memory, so a freshly started
/// worker with zero history reaches exactly the same conclusion a crashed
/// one's replacement would (implementation plan Phase 6: "do not rely on
/// process memory to determine recovery").
fn find_resume_point(
    zip_path: &Path,
    extract_staging: &Path,
    parquet_staging: &Path,
) -> ResumePoint {
    if !zip_path.is_file() {
        return ResumePoint::FromScratch;
    }
    let Ok(outcome) = download::recompute_outcome_from_existing_file(zip_path) else {
        // Exists but couldn't even be read back (e.g. a permissions issue,
        // or it's actually a directory) — treat it as if it weren't there.
        return ResumePoint::FromScratch;
    };

    // Checked furthest-along-first: a complete conversion must win even when
    // `extract_staging` no longer exists (it's normally removed immediately
    // after a successful conversion, before Publish) — checking extraction
    // validity first would wrongly fall back to `Downloaded` in exactly that
    // case, discarding a fully-converted `parquet_staging` for nothing.
    if is_conversion_complete(extract_staging, parquet_staging) {
        return ResumePoint::Converted(outcome);
    }

    let extraction_is_valid =
        extract_staging.is_dir() && archive::verify_extracted_members(extract_staging).is_ok();
    if !extraction_is_valid {
        return ResumePoint::Downloaded(outcome);
    }

    ResumePoint::Extracted(outcome)
}

impl ResumePoint {
    /// A short label for the "version" span (implementation plan Phase 7),
    /// recorded once `find_resume_point` has decided — so a trace can answer
    /// "how much of this version's work was actually redone" without
    /// needing a separate metric alongside the span.
    fn label(&self) -> &'static str {
        match self {
            ResumePoint::FromScratch => "from_scratch",
            ResumePoint::Downloaded(_) => "downloaded",
            ResumePoint::Extracted(_) => "extracted",
            ResumePoint::Converted(_) => "converted",
        }
    }
}

/// Whether `parquet_staging` is a fully completed conversion of
/// `extract_staging`.
///
/// If `extract_staging` still exists, completeness means exact
/// correspondence: every `*.txt` member has a matching, non-empty
/// `*.parquet` file. If `extract_staging` no longer exists, the only way
/// that can happen in this pipeline is that a prior run's conversion already
/// succeeded and its own post-conversion cleanup removed it (nothing else in
/// `run_stages` ever removes `extract_staging`) — so a non-empty
/// `parquet_staging` found in that situation is trusted as complete, with no
/// `.txt` manifest left to double-check it against.
fn is_conversion_complete(extract_staging: &Path, parquet_staging: &Path) -> bool {
    if !parquet_staging.is_dir() {
        return false;
    }

    if !extract_staging.is_dir() {
        return std::fs::read_dir(parquet_staging)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
    }

    let Ok(entries) = std::fs::read_dir(extract_staging) else {
        return false;
    };
    let txt_stems: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
        })
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .collect();

    if txt_stems.is_empty() {
        return false;
    }
    txt_stems.iter().all(|stem| {
        let parquet_file = parquet_staging.join(format!("{stem}.parquet"));
        std::fs::metadata(&parquet_file)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    })
}

/// Awaits `fut`, marking `span` as errored (see
/// `ti_common::observability::mark_span_error`) if it resolves to `Err` —
/// the "run this stage's future, and if it fails, record that failure on
/// its own span" pattern every stage below needs, written once rather than
/// once per stage. Stage-specific cleanup (removing a partial download,
/// discarding a bad extraction, ...) still happens at each call site, since
/// that's the one part that genuinely differs stage to stage.
async fn instrumented_stage<F, T, E>(span: &tracing::Span, fut: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    match fut.await {
        Ok(value) => Ok(value),
        Err(e) => {
            ti_common::observability::mark_span_error(span, e.to_string());
            Err(e)
        }
    }
}

/// Stages 2 through 7: download the archive to disposable staging (or reuse
/// one already there), verify it, extract and validate it (or reuse an
/// already-valid extraction), convert it to Parquet (or reuse an
/// already-complete conversion), and atomically publish it. See the module
/// doc comment for the full recovery matrix this implements.
async fn run_stages(
    http: &reqwest::Client,
    layout: &RawLayout,
    resource: &UpstreamResource,
    permits: &ResourcePermits,
) -> Result<SnapshotMeta, PipelineError> {
    let dir_name = resource.snapshot_dir_name();
    let part_path = layout.staging_part_path(&dir_name);
    let zip_path = layout.staging_zip_path(&dir_name);
    let extract_staging = layout.staging_extract_dir(&dir_name);
    let parquet_staging = layout.staging_parquet_dir(&dir_name);

    // A `.part` file is never resumable (no partial-byte-range support in
    // this design) — always garbage, regardless of what else is found.
    let _ = std::fs::remove_file(&part_path);

    let resume = find_resume_point(&zip_path, &extract_staging, &parquet_staging);
    let (need_download, need_extract, need_convert) = match &resume {
        ResumePoint::FromScratch => (true, true, true),
        ResumePoint::Downloaded(_) => (false, true, true),
        ResumePoint::Extracted(_) => (false, false, true),
        ResumePoint::Converted(_) => (false, false, false),
    };
    tracing::Span::current().record("resume_stage", resume.label());
    tracing::info!(
        version = %resource.version,
        need_download, need_extract, need_convert,
        "resuming from filesystem-verified staging state"
    );

    // Redoing Extract invalidates any conversion derived from the old
    // extraction; redoing Convert alone means whatever's in parquet_staging
    // (if anything — `find_resume_point` already decided it's incomplete)
    // must go too. Discard exactly what's about to be redone, nothing more.
    if need_extract && extract_staging.exists() {
        std::fs::remove_dir_all(&extract_staging)?;
    }
    if (need_extract || need_convert) && parquet_staging.exists() {
        std::fs::remove_dir_all(&parquet_staging)?;
    }

    // -- Stage 2: Download (bounded by the network permit pool) --------------
    let downloaded_at = Utc::now();
    let outcome = match resume {
        ResumePoint::FromScratch => {
            let download_span = tracing::info_span!("download");
            let outcome = instrumented_stage(
                &download_span,
                permits
                    .with_download_permit(|| {
                        download::download_to_staging(
                            http,
                            &resource.download_url,
                            &part_path,
                            &zip_path,
                        )
                    })
                    .instrument(download_span.clone()),
            )
            .await?;
            tracing::info!(
                version = %resource.version,
                bytes = outcome.bytes,
                content_length_header = ?outcome.content_length_header,
                sha256 = %outcome.sha256,
                "download verified"
            );
            outcome
        }
        ResumePoint::Downloaded(outcome)
        | ResumePoint::Extracted(outcome)
        | ResumePoint::Converted(outcome) => {
            tracing::info!(
                version = %resource.version,
                bytes = outcome.bytes,
                sha256 = %outcome.sha256,
                "resuming from an already-downloaded archive; re-verified from disk, not re-fetched"
            );
            outcome
        }
    };

    // -- Stage 3: Verify ------------------------------------------------------
    // (byte count against Content-Length already checked inside
    // `download_to_staging` for a fresh download; a resumed archive is
    // re-hashed above from its real, current bytes either way.)
    {
        let verify_span = tracing::info_span!("verify");
        let _entered = verify_span.enter();
        if let Err(reason) =
            crate::domain::verify_upstream_hash(resource.upstream_hash.as_deref(), &outcome.sha256)
        {
            ti_common::observability::mark_span_error(&verify_span, reason.as_str());
            let _ = std::fs::remove_file(&zip_path);
            return Err(PipelineError::HashMismatch(reason));
        }
    }

    // -- Stages 4-5: Extract + Validate (bounded by the processing permit pool) --
    if need_extract {
        std::fs::create_dir_all(&extract_staging)?;
        let zip = zip_path.clone();
        let extract = extract_staging.clone();
        let extract_span = tracing::info_span!("extract");
        let result = instrumented_stage(
            &extract_span,
            permits
                .with_processing_permit(move || {
                    tokio::task::spawn_blocking(move || {
                        archive::validate_and_extract(&zip, &extract)
                    })
                })
                .instrument(extract_span.clone()),
        )
        .await?;
        if let Err(e) = result {
            ti_common::observability::mark_span_error(&extract_span, e.to_string());
            let _ = std::fs::remove_file(&zip_path);
            let _ = std::fs::remove_dir_all(&extract_staging);
            return Err(e.into());
        }
        tracing::info!(version = %resource.version, "archive-level validation passed (Tier 1)");
    } else {
        tracing::info!(version = %resource.version, "extraction already valid on disk; skipping Extract/Validate");
    }

    // -- Stage 6: Convert (same processing permit pool as Extract) -----------
    if need_convert {
        std::fs::create_dir_all(&parquet_staging)?;
        let csv_dir = extract_staging.clone();
        let pq_dir = parquet_staging.clone();
        let convert_span = tracing::info_span!("convert");
        let result = instrumented_stage(
            &convert_span,
            permits
                .with_processing_permit(move || {
                    tokio::task::spawn_blocking(move || {
                        parquet_convert::convert_directory(&csv_dir, &pq_dir)
                    })
                })
                .instrument(convert_span.clone()),
        )
        .await?;
        if let Err(e) = result {
            ti_common::observability::mark_span_error(&convert_span, e.to_string());
            let _ = std::fs::remove_file(&zip_path);
            let _ = std::fs::remove_dir_all(&extract_staging);
            let _ = std::fs::remove_dir_all(&parquet_staging);
            return Err(e.into());
        }
        if extract_staging.exists() {
            std::fs::remove_dir_all(&extract_staging)?;
        }
        tracing::info!(version = %resource.version, "converted to parquet");
    } else {
        tracing::info!(version = %resource.version, "conversion already complete on disk; skipping Convert");
    }

    // -- Stage 7: Publish -------------------------------------------------------
    let publish_span = tracing::info_span!("publish");
    let _entered = publish_span.enter();
    let final_dir = layout.final_dir(&dir_name);
    if final_dir.exists() {
        tracing::warn!(
            dir = %final_dir.display(),
            "overwriting pre-existing directory with no sidecar using freshly validated snapshot"
        );
        if let Err(e) = std::fs::remove_dir_all(&final_dir) {
            ti_common::observability::mark_span_error(&publish_span, e.to_string());
            return Err(e.into());
        }
    }
    if let Err(e) = std::fs::rename(&parquet_staging, &final_dir) {
        ti_common::observability::mark_span_error(&publish_span, e.to_string());
        return Err(e.into());
    }
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
    if let Err(e) = manifest::write_sidecar(layout, &dir_name, &meta) {
        ti_common::observability::mark_span_error(&publish_span, e.to_string());
        return Err(e.into());
    }

    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    const REQUIRED_GTFS: &[&str] = &[
        "stops.txt",
        "trips.txt",
        "routes.txt",
        "stop_times.txt",
        "calendar_dates.txt",
    ];

    fn write_valid_zip(path: &Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        for name in REQUIRED_GTFS {
            w.start_file(*name, opts).unwrap();
            w.write_all(b"col_a,col_b\n1,2\n").unwrap();
        }
        w.finish().unwrap();
    }

    fn write_valid_extraction(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        for name in REQUIRED_GTFS {
            std::fs::write(dir.join(name), b"col_a,col_b\n1,2\n").unwrap();
        }
    }

    fn write_complete_conversion(extract_dir: &Path, parquet_dir: &Path) {
        write_valid_extraction(extract_dir);
        std::fs::create_dir_all(parquet_dir).unwrap();
        parquet_convert::convert_directory(extract_dir, parquet_dir).unwrap();
    }

    #[test]
    fn from_scratch_when_nothing_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let resume = find_resume_point(
            &tmp.path().join("v.zip"),
            &tmp.path().join("v"),
            &tmp.path().join("v.parquet"),
        );
        assert!(matches!(resume, ResumePoint::FromScratch));
    }

    #[test]
    fn from_scratch_when_zip_path_is_actually_a_directory() {
        // is_file() is false for a directory — treated the same as absent.
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("v.zip");
        std::fs::create_dir_all(&zip_path).unwrap();
        let resume = find_resume_point(
            &zip_path,
            &tmp.path().join("v"),
            &tmp.path().join("v.parquet"),
        );
        assert!(matches!(resume, ResumePoint::FromScratch));
    }

    #[test]
    fn downloaded_when_only_a_valid_zip_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("v.zip");
        write_valid_zip(&zip_path);

        let resume = find_resume_point(
            &zip_path,
            &tmp.path().join("v"),
            &tmp.path().join("v.parquet"),
        );
        assert!(matches!(resume, ResumePoint::Downloaded(_)));
    }

    #[test]
    fn downloaded_when_extraction_exists_but_is_incomplete() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("v.zip");
        write_valid_zip(&zip_path);
        let extract_dir = tmp.path().join("v");
        std::fs::create_dir_all(&extract_dir).unwrap();
        std::fs::write(extract_dir.join("stops.txt"), b"only one required member").unwrap();

        let resume = find_resume_point(&zip_path, &extract_dir, &tmp.path().join("v.parquet"));
        assert!(
            matches!(resume, ResumePoint::Downloaded(_)),
            "an incomplete extraction must not be trusted; Extract must still run"
        );
    }

    #[test]
    fn extracted_when_extraction_is_valid_but_conversion_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("v.zip");
        write_valid_zip(&zip_path);
        let extract_dir = tmp.path().join("v");
        write_valid_extraction(&extract_dir);

        let resume = find_resume_point(&zip_path, &extract_dir, &tmp.path().join("v.parquet"));
        assert!(matches!(resume, ResumePoint::Extracted(_)));
    }

    #[test]
    fn extracted_when_conversion_exists_but_is_incomplete() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("v.zip");
        write_valid_zip(&zip_path);
        let extract_dir = tmp.path().join("v");
        write_valid_extraction(&extract_dir);
        let parquet_dir = tmp.path().join("v.parquet");
        std::fs::create_dir_all(&parquet_dir).unwrap();
        // Only convert one of the five required members.
        std::fs::write(parquet_dir.join("stops.parquet"), b"not empty").unwrap();

        let resume = find_resume_point(&zip_path, &extract_dir, &parquet_dir);
        assert!(
            matches!(resume, ResumePoint::Extracted(_)),
            "an incomplete conversion must not be trusted; Convert must still run"
        );
    }

    #[test]
    fn converted_when_conversion_is_fully_complete() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("v.zip");
        write_valid_zip(&zip_path);
        let extract_dir = tmp.path().join("v");
        let parquet_dir = tmp.path().join("v.parquet");
        write_complete_conversion(&extract_dir, &parquet_dir);

        let resume = find_resume_point(&zip_path, &extract_dir, &parquet_dir);
        assert!(matches!(resume, ResumePoint::Converted(_)));
    }

    #[test]
    fn converted_when_extraction_was_already_cleaned_up_after_a_successful_conversion() {
        // Mirrors run_stages's own sequence: extract_staging is removed
        // immediately after a successful conversion, before Publish.
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("v.zip");
        write_valid_zip(&zip_path);
        let extract_dir = tmp.path().join("v");
        let parquet_dir = tmp.path().join("v.parquet");
        write_complete_conversion(&extract_dir, &parquet_dir);
        std::fs::remove_dir_all(&extract_dir).unwrap();

        let resume = find_resume_point(&zip_path, &extract_dir, &parquet_dir);
        assert!(
            matches!(resume, ResumePoint::Converted(_)),
            "a complete parquet_staging must be trusted even once extract_staging is gone"
        );
    }

    #[test]
    fn is_conversion_complete_is_false_when_a_parquet_file_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let extract_dir = tmp.path().join("v");
        write_valid_extraction(&extract_dir);
        let parquet_dir = tmp.path().join("v.parquet");
        std::fs::create_dir_all(&parquet_dir).unwrap();
        for name in ["stops", "trips", "routes", "stop_times"] {
            std::fs::write(parquet_dir.join(format!("{name}.parquet")), b"x").unwrap();
        }
        // calendar_dates.parquet deliberately missing.

        assert!(!is_conversion_complete(&extract_dir, &parquet_dir));
    }

    #[test]
    fn is_conversion_complete_is_false_when_a_parquet_file_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let extract_dir = tmp.path().join("v");
        write_valid_extraction(&extract_dir);
        let parquet_dir = tmp.path().join("v.parquet");
        std::fs::create_dir_all(&parquet_dir).unwrap();
        for name in REQUIRED_GTFS {
            let stem = name.trim_end_matches(".txt");
            let contents: &[u8] = if stem == "stops" { b"" } else { b"x" };
            std::fs::write(parquet_dir.join(format!("{stem}.parquet")), contents).unwrap();
        }

        assert!(!is_conversion_complete(&extract_dir, &parquet_dir));
    }

    #[test]
    fn is_conversion_complete_is_true_when_every_txt_has_a_matching_nonempty_parquet() {
        let tmp = tempfile::tempdir().unwrap();
        let extract_dir = tmp.path().join("v");
        let parquet_dir = tmp.path().join("v.parquet");
        write_complete_conversion(&extract_dir, &parquet_dir);

        assert!(is_conversion_complete(&extract_dir, &parquet_dir));
    }
}
