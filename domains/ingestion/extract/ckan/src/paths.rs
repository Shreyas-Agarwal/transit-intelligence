//! Filesystem layout under `domains/gtfs_s/raw/` — one place that knows every
//! path convention from the design doc (§2, §5, §11), so nothing else has to
//! hand-build paths.

use std::path::PathBuf;

pub struct RawLayout {
    raw_dir: PathBuf,
}

impl RawLayout {
    pub fn new(raw_dir: PathBuf) -> Self {
        Self { raw_dir }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.raw_dir
    }

    pub fn staging_dir(&self) -> PathBuf {
        self.raw_dir.join(".staging")
    }

    /// Path for the in-progress download before the transfer completes.
    pub fn staging_part_path(&self, snapshot_dir_name: &str) -> PathBuf {
        self.staging_dir()
            .join(format!("{snapshot_dir_name}.zip.part"))
    }

    /// Path the download is renamed to once the transfer completes without error.
    pub fn staging_zip_path(&self, snapshot_dir_name: &str) -> PathBuf {
        self.staging_dir().join(format!("{snapshot_dir_name}.zip"))
    }

    /// Staging extraction directory for the raw CSVs pulled out of the zip —
    /// never the final name, and never retained past Tier 1 validation +
    /// Parquet conversion (design doc §6, §8).
    pub fn staging_extract_dir(&self, snapshot_dir_name: &str) -> PathBuf {
        self.staging_dir().join(snapshot_dir_name)
    }

    /// Staging directory for the Parquet files converted from the CSVs above.
    /// This is what actually gets atomically renamed into `final_dir` — the
    /// CSV staging directory is deleted once conversion succeeds.
    pub fn staging_parquet_dir(&self, snapshot_dir_name: &str) -> PathBuf {
        self.staging_dir()
            .join(format!("{snapshot_dir_name}.parquet"))
    }

    pub fn final_dir(&self, snapshot_dir_name: &str) -> PathBuf {
        self.raw_dir.join(snapshot_dir_name)
    }

    pub fn sidecar_path_for_dir(&self, snapshot_dir_name: &str) -> PathBuf {
        self.final_dir(snapshot_dir_name)
            .join(".snapshot-meta.json")
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.raw_dir.join(".manifest.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.raw_dir.join(".updater.lock")
    }

    pub fn latest_symlink_path(&self) -> PathBuf {
        self.raw_dir.join("latest")
    }

    /// A temp path used only during the atomic symlink swap in [`crate::symlink`].
    pub fn latest_symlink_tmp_path(&self, pid: u32) -> PathBuf {
        self.raw_dir.join(format!(".latest.tmp.{pid}"))
    }

    /// Whether the given directory name looks like a snapshot directory rather
    /// than one of the reserved entries (`.staging`, `.manifest.json`, `latest`,
    /// `.updater.lock`, dotfiles in general).
    pub fn looks_like_snapshot_dir(name: &str) -> bool {
        !name.starts_with('.') && name != "latest"
    }
}
