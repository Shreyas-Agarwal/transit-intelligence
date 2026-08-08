//! Design doc §11: a lock left behind by a dead process must self-heal on the
//! next run, and a lock held by a live process must block a second run.

use ckan::lock::UpdaterLock;

#[test]
fn acquire_and_release_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let lock_path = tmp.path().join(".updater.lock");

    let lock = UpdaterLock::acquire(lock_path.clone()).unwrap();
    assert!(lock_path.exists());
    drop(lock);
    assert!(!lock_path.exists(), "lock file must be removed on drop");
}

#[test]
fn stale_lock_from_a_dead_pid_self_heals() {
    let tmp = tempfile::tempdir().unwrap();
    let lock_path = tmp.path().join(".updater.lock");

    // A pid essentially guaranteed not to be running (Linux caps pid_max well
    // below this), on this host, written in the same shape `UpdaterLock` uses.
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| {
        std::fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown-host".to_string())
    });
    let stale = serde_json::json!({
        "pid": 999_999_999,
        "hostname": hostname,
        "started_at": "2020-01-01T00:00:00Z",
    });
    std::fs::write(&lock_path, stale.to_string()).unwrap();

    // Should detect the dead pid, clear the stale lock, and acquire cleanly.
    let lock = UpdaterLock::acquire(lock_path.clone()).unwrap();
    assert!(lock_path.exists());
    drop(lock);
}

#[test]
fn lock_held_by_the_current_process_is_treated_as_contended() {
    let tmp = tempfile::tempdir().unwrap();
    let lock_path = tmp.path().join(".updater.lock");

    // This process's own pid is, by definition, running — simulates a second
    // overlapping run racing against this one.
    let _first = UpdaterLock::acquire(lock_path.clone()).unwrap();
    let second = UpdaterLock::acquire(lock_path.clone());
    assert!(
        second.is_err(),
        "a lock held by a live pid must not be acquired again"
    );
}
