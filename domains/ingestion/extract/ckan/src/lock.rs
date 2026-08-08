//! The updater lock protocol from the design doc §11: exclusive-create a lock
//! file so two overlapping runs can't race on staging paths or `latest`, with a
//! staleness check so a crash doesn't require manual intervention to clear it.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("another updater run is already in progress (lock held by pid {pid} on {hostname})")]
    HeldByOther { pid: u32, hostname: String },
    #[error("io error acquiring/releasing lock: {0}")]
    Io(#[from] std::io::Error),
    #[error("lock file content is corrupt: {0}")]
    Corrupt(String),
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LockContents {
    pid: u32,
    hostname: String,
    started_at: chrono::DateTime<chrono::Utc>,
}

/// An acquired updater lock. Released automatically on drop, which covers both
/// the success path and any error path that unwinds through `?` — the design's
/// "release in a finally/defer" requirement, expressed as RAII.
pub struct UpdaterLock {
    path: PathBuf,
}

impl UpdaterLock {
    /// Acquires the lock at `path`, retrying once if an existing lock turns out
    /// to be stale (left behind by a process that's no longer running on this host).
    pub fn acquire(path: PathBuf) -> Result<Self, LockError> {
        match Self::try_create(&path) {
            Ok(()) => Ok(Self { path }),
            Err(LockError::Io(e)) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Self::handle_contention(&path)?;
                // One retry, per the design's step 2: "remove it, and retry
                // acquisition once". If this second attempt also finds the lock
                // held, that's a genuine concurrent run (or a race against
                // another process re-acquiring in the same instant) and we
                // surface it rather than looping.
                Self::try_create(&path)?;
                Ok(Self { path })
            }
            Err(e) => Err(e),
        }
    }

    fn try_create(path: &Path) -> Result<(), LockError> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let contents = LockContents {
            pid: std::process::id(),
            hostname: hostname(),
            started_at: chrono::Utc::now(),
        };
        let json =
            serde_json::to_string(&contents).expect("LockContents serialization is infallible");
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    fn handle_contention(path: &Path) -> Result<(), LockError> {
        let contents = read_lock_contents(path)?;
        let this_host = hostname();

        if contents.hostname != this_host {
            // Can't check whether a PID is alive on a different host; treat the
            // lock as held rather than guessing.
            return Err(LockError::HeldByOther {
                pid: contents.pid,
                hostname: contents.hostname,
            });
        }

        if is_process_running(contents.pid) {
            return Err(LockError::HeldByOther {
                pid: contents.pid,
                hostname: contents.hostname,
            });
        }

        tracing::warn!(
            pid = contents.pid,
            hostname = %contents.hostname,
            "removing stale updater lock left behind by a dead process"
        );
        std::fs::remove_file(path)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UpdaterLock {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            // Best-effort: if the lock file is already gone (e.g. removed out of
            // band) there's nothing more to do, and a Drop impl can't propagate
            // an error anyway.
            tracing::warn!(error = %e, path = %self.path.display(), "failed to release updater lock");
        }
    }
}

fn read_lock_contents(path: &Path) -> Result<LockContents, LockError> {
    let mut buf = String::new();
    std::fs::File::open(path)?.read_to_string(&mut buf)?;
    serde_json::from_str(&buf).map_err(|e| LockError::Corrupt(e.to_string()))
}

fn hostname() -> String {
    // `hostname::get()` would pull in another dependency for one syscall; the
    // env var is set by essentially every POSIX shell and is good enough for a
    // same-host liveness check.
    std::env::var("HOSTNAME").unwrap_or_else(|_| {
        std::fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown-host".to_string())
    })
}

/// Whether `pid` is a running process **on this host**. Linux-specific
/// (`/proc/<pid>`), which is acceptable here: this workspace targets Linux/WSL2
/// deployment (see `rust-toolchain.toml` / project environment), and getting this
/// wrong only affects stale-lock cleanup, not correctness of the pipeline itself.
fn is_process_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}
