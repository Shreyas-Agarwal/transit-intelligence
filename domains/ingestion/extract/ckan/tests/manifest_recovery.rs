//! Design doc §2: "If [the manifest] is deleted or corrupted, it is
//! regenerated from the sidecars on the next run — this is a design
//! invariant... and should be exercised by a test."

use ckan::domain::VersionId;
use ckan::manifest::{self, ManifestStatus, SidecarStatus, SnapshotMeta};
use ckan::paths::RawLayout;

fn make_meta(version: &str, extract_path: &str) -> SnapshotMeta {
    SnapshotMeta {
        version: VersionId::parse(version).unwrap(),
        source_url: format!("https://example.invalid/{version}.zip"),
        downloaded_at: "2026-08-06T04:00:12Z".parse().unwrap(),
        archive_size_bytes: 1234,
        archive_sha256: "deadbeef".to_string(),
        publisher_last_modified: None,
        etag: None,
        extract_path: extract_path.to_string(),
        status: SidecarStatus::Verified,
    }
}

#[test]
fn manifest_rebuilds_identically_after_deletion() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.root()).unwrap();

    for (version, name) in [
        ("20260729", "gtfs_fp2026_20260729"),
        ("20260805", "gtfs_fp2026_20260805"),
    ] {
        let dir = layout.final_dir(name);
        std::fs::create_dir_all(&dir).unwrap();
        manifest::write_sidecar(&layout, name, &make_meta(version, dir.to_str().unwrap())).unwrap();
    }

    let installed_before = manifest::scan_sidecars(&layout);
    let manifest_before = manifest::rebuild_manifest(
        &installed_before,
        &Default::default(),
        "2026-08-06T04:00:20Z".parse().unwrap(),
    );
    manifest::write_manifest(&layout, &manifest_before).unwrap();

    // Simulate deletion/corruption of the rollup manifest.
    std::fs::remove_file(layout.manifest_path()).unwrap();
    assert!(!layout.manifest_path().exists());

    let installed_after = manifest::scan_sidecars(&layout);
    let manifest_after = manifest::rebuild_manifest(
        &installed_after,
        &Default::default(),
        "2026-08-06T04:00:20Z".parse().unwrap(),
    );

    assert_eq!(manifest_before.latest, manifest_after.latest);
    assert_eq!(
        manifest_before.versions.len(),
        manifest_after.versions.len()
    );
    for (version, entry) in &manifest_before.versions {
        let rebuilt = &manifest_after.versions[version];
        assert_eq!(entry.status, rebuilt.status);
        assert_eq!(entry.extract_path, rebuilt.extract_path);
    }
}

#[test]
fn latest_is_the_newest_verified_version_not_upload_order() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.root()).unwrap();

    // Written out of chronological order to make sure "latest" is computed
    // from the version id, not insertion order.
    for (version, name) in [
        ("20260805", "gtfs_fp2026_20260805"),
        ("20260624", "gtfs_fp2026_20260624"),
        ("20260729", "gtfs_fp2026_20260729"),
    ] {
        let dir = layout.final_dir(name);
        std::fs::create_dir_all(&dir).unwrap();
        manifest::write_sidecar(&layout, name, &make_meta(version, dir.to_str().unwrap())).unwrap();
    }

    let installed = manifest::scan_sidecars(&layout);
    let manifest = manifest::rebuild_manifest(
        &installed,
        &Default::default(),
        "2026-08-06T04:00:20Z".parse().unwrap(),
    );

    assert_eq!(manifest.latest, Some(VersionId::parse("20260805").unwrap()));
    assert_eq!(
        manifest.versions[&VersionId::parse("20260805").unwrap()].status,
        ManifestStatus::Verified
    );
    assert_eq!(
        manifest.versions[&VersionId::parse("20260729").unwrap()].status,
        ManifestStatus::Superseded
    );
}

#[test]
fn snapshot_directory_without_sidecar_is_not_treated_as_installed() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    // A pre-existing directory with no sidecar (e.g. the pre-automation
    // baseline, or manual tampering) — design doc §12, row 3.
    std::fs::create_dir_all(layout.final_dir("gtfs_fp2026_20260805")).unwrap();
    std::fs::write(
        layout.final_dir("gtfs_fp2026_20260805").join("stops.txt"),
        b"stop_id\n",
    )
    .unwrap();

    let installed = manifest::scan_sidecars(&layout);
    assert!(
        installed.is_empty(),
        "directory without a sidecar must not count as installed"
    );
}
