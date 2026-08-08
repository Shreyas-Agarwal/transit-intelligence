//! Atomic `latest` symlink management — design doc §7.
//!
//! Equivalent to `ln -sfn <target> raw/latest`: create a new symlink at a
//! temporary path and `rename()` it over the old one. `rename()` on the same
//! filesystem is atomic, so readers always see either the old target or the new
//! one, never a missing/broken link — the same guarantee `ln -sfn` relies on.
//! We don't shell out to `ln`; std's `rename`-over-symlink gives the identical
//! atomicity guarantee directly, without a footgun-prone external command.
//!
//! Note the explicit `-fn` (not just `-f`) footgun the design doc calls out:
//! `ln -f` alone would place the new link *inside* an existing symlinked
//! directory instead of replacing it. Building the swap from `symlink` +
//! `rename` sidesteps this entirely — `rename` always replaces the destination
//! path itself, never resolves through it first.

use std::path::Path;

use crate::paths::RawLayout;

#[derive(Debug, thiserror::Error)]
pub enum SymlinkError {
    #[error("io error updating latest symlink: {0}")]
    Io(#[from] std::io::Error),
}

/// Repoints `raw/latest` at `snapshot_dir_name` (a bare directory name, resolved
/// relative to `raw/` itself — matching `ln -sfn gtfs_fp2026_20260805 raw/latest`).
pub fn advance_latest(layout: &RawLayout, snapshot_dir_name: &str) -> Result<(), SymlinkError> {
    let tmp_path = layout.latest_symlink_tmp_path(std::process::id());
    // Defensive: a previous crash mid-swap could have left this exact temp path
    // behind (same pid is unlikely but not impossible across reboots with pid
    // reuse); `symlink` fails with AlreadyExists otherwise.
    if tmp_path.exists() || tmp_path.symlink_metadata().is_ok() {
        std::fs::remove_file(&tmp_path)?;
    }

    std::os::unix::fs::symlink(snapshot_dir_name, &tmp_path)?;
    std::fs::rename(&tmp_path, layout.latest_symlink_path())?;
    Ok(())
}

/// Reads the current target of `raw/latest`, if the symlink exists.
pub fn read_latest(layout: &RawLayout) -> Result<Option<String>, SymlinkError> {
    let path = layout.latest_symlink_path();
    match std::fs::read_link(&path) {
        Ok(target) => Ok(Some(target_file_name(&target))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn target_file_name(target: &Path) -> String {
    target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.to_string_lossy().to_string())
}
