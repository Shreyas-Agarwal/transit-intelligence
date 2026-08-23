//! End-to-end reconciliation test (implementation plan Phase 2): proves the
//! scheduler is disposable — restarting it and re-reading durable state from
//! disk reconstructs the same pending work, with no scheduler-private memory
//! required.

use ckan::domain::{UpstreamResource, VersionId};
use ckan::manifest::{self, SidecarStatus, SnapshotMeta};
use ckan::paths::RawLayout;
use ckan::reconcile::reconcile;
use ckan::work_state::{self, WorkState};

fn now() -> chrono::DateTime<chrono::Utc> {
    "2026-08-06T00:00:00Z".parse().unwrap()
}

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

fn v(s: &str) -> VersionId {
    VersionId::parse(s).unwrap()
}

/// Simulates: run 1 discovers and queues a version but crashes mid-run
/// (leaving it RUNNING on disk); run 2 (a fresh process, fresh in-memory
/// state, scanning everything from disk from scratch) must recover it and
/// re-offer it as eligible work — without ever having kept anything in
/// memory across the "restart".
#[test]
fn restart_reconstructs_pending_work_purely_from_durable_state() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());

    // -- run 1: discover, queue, start, then "crash" (no publish/fail) ------
    {
        let states = work_state::scan_work_states(&layout); // empty on first run
        let outcome = reconcile(
            &[resource("20260805")],
            None,
            &manifest::scan_sidecars(&layout),
            states,
            now(),
        );
        assert_eq!(outcome.eligible, vec![v("20260805")]);

        let mut states = outcome.states;
        let work = states.get_mut(&v("20260805")).unwrap();
        work.start(Some("run-1-worker".to_string()), now()).unwrap();
        for w in states.values() {
            work_state::write_work_state(&layout, w).unwrap();
        }
        // Process crashes here: no publish(), no fail(), nothing further
        // written. The persisted record is left RUNNING.
    }

    let persisted = work_state::scan_work_states(&layout);
    assert_eq!(persisted[&v("20260805")].state, WorkState::Running);

    // -- run 2: fresh process, nothing carried over except what's on disk ---
    let states = work_state::scan_work_states(&layout);
    let outcome = reconcile(
        &[resource("20260805")],
        None,
        &manifest::scan_sidecars(&layout),
        states,
        now(),
    );

    assert_eq!(
        outcome.recovered_from_stale_running,
        vec![v("20260805")],
        "run 2 must recognize the RUNNING record as stale on its own, from disk alone"
    );
    assert_eq!(
        outcome.eligible,
        vec![v("20260805")],
        "the recovered version must be re-offered as eligible work"
    );
    assert_eq!(outcome.states[&v("20260805")].state, WorkState::Queued);
    assert_eq!(
        outcome.states[&v("20260805")].attempt,
        1,
        "the interrupted attempt is not double-counted by recovery"
    );
}

/// A version that finishes publishing to the filesystem in run 1 (atomic
/// rename + sidecar written) must be recognized as complete in run 2 purely
/// from the filesystem + a freshly rescanned control plane, even though
/// run 2 never observed run 1's in-memory `VersionWork` at all.
#[test]
fn restart_recognizes_a_filesystem_published_snapshot_without_replaying_history() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());

    // Simulate a fully published snapshot: sidecar on disk, but no control
    // record was ever written for it (e.g. it predates this control plane,
    // or the process crashed after the sidecar write but before recording
    // PUBLISHED).
    let dir_name = "gtfs_fp2026_20260805";
    std::fs::create_dir_all(layout.final_dir(dir_name)).unwrap();
    let meta = SnapshotMeta {
        version: v("20260805"),
        source_url: "https://example.invalid/20260805.zip".to_string(),
        downloaded_at: now(),
        archive_size_bytes: 1024,
        archive_sha256: "deadbeef".to_string(),
        publisher_last_modified: None,
        etag: None,
        extract_path: layout.final_dir(dir_name).to_string_lossy().to_string(),
        status: SidecarStatus::Verified,
    };
    manifest::write_sidecar(&layout, dir_name, &meta).unwrap();

    let states = work_state::scan_work_states(&layout); // empty: no control record yet
    let outcome = reconcile(
        &[resource("20260805")],
        None,
        &manifest::scan_sidecars(&layout),
        states,
        now(),
    );

    assert!(
        outcome.eligible.is_empty(),
        "an installed snapshot must never be queued for reprocessing"
    );
    assert_eq!(outcome.states[&v("20260805")].state, WorkState::Published);
}

/// Two reconciliation passes over an unchanging durable/filesystem/upstream
/// world, persisted to disk between them, produce the same outcome — the
/// scheduler carries no hidden state that a restart could lose or duplicate.
#[test]
fn two_passes_separated_by_a_full_persist_and_rescan_agree() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    let resources = vec![resource("20260805"), resource("20260812")];

    let first = reconcile(
        &resources,
        None,
        &manifest::scan_sidecars(&layout),
        work_state::scan_work_states(&layout),
        now(),
    );
    for w in first.states.values() {
        work_state::write_work_state(&layout, w).unwrap();
    }

    let second = reconcile(
        &resources,
        None,
        &manifest::scan_sidecars(&layout),
        work_state::scan_work_states(&layout),
        now(),
    );

    assert_eq!(first.states, second.states);
    assert_eq!(first.eligible, second.eligible);
    assert_eq!(second.eligible, vec![v("20260805"), v("20260812")]);
}
