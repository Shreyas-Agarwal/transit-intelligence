//! Durable per-version control-plane state (implementation plan Phase 1).
//!
//! The fundamental unit of work is `ProcessSnapshot(version)`, tracked
//! through:
//!
//! ```text
//! DISCOVERED
//!     |
//!     v
//!   QUEUED  <---------------+
//!     |                     |
//!     v                     |
//!   RUNNING ---------(retry)+
//!     |    \
//!     |     \--> FAILED
//!     v
//! PUBLISHED (idempotent: publishing an already-PUBLISHED record is a no-op)
//! ```
//!
//! `RUNNING -> QUEUED` is the stale-running recovery path: this project runs
//! one invocation at a time (`crate::lock::UpdaterLock`), so any record found
//! in `RUNNING` at the *start* of a new invocation necessarily predates that
//! invocation and cannot have a live worker behind it. Recovering it is not
//! the same as a failure — nothing conclusively went wrong, so it doesn't
//! consume a `last_error` slot or otherwise look like an attempt failure.
//!
//! This is the **control plane**: what work should happen. It is a separate
//! concern from the **data plane** — what snapshot data actually exists,
//! which remains authoritative in the filesystem sidecars
//! (`raw/<version>/.snapshot-meta.json`, see `crate::manifest`) per DD-001
//! §2. Nothing here changes that: a version can be marked `PUBLISHED` here
//! and this module still has no opinion on whether the snapshot directory or
//! sidecar actually exists — reconciling the two is Phase 2's job.
//!
//! Persisted as one JSON file per version under `.work/<version>.json`,
//! mirroring the existing per-snapshot sidecar convention in
//! `crate::manifest` rather than introducing a database.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::domain::VersionId;
use crate::paths::RawLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkState {
    Discovered,
    Queued,
    Running,
    Published,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid work state transition: {from:?} -> {to:?}")]
pub struct InvalidTransition {
    pub from: WorkState,
    pub to: WorkState,
}

/// The state graph. `Published -> Published` is the one self-loop, modelling
/// idempotent republication; everything else moves strictly along the arrows
/// documented in the module doc comment.
fn is_valid_transition(from: WorkState, to: WorkState) -> bool {
    use WorkState::*;
    matches!(
        (from, to),
        (Discovered, Queued)
            | (Queued, Running)
            | (Running, Published)
            | (Running, Failed)
            | (Running, Queued) // stale-running recovery
            | (Failed, Queued) // retry
            | (Published, Published) // idempotent no-op
    )
}

/// The durable, per-version control-plane record: `.work/<version>.json`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VersionWork {
    pub version: VersionId,
    pub source_url: String,
    pub state: WorkState,
    /// How many times this version has entered RUNNING. Incremented on every
    /// QUEUED -> RUNNING transition, including retries after a failure.
    pub attempt: u32,
    /// Set on entering RUNNING, cleared on leaving it (published, failed, or
    /// recovered as stale). Optional in this phase since there is exactly
    /// one worker (this process) — a future distributed backend would
    /// populate it with a real worker identity for lease ownership.
    pub worker_id: Option<String>,
    /// Start of the most recent RUNNING attempt.
    pub started_at: Option<DateTime<Utc>>,
    /// When the most recent attempt reached a conclusive outcome (published
    /// or failed). Left untouched by stale-running recovery, since an
    /// interrupted attempt reached no conclusion of its own.
    pub completed_at: Option<DateTime<Utc>>,
    /// Reason for the most recent failure. Cleared at the start of the next
    /// attempt; untouched by stale-running recovery.
    pub last_error: Option<String>,
}

impl VersionWork {
    /// A newly discovered version, not yet queued.
    pub fn discovered(version: VersionId, source_url: String) -> Self {
        Self {
            version,
            source_url,
            state: WorkState::Discovered,
            attempt: 0,
            worker_id: None,
            started_at: None,
            completed_at: None,
            last_error: None,
        }
    }

    fn transition(&mut self, to: WorkState) -> Result<(), InvalidTransition> {
        if !is_valid_transition(self.state, to) {
            return Err(InvalidTransition {
                from: self.state,
                to,
            });
        }
        self.state = to;
        Ok(())
    }

    pub fn queue(&mut self) -> Result<(), InvalidTransition> {
        self.transition(WorkState::Queued)
    }

