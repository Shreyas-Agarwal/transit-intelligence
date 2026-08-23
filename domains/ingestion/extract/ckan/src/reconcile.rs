//! Reconciliation scheduler (implementation plan Phase 2): separates
//! *discovery* (what does upstream have) from *execution* (what should this
//! run actually process).
//!
//! ```text
//! CKAN
//!   |
//!   v
//! discover()      ckan_client::list_gtfs_zip_resources (unchanged)
//!   |
//!   v
//! reconcile()      <- this module
//!   |
//!   v
//! durable version/work state      (.work/<version>.json, crate::work_state)
//!   |
//!   v
//! eligible work    (versions now QUEUED, ready to be claimed and run)
//! ```
//!
//! [`reconcile`] is a pure function of its inputs — the upstream resource
//! list, the cutoff, the filesystem-authoritative installed set
//! (`crate::manifest::scan_sidecars`), and the previously persisted work
//! states (`crate::work_state::scan_work_states`) — and returns a new work
//! state map plus the list of versions eligible to run this pass. It does
//! not perform any I/O itself; callers scan durable state in and persist the
//! returned state back out.
//!
//! This makes the scheduler disposable: restarting it and calling
//! `reconcile` again with the same durable state (rescanned fresh from disk)
//! and the same upstream/filesystem observations reproduces the same
//! decision, by construction — there is no scheduler-private memory for a
//! restart to lose. See `tests/reconcile.rs` for the end-to-end restart
//! test.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::domain::{UpstreamResource, VersionId};
use crate::manifest::SnapshotMeta;
use crate::work_state::{self, VersionWork, WorkState};

/// The result of one reconciliation pass. `states` is the full updated
/// work-state map (unchanged entries included) — the caller is expected to
/// persist it (e.g. `work_state::write_work_state` per entry) before acting
/// on `eligible`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReconcileOutcome {
    pub states: BTreeMap<VersionId, VersionWork>,
    /// Versions now QUEUED and ready to be claimed for processing this pass,
    /// oldest version first (matches the existing pipeline's processing
    /// order, DD-001 §1: "processed oldest-first").
    pub eligible: Vec<VersionId>,
    /// Versions upstream reported but below the cutoff. Recorded for
    /// visibility only — no work record is created or touched for these.
    pub ignored_below_cutoff: Vec<VersionId>,
    /// RUNNING records recovered to QUEUED because this pass ran at the
    /// start of a new invocation (see `work_state::recover_stale_running`).
    pub recovered_from_stale_running: Vec<VersionId>,
    /// Versions the control plane believes are PUBLISHED but that the
    /// filesystem does not currently show as installed. Not acted on
    /// automatically: manual filesystem manipulation is outside normal
    /// operation (DD-001 "User Responsibilities"), so this is surfaced for
    /// an operator/caller to investigate rather than silently reprocessed
    /// or silently trusted.
    pub diverged_published_without_filesystem: Vec<VersionId>,
}

