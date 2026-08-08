//! Design doc §7: advancing `latest` must never leave it missing or broken,
//! and must only ever point at an existing snapshot directory.

use ckan::paths::RawLayout;
use ckan::symlink;

#[test]
fn advance_latest_creates_and_repoints_the_symlink() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.root()).unwrap();
    std::fs::create_dir_all(layout.final_dir("gtfs_fp2026_20260729")).unwrap();
    std::fs::create_dir_all(layout.final_dir("gtfs_fp2026_20260805")).unwrap();

    assert_eq!(symlink::read_latest(&layout).unwrap(), None);

    symlink::advance_latest(&layout, "gtfs_fp2026_20260729").unwrap();
    assert_eq!(
        symlink::read_latest(&layout).unwrap().as_deref(),
        Some("gtfs_fp2026_20260729")
    );

    // Repointing to a newer version must fully replace the old target, not
    // nest inside it (the `ln -f` vs `ln -sfn` footgun called out in the
    // design doc §7).
    symlink::advance_latest(&layout, "gtfs_fp2026_20260805").unwrap();
    assert_eq!(
        symlink::read_latest(&layout).unwrap().as_deref(),
        Some("gtfs_fp2026_20260805")
    );

    let metadata = std::fs::symlink_metadata(layout.latest_symlink_path()).unwrap();
    assert!(
        metadata.file_type().is_symlink(),
        "latest must remain a single symlink, not a directory"
    );
}

#[test]
fn no_leftover_temp_symlink_after_advance() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.root()).unwrap();
    std::fs::create_dir_all(layout.final_dir("gtfs_fp2026_20260805")).unwrap();

    symlink::advance_latest(&layout, "gtfs_fp2026_20260805").unwrap();

    let leftover: Vec<_> = std::fs::read_dir(layout.root())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(".latest.tmp"))
        .collect();
    assert!(
        leftover.is_empty(),
        "the temp symlink used for the atomic swap must not survive"
    );
}