    /// QUEUED -> RUNNING. Bumps `attempt`, records the worker and start time,
    /// and clears any error/completion left over from a previous attempt.
    pub fn start(
        &mut self,
        worker_id: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<(), InvalidTransition> {
        self.transition(WorkState::Running)?;
        self.attempt += 1;
        self.worker_id = worker_id;
        self.started_at = Some(now);
        self.completed_at = None;
        self.last_error = None;
        Ok(())
    }

    /// RUNNING -> PUBLISHED, or a no-op if already PUBLISHED.
    ///
    /// Idempotent by design: a crash between the atomic filesystem publish
    /// (`crate::pipeline::process_version`'s rename) and this control-plane
    /// update must be safely repeatable on the next run without erroring —
    /// "version processing is idempotent" is a non-negotiable invariant of
    /// the wider plan.
    pub fn publish(&mut self, now: DateTime<Utc>) -> Result<(), InvalidTransition> {
        if self.state == WorkState::Published {
            return Ok(());
        }
        self.transition(WorkState::Published)?;
        self.worker_id = None;
        self.completed_at = Some(now);
        Ok(())
    }

    /// RUNNING -> FAILED. Not itself terminal — `retry` moves a FAILED
    /// record back to QUEUED — but a FAILED record with no retry call stays
    /// FAILED indefinitely; nothing here forces a retry.
    pub fn fail(&mut self, error: String, now: DateTime<Utc>) -> Result<(), InvalidTransition> {
        self.transition(WorkState::Failed)?;
        self.worker_id = None;
        self.completed_at = Some(now);
        self.last_error = Some(error);
        Ok(())
    }

    /// FAILED -> QUEUED: makes a failed version eligible for another
    /// attempt. Whether/when to call this is a reconciliation policy
    /// decision (Phase 2); this only enforces that the transition itself is
    /// legal and leaves the failure's diagnostics (`last_error`) in place
    /// until the next attempt starts.
    pub fn retry(&mut self) -> Result<(), InvalidTransition> {
        self.transition(WorkState::Queued)
    }

    /// RUNNING -> QUEUED, for a record found RUNNING with no possible live
    /// owner (see module doc comment). Deliberately distinct from `fail`:
    /// it does not touch `last_error` and clears `started_at`, since the
    /// interrupted attempt reached no conclusion worth diagnosing.
    pub fn recover_stale_running(&mut self) -> Result<(), InvalidTransition> {
        self.transition(WorkState::Queued)?;
        self.worker_id = None;
        self.started_at = None;
        Ok(())
    }

    /// Forces the record to PUBLISHED regardless of its current state, to
    /// represent an outside observation of ground truth — an installed,
    /// sidecar-verified filesystem snapshot (`crate::manifest::scan_sidecars`)
    /// — rather than a normal lifecycle transition reached via `start`/
    /// `publish`.
    ///
    /// Deliberately bypasses `transition`'s validity check. The filesystem
    /// sidecar is authoritative for what is actually published (DD-001 §2):
    /// if it disagrees with this record's control-plane state — e.g. the
    /// record is still QUEUED, or FAILED, or has no record at all yet
    /// (bootstrapping control-plane state for a pre-existing snapshot) —
    /// the filesystem wins outright, unconditionally, rather than requiring
    /// this record to have arrived at PUBLISHED through RUNNING first.
    ///
    /// A no-op if already PUBLISHED (same idempotence guarantee as
    /// `publish`). Leaves `attempt` and `last_error` untouched: they
    /// describe this control plane's own attempt history, which an outside
    /// observation doesn't get to rewrite — only `state`, `worker_id`, and
    /// `completed_at` are corrected.
    pub fn reconcile_as_published(&mut self, now: DateTime<Utc>) {
        if self.state == WorkState::Published {
            return;
        }
        self.state = WorkState::Published;
        self.worker_id = None;
        self.completed_at = Some(now);
    }
}

pub fn write_work_state(layout: &RawLayout, work: &VersionWork) -> std::io::Result<()> {
    std::fs::create_dir_all(layout.work_dir())?;
    let json = serde_json::to_string_pretty(work).expect("VersionWork serialization is infallible");
    std::fs::write(layout.work_state_path(&work.version), json)
}

fn read_work_state_at(path: &Path) -> Option<VersionWork> {
    let contents = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&contents) {
        Ok(work) => Some(work),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "work state file is corrupt, ignoring");
            None
        }
    }
}

/// Scans `.work/*.json`, returning every version with a valid control-plane
/// record. A corrupt or unreadable record is logged and omitted — the caller
/// sees it as if the version had never been queued, which is safe: it will
/// be rediscovered from upstream on the next reconciliation pass (Phase 2).
pub fn scan_work_states(layout: &RawLayout) -> BTreeMap<VersionId, VersionWork> {
    let mut found = BTreeMap::new();

    let work_dir = layout.work_dir();
    if !work_dir.exists() {
        return found;
    }

    for entry in WalkDir::new(&work_dir).min_depth(1).max_depth(1) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(work) = read_work_state_at(entry.path()) {
            found.insert(work.version.clone(), work);
        }
    }

    found
}