/// Runs one reconciliation pass. See the module doc comment for the overall
/// shape; rule-by-rule:
///
/// * A resource below `cutoff_version` is ignored outright — no work record
///   is created, and an existing one (if any) is left untouched.
/// * A version present in `installed` (filesystem-verified) is forced to
///   PUBLISHED regardless of what the control plane currently believes —
///   the filesystem is authoritative for what's actually published. This
///   also bootstraps a work record for a snapshot that predates the control
///   plane's existence.
/// * A version already QUEUED (including one just recovered from stale
///   RUNNING) stays QUEUED and is eligible again — it never got processed.
/// * A version FAILED and not installed is retried: FAILED -> QUEUED,
///   eligible.
/// * A version with no record yet, not installed: first discovery —
///   DISCOVERED -> QUEUED in the same pass, eligible.
/// * A version the control plane believes PUBLISHED but that isn't actually
///   installed is left as-is and reported in
///   `diverged_published_without_filesystem` rather than guessed at.
pub fn reconcile(
    resources: &[UpstreamResource],
    cutoff_version: Option<&VersionId>,
    installed: &BTreeMap<VersionId, SnapshotMeta>,
    mut states: BTreeMap<VersionId, VersionWork>,
    now: DateTime<Utc>,
) -> ReconcileOutcome {
    // Any RUNNING record at the start of a pass predates this invocation
    // (single-invocation lock, crate::lock::UpdaterLock) and cannot have a
    // live worker behind it.
    let recovered_from_stale_running = work_state::recover_stale_running(&mut states);

    let mut sorted: Vec<&UpstreamResource> = resources.iter().collect();
    sorted.sort_by(|a, b| a.version.cmp(&b.version));

    let mut eligible = Vec::new();
    let mut ignored_below_cutoff = Vec::new();
    let mut diverged_published_without_filesystem = Vec::new();

    for resource in sorted {
        if let Some(cutoff) = cutoff_version
            && &resource.version < cutoff
        {
            ignored_below_cutoff.push(resource.version.clone());
            continue;
        }

        if installed.contains_key(&resource.version) {
            let entry = states.entry(resource.version.clone()).or_insert_with(|| {
                VersionWork::discovered(resource.version.clone(), resource.download_url.clone())
            });
            entry.reconcile_as_published(now);
            continue;
        }

        match states.get_mut(&resource.version) {
            None => {
                let mut work = VersionWork::discovered(
                    resource.version.clone(),
                    resource.download_url.clone(),
                );
                work.queue().expect("DISCOVERED -> QUEUED is always valid");
                states.insert(resource.version.clone(), work);
                eligible.push(resource.version.clone());
            }
            Some(work) => match work.state {
                WorkState::Discovered => {
                    // Not expected to persist across a reconcile pass (this
                    // function always advances DISCOVERED -> QUEUED in the
                    // same pass it creates one) but handled rather than
                    // assumed impossible.
                    work.queue().expect("DISCOVERED -> QUEUED is always valid");
                    eligible.push(resource.version.clone());
                }
                WorkState::Queued => {
                    eligible.push(resource.version.clone());
                }
                WorkState::Running => {
                    // Already handled by recover_stale_running above; this
                    // arm is defensive rather than reachable.
                    work.recover_stale_running()
                        .expect("RUNNING -> QUEUED is always valid");
                    eligible.push(resource.version.clone());
                }
                WorkState::Failed => {
                    work.retry().expect("FAILED -> QUEUED is always valid");
                    eligible.push(resource.version.clone());
                }
                WorkState::Published => {
                    diverged_published_without_filesystem.push(resource.version.clone());
                }
            },
        }
    }

    ReconcileOutcome {
        states,
        eligible,
        ignored_below_cutoff,
        recovered_from_stale_running,
        diverged_published_without_filesystem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(version: &str) -> UpstreamResource {
        UpstreamResource {
            version: VersionId::parse(version).unwrap(),
            name_prefix: "gtfs_fp2026".to_string(),
            download_url: format!("https://example.invalid/{version}.zip"),
            original_filename: format!("GTFS_FP2026_{version}.zip"),
            publisher_last_modified: None,
            upstream_hash: None,
        }
    }

    fn snapshot_meta(version: &str) -> SnapshotMeta {
        SnapshotMeta {
            version: VersionId::parse(version).unwrap(),
            source_url: format!("https://example.invalid/{version}.zip"),
            downloaded_at: now(),
            archive_size_bytes: 1024,
            archive_sha256: "deadbeef".to_string(),
            publisher_last_modified: None,
            etag: None,
            extract_path: format!("/data/bronze/static/gtfs_fp2026_{version}"),
            status: crate::manifest::SidecarStatus::Verified,
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-08-06T00:00:00Z".parse().unwrap()
    }

    fn v(s: &str) -> VersionId {
        VersionId::parse(s).unwrap()
    }

    // -- first discovery -----------------------------------------------

    #[test]
    fn a_version_with_no_prior_record_is_discovered_and_queued() {
        let outcome = reconcile(
            &[resource("20260805")],
            None,
            &BTreeMap::new(),
            BTreeMap::new(),
            now(),
        );

        assert_eq!(outcome.eligible, vec![v("20260805")]);
        let work = &outcome.states[&v("20260805")];
        assert_eq!(work.state, WorkState::Queued);
        assert_eq!(work.attempt, 0, "queueing alone is not an attempt");
        assert_eq!(work.source_url, "https://example.invalid/20260805.zip");
    }

    // -- already-known versions -------------------------------------------

    #[test]
    fn a_version_already_queued_stays_queued_and_eligible_without_duplication() {
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20260805"), "old-url".to_string());
        existing.queue().unwrap();
        states.insert(v("20260805"), existing.clone());

        let outcome = reconcile(
            &[resource("20260805")],
            None,
            &BTreeMap::new(),
            states,
            now(),
        );

        assert_eq!(outcome.eligible, vec![v("20260805")]);
        assert_eq!(
            outcome.states.len(),
            1,
            "must not create a duplicate record"
        );
        assert_eq!(outcome.states[&v("20260805")].state, WorkState::Queued);
    }

    // -- already-published versions ----------------------------------------

    #[test]
    fn an_already_published_version_with_matching_filesystem_state_is_a_noop() {
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20260805"), "url".to_string());
        existing.queue().unwrap();
        existing.start(None, now()).unwrap();
        existing.publish(now()).unwrap();
        let before = existing.clone();
        states.insert(v("20260805"), existing);

        let mut installed = BTreeMap::new();
        installed.insert(v("20260805"), snapshot_meta("20260805"));

        let outcome = reconcile(&[resource("20260805")], None, &installed, states, now());

        assert!(
            outcome.eligible.is_empty(),
            "an already-published, still-installed version must not be reprocessed"
        );
        assert_eq!(
            outcome.states[&v("20260805")],
            before,
            "no-op means byte-for-byte unchanged, not just same state"
        );
        assert!(outcome.diverged_published_without_filesystem.is_empty());
    }

    // -- failed versions eligible for retry ---------------------------------

    #[test]
    fn a_failed_version_not_yet_installed_is_retried() {
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20260805"), "url".to_string());
        existing.queue().unwrap();
        existing.start(None, now()).unwrap();
        existing.fail("boom".to_string(), now()).unwrap();
        states.insert(v("20260805"), existing);

        let outcome = reconcile(
            &[resource("20260805")],
            None,
            &BTreeMap::new(),
            states,
            now(),
        );

        assert_eq!(outcome.eligible, vec![v("20260805")]);
        let work = &outcome.states[&v("20260805")];
        assert_eq!(work.state, WorkState::Queued);
        assert_eq!(
            work.last_error.as_deref(),
            Some("boom"),
            "retry alone doesn't erase the diagnostic; the next attempt does"
        );
    }

    // -- stale running versions ----------------------------------------------

    #[test]
    fn a_stale_running_version_is_recovered_to_queued_and_eligible() {
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20260805"), "url".to_string());
        existing.queue().unwrap();
        existing
            .start(Some("dead-worker".to_string()), now())
            .unwrap();
        states.insert(v("20260805"), existing);

        let outcome = reconcile(
            &[resource("20260805")],
            None,
            &BTreeMap::new(),
            states,
            now(),
        );

        assert_eq!(outcome.recovered_from_stale_running, vec![v("20260805")]);
        assert_eq!(outcome.eligible, vec![v("20260805")]);
        let work = &outcome.states[&v("20260805")];
        assert_eq!(work.state, WorkState::Queued);
        assert!(work.worker_id.is_none());
    }

    // -- cutoff behavior -----------------------------------------------------

    #[test]
    fn a_version_below_cutoff_is_ignored_and_gets_no_record() {
        let outcome = reconcile(
            &[resource("20250101"), resource("20260805")],
            Some(&v("20260101")),
            &BTreeMap::new(),
            BTreeMap::new(),
            now(),
        );

        assert_eq!(outcome.ignored_below_cutoff, vec![v("20250101")]);
        assert_eq!(outcome.eligible, vec![v("20260805")]);
        assert!(!outcome.states.contains_key(&v("20250101")));
    }

    #[test]
    fn cutoff_does_not_disturb_an_existing_record_for_an_old_version() {
        // A version below the (possibly newly raised) cutoff that already
        // has a control-plane record must not have that record touched.
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20250101"), "url".to_string());
        existing.queue().unwrap();
        states.insert(v("20250101"), existing.clone());

        let outcome = reconcile(
            &[resource("20250101")],
            Some(&v("20260101")),
            &BTreeMap::new(),
            states,
            now(),
        );

        assert_eq!(outcome.ignored_below_cutoff, vec![v("20250101")]);
        assert!(outcome.eligible.is_empty());
        assert_eq!(
            outcome.states[&v("20250101")],
            existing,
            "an ignored version's existing record must be left untouched"
        );
    }

    // -- filesystem state overriding stale control-plane assumptions --------

    #[test]
    fn an_installed_filesystem_snapshot_with_no_control_record_bootstraps_as_published() {
        let mut installed = BTreeMap::new();
        installed.insert(v("20260805"), snapshot_meta("20260805"));

        let outcome = reconcile(
            &[resource("20260805")],
            None,
            &installed,
            BTreeMap::new(),
            now(),
        );

        assert!(outcome.eligible.is_empty());
        assert_eq!(outcome.states[&v("20260805")].state, WorkState::Published);
    }

    #[test]
    fn an_installed_filesystem_snapshot_overrides_a_failed_control_record() {
        // e.g. a version failed, was manually fixed and republished outside
        // the normal pipeline, or completed just before a crash that lost
        // the control-plane update.
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20260805"), "url".to_string());
        existing.queue().unwrap();
        existing.start(None, now()).unwrap();
        existing.fail("boom".to_string(), now()).unwrap();
        states.insert(v("20260805"), existing);

        let mut installed = BTreeMap::new();
        installed.insert(v("20260805"), snapshot_meta("20260805"));

        let outcome = reconcile(&[resource("20260805")], None, &installed, states, now());

        assert!(
            outcome.eligible.is_empty(),
            "an installed version must never be re-downloaded"
        );
        let work = &outcome.states[&v("20260805")];
        assert_eq!(work.state, WorkState::Published);
        assert_eq!(
            work.last_error.as_deref(),
            Some("boom"),
            "the override doesn't erase prior diagnostic history"
        );
    }

    #[test]
    fn an_installed_filesystem_snapshot_overrides_a_queued_control_record() {
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20260805"), "url".to_string());
        existing.queue().unwrap();
        states.insert(v("20260805"), existing);

        let mut installed = BTreeMap::new();
        installed.insert(v("20260805"), snapshot_meta("20260805"));

        let outcome = reconcile(&[resource("20260805")], None, &installed, states, now());

        assert!(outcome.eligible.is_empty());
        assert_eq!(outcome.states[&v("20260805")].state, WorkState::Published);
    }

    // -- divergence: control plane published, filesystem disagrees ----------

    #[test]
    fn a_published_control_record_without_filesystem_backing_is_flagged_not_requeued() {
        let mut states = BTreeMap::new();
        let mut existing = VersionWork::discovered(v("20260805"), "url".to_string());
        existing.queue().unwrap();
        existing.start(None, now()).unwrap();
        existing.publish(now()).unwrap();
        states.insert(v("20260805"), existing.clone());

        // installed is empty: filesystem doesn't actually have it.
        let outcome = reconcile(
            &[resource("20260805")],
            None,
            &BTreeMap::new(),
            states,
            now(),
        );

        assert_eq!(
            outcome.diverged_published_without_filesystem,
            vec![v("20260805")]
        );
        assert!(
            outcome.eligible.is_empty(),
            "divergence is surfaced, not silently auto-resolved by reprocessing"
        );
        assert_eq!(
            outcome.states[&v("20260805")],
            existing,
            "the record itself is left untouched pending investigation"
        );
    }

    // -- disposability: reconciling twice with the same inputs is stable ----

    #[test]
    fn reconciling_twice_with_the_same_inputs_is_idempotent() {
        let first = reconcile(
            &[resource("20260805")],
            None,
            &BTreeMap::new(),
            BTreeMap::new(),
            now(),
        );

        let second = reconcile(
            &[resource("20260805")],
            None,
            &BTreeMap::new(),
            first.states.clone(),
            now(),
        );

        assert_eq!(first.states, second.states);
        assert_eq!(first.eligible, second.eligible);
    }
}
