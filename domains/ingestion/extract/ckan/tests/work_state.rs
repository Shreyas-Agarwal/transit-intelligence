//! Persistence integration tests for the durable control-plane work state
//! (implementation plan Phase 1): round-tripping `.work/<version>.json`
//! through disk, and recovering a stale RUNNING record left behind by a
//! crashed invocation.

use ckan::domain::VersionId;
use ckan::paths::RawLayout;
use ckan::work_state::{self, VersionWork, WorkState};

fn now() -> chrono::DateTime<chrono::Utc> {
    "2026-08-06T00:00:00Z".parse().unwrap()
}

#[test]
fn write_then_scan_round_trips_every_field() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());

    let mut work = VersionWork::discovered(
        VersionId::parse("20260805").unwrap(),
        "https://example.invalid/20260805.zip".to_string(),
    );
    work.queue().unwrap();
    work.start(Some("worker-1".to_string()), now()).unwrap();
    work.fail("archive validation failed".to_string(), now())
        .unwrap();

    work_state::write_work_state(&layout, &work).unwrap();
    assert!(layout.work_state_path(&work.version).exists());

    let scanned = work_state::scan_work_states(&layout);
    let reread = &scanned[&work.version];
    assert_eq!(reread, &work);
}

#[test]
fn scan_ignores_corrupt_and_non_json_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.work_dir()).unwrap();

    // A valid record.
    let good = VersionWork::discovered(
        VersionId::parse("20260805").unwrap(),
        "https://example.invalid/20260805.zip".to_string(),
    );
    work_state::write_work_state(&layout, &good).unwrap();

    // Corrupt JSON and an unrelated file must not break the scan.
    std::fs::write(layout.work_dir().join("20260812.json"), b"{not json").unwrap();
    std::fs::write(layout.work_dir().join("README.md"), b"not a work record").unwrap();

    let scanned = work_state::scan_work_states(&layout);
    assert_eq!(scanned.len(), 1);
    assert!(scanned.contains_key(&VersionId::parse("20260805").unwrap()));
}

#[test]
fn scan_on_missing_work_dir_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    // .work/ is never created.
    assert!(work_state::scan_work_states(&layout).is_empty());
}

/// Simulates a crash mid-run: a RUNNING record is left on disk from a
/// previous invocation. The next invocation's startup recovery pass must
/// find it, requeue it, and persist the recovered state — without treating
/// any other persisted version as touched.
#[test]
fn stale_running_record_recovers_across_a_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());

    let mut crashed = VersionWork::discovered(
        VersionId::parse("20260805").unwrap(),
        "https://example.invalid/20260805.zip".to_string(),
    );
    crashed.queue().unwrap();
    crashed.start(Some("worker-1".to_string()), now()).unwrap();
    work_state::write_work_state(&layout, &crashed).unwrap();

    let mut untouched = VersionWork::discovered(
        VersionId::parse("20260729").unwrap(),
        "https://example.invalid/20260729.zip".to_string(),
    );
    untouched.queue().unwrap();
    untouched
        .start(Some("worker-1".to_string()), now())
        .unwrap();
    untouched.publish(now()).unwrap();
    work_state::write_work_state(&layout, &untouched).unwrap();

    // -- "restart": scan, recover, persist ---------------------------------
    let mut states = work_state::scan_work_states(&layout);
    let recovered = work_state::recover_stale_running(&mut states);
    assert_eq!(recovered, vec![VersionId::parse("20260805").unwrap()]);
    for work in states.values() {
        work_state::write_work_state(&layout, work).unwrap();
    }

    // -- verify durably persisted, not just in memory ----------------------
    let reread = work_state::scan_work_states(&layout);
    let recovered_record = &reread[&VersionId::parse("20260805").unwrap()];
    assert_eq!(recovered_record.state, WorkState::Queued);
    assert!(recovered_record.worker_id.is_none());
    assert!(recovered_record.started_at.is_none());
    assert_eq!(
        recovered_record.attempt, 1,
        "recovery must not consume an attempt"
    );

    let published_record = &reread[&VersionId::parse("20260729").unwrap()];
    assert_eq!(
        published_record.state,
        WorkState::Published,
        "an unrelated PUBLISHED record must be untouched by recovery"
    );
}