/// Applies stale-RUNNING recovery in place to every record currently in
/// `RUNNING`, returning the versions that were recovered. Pure/in-memory —
/// callers decide whether and how to persist the result (Phase 2 wires this
/// into the startup reconciliation pass).
pub fn recover_stale_running(states: &mut BTreeMap<VersionId, VersionWork>) -> Vec<VersionId> {
    let mut recovered = Vec::new();
    for (version, work) in states.iter_mut() {
        if work.state == WorkState::Running {
            work.recover_stale_running()
                .expect("Running -> Queued is always a valid transition");
            recovered.push(version.clone());
        }
    }
    recovered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(state: WorkState) -> VersionWork {
        let mut work = VersionWork::discovered(
            VersionId::parse("20260805").unwrap(),
            "https://example.invalid/20260805.zip".to_string(),
        );
        let now: DateTime<Utc> = "2026-08-06T00:00:00Z".parse().unwrap();
        match state {
            WorkState::Discovered => {}
            WorkState::Queued => work.queue().unwrap(),
            WorkState::Running => {
                work.queue().unwrap();
                work.start(Some("worker-1".to_string()), now).unwrap();
            }
            WorkState::Published => {
                work.queue().unwrap();
                work.start(Some("worker-1".to_string()), now).unwrap();
                work.publish(now).unwrap();
            }
            WorkState::Failed => {
                work.queue().unwrap();
                work.start(Some("worker-1".to_string()), now).unwrap();
                work.fail("boom".to_string(), now).unwrap();
            }
        }
        work
    }

    fn now() -> DateTime<Utc> {
        "2026-08-06T00:00:00Z".parse().unwrap()
    }

    // -- valid state transitions ---------------------------------------

    #[test]
    fn happy_path_discovered_to_published() {
        let mut work = sample(WorkState::Discovered);
        assert_eq!(work.state, WorkState::Discovered);

        work.queue().unwrap();
        assert_eq!(work.state, WorkState::Queued);

        work.start(Some("worker-1".to_string()), now()).unwrap();
        assert_eq!(work.state, WorkState::Running);
        assert_eq!(work.attempt, 1);
        assert_eq!(work.worker_id.as_deref(), Some("worker-1"));
        assert_eq!(work.started_at, Some(now()));

        work.publish(now()).unwrap();
        assert_eq!(work.state, WorkState::Published);
        assert_eq!(work.completed_at, Some(now()));
        assert!(work.worker_id.is_none(), "ownership released on publish");
    }

    // -- invalid state transitions ---------------------------------------

    #[test]
    fn skipping_queued_is_rejected() {
        let mut work = sample(WorkState::Discovered);
        let err = work.transition(WorkState::Running).unwrap_err();
        assert_eq!(
            err,
            InvalidTransition {
                from: WorkState::Discovered,
                to: WorkState::Running,
            }
        );
    }

    #[test]
    fn every_illegal_transition_is_rejected() {
        use WorkState::*;
        let all = [Discovered, Queued, Running, Published, Failed];
        for &from in &all {
            for &to in &all {
                if is_valid_transition(from, to) {
                    continue;
                }
                let mut work = sample(from);
                assert!(
                    work.transition(to).is_err(),
                    "{from:?} -> {to:?} must be rejected"
                );
                assert_eq!(
                    work.state, from,
                    "a rejected transition must not mutate state"
                );
            }
        }
    }

    #[test]
    fn published_cannot_be_restarted() {
        let mut work = sample(WorkState::Published);
        assert!(work.transition(WorkState::Running).is_err());
        assert!(work.transition(WorkState::Queued).is_err());
        assert!(work.transition(WorkState::Failed).is_err());
    }

    // -- idempotent transition to PUBLISHED -------------------------------

    #[test]
    fn republishing_an_already_published_record_is_a_noop() {
        let mut work = sample(WorkState::Published);
        let before = work.clone();

        work.publish(now()).unwrap();

        assert_eq!(
            work, before,
            "a second publish call must not mutate the record"
        );
    }

    // -- retryable failure -------------------------------------------------

    #[test]
    fn a_failed_version_can_be_retried_and_reattempted() {
        let mut work = sample(WorkState::Failed);
        assert_eq!(work.attempt, 1);
        assert_eq!(work.last_error.as_deref(), Some("boom"));

        work.retry().unwrap();
        assert_eq!(work.state, WorkState::Queued);
        // Retrying alone doesn't erase the diagnostic; the next attempt does.
        assert_eq!(work.last_error.as_deref(), Some("boom"));

        work.start(Some("worker-2".to_string()), now()).unwrap();
        assert_eq!(work.state, WorkState::Running);
        assert_eq!(work.attempt, 2, "second attempt increments the counter");
        assert!(
            work.last_error.is_none(),
            "starting a new attempt clears the old error"
        );
    }

    // -- terminal failure ---------------------------------------------------

    #[test]
    fn a_failed_version_stays_failed_until_explicitly_retried() {
        let work = sample(WorkState::Failed);
        // No transition happens on its own; FAILED is a stable resting state.
        assert_eq!(work.state, WorkState::Failed);
        // And every transition except the explicit retry path is rejected.
        assert!(!is_valid_transition(WorkState::Failed, WorkState::Running));
        assert!(!is_valid_transition(
            WorkState::Failed,
            WorkState::Published
        ));
        assert!(!is_valid_transition(
            WorkState::Failed,
            WorkState::Discovered
        ));
        assert!(!is_valid_transition(WorkState::Failed, WorkState::Failed));
    }

    // -- recovery of stale RUNNING state -------------------------------------

    #[test]
    fn stale_running_recovers_to_queued_without_counting_as_a_failure() {
        let mut work = sample(WorkState::Running);
        assert_eq!(work.attempt, 1);

        work.recover_stale_running().unwrap();

        assert_eq!(work.state, WorkState::Queued);
        assert!(work.worker_id.is_none());
        assert!(work.started_at.is_none());
        assert_eq!(work.attempt, 1, "recovery does not consume an attempt");
        assert!(
            work.last_error.is_none(),
            "recovery is not a failure and must not populate last_error"
        );
        assert!(work.completed_at.is_none());
    }

    #[test]
    fn recover_stale_running_batch_only_touches_running_records() {
        let mut states = BTreeMap::new();
        for (v, state) in [
            ("20260701", WorkState::Discovered),
            ("20260708", WorkState::Queued),
            ("20260715", WorkState::Running),
            ("20260722", WorkState::Published),
            ("20260729", WorkState::Failed),
        ] {
            let mut work = sample(state);
            work.version = VersionId::parse(v).unwrap();
            states.insert(work.version.clone(), work);
        }

        let recovered = recover_stale_running(&mut states);

        assert_eq!(recovered, vec![VersionId::parse("20260715").unwrap()]);
        assert_eq!(
            states[&VersionId::parse("20260715").unwrap()].state,
            WorkState::Queued
        );
        // Everything else is untouched.
        assert_eq!(
            states[&VersionId::parse("20260701").unwrap()].state,
            WorkState::Discovered
        );
        assert_eq!(
            states[&VersionId::parse("20260708").unwrap()].state,
            WorkState::Queued
        );
        assert_eq!(
            states[&VersionId::parse("20260722").unwrap()].state,
            WorkState::Published
        );
        assert_eq!(
            states[&VersionId::parse("20260729").unwrap()].state,
            WorkState::Failed
        );
    }

    // -- reconcile_as_published (filesystem-observed override) -------------

    #[test]
    fn reconcile_as_published_forces_state_from_any_non_published_state() {
        use WorkState::*;
        for state in [Discovered, Queued, Running, Failed] {
            let mut work = sample(state);
            work.reconcile_as_published(now());
            assert_eq!(
                work.state, Published,
                "reconcile_as_published must override {state:?}"
            );
            assert!(work.worker_id.is_none());
            assert_eq!(work.completed_at, Some(now()));
        }
    }

    #[test]
    fn reconcile_as_published_preserves_attempt_and_last_error_history() {
        let mut work = sample(WorkState::Failed);
        assert_eq!(work.attempt, 1);
        assert_eq!(work.last_error.as_deref(), Some("boom"));

        work.reconcile_as_published(now());

        assert_eq!(work.state, WorkState::Published);
        assert_eq!(
            work.attempt, 1,
            "an outside observation doesn't rewrite attempt history"
        );
        assert_eq!(
            work.last_error.as_deref(),
            Some("boom"),
            "an outside observation doesn't erase prior diagnostics"
        );
    }

    #[test]
    fn reconcile_as_published_on_an_already_published_record_is_a_noop() {
        let mut work = sample(WorkState::Published);
        let before = work.clone();

        work.reconcile_as_published(now());

        assert_eq!(work, before);
    }
}
