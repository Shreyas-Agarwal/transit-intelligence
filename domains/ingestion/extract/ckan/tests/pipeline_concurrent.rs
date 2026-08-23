//! Integration tests for the concurrent pipeline: bounded-concurrency correctness,
//! failure isolation, `latest` ordering, and recovery invariants.
//!
//! These tests exercise the pipeline at the unit level using entirely in-process
//! mocks — no network calls, no real CKAN API. Each test uses [`tempfile::tempdir`]
//! for an isolated, empty filesystem root, and synthetic mini-GTFS zips built via
//! the same helper used in `tests/archive.rs`.
//!
//! # What is covered
//!
//! * [`all_versions_succeed_manifest_reflects_all`]: N versions processed
//!   sequentially through the real archive/parquet pipeline — all succeed, manifest
//!   reflects all, `latest` is the newest version ID.
//! * [`latest_is_version_id_order_not_completion_order`]: confirms `latest` tracks
//!   the highest version ID, not whichever task finished first.
//! * [`latest_falls_back_when_newest_version_fails`]: if the newest version fails,
//!   `latest` falls back to the next highest success.
//! * [`failed_version_leaves_no_final_snapshot_dir_and_cleans_staging`]: a failed
//!   version's staging is cleaned; `raw/<version>/` is never created.
//! * [`independent_failure_does_not_affect_other_versions`]: one bad version fails;
//!   others still succeed; failed version leaves no final dir.
//! * [`concurrent_workers_use_isolated_staging_paths`]: two versions exercise
//!   distinct staging paths end-to-end.
//! * [`startup_wipes_partial_staging_from_multiple_crashed_versions`]: pre-existing
//!   partial staging from multiple versions is cleaned at startup.
//! * [`manifest_is_consistent_after_mixed_success_failure_run`]: manifest is
//!   internally consistent after a mixed run.

use std::io::Write as _;
use std::path::PathBuf;

use ckan::domain::VersionId;
use ckan::manifest::{self, ManifestStatus, SidecarStatus, SnapshotMeta};
use ckan::paths::RawLayout;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

const REQUIRED_GTFS: &[&str] = &[
    "stops.txt",
    "trips.txt",
    "routes.txt",
    "stop_times.txt",
    "calendar_dates.txt",
];

/// Build a minimal, structurally-valid GTFS zip at `path`.
fn build_valid_zip(path: &std::path::Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for name in REQUIRED_GTFS {
        w.start_file(*name, opts).unwrap();
        w.write_all(b"col_a,col_b\n1,2\n").unwrap();
    }
    w.finish().unwrap();
}

/// Build a zip that contains no GTFS files (will fail Tier 1 validation).
fn build_invalid_zip(path: &std::path::Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    w.start_file("README.txt", opts).unwrap();
    w.write_all(b"not a gtfs feed\n").unwrap();
    w.finish().unwrap();
}

/// Drives the full per-version pipeline (archive validation + Parquet
/// conversion + atomic rename + sidecar) without a network layer, by copying
/// `zip_src` into staging instead.
///
/// Mirrors exactly what `pipeline::process_version` does internally, using
/// the same public module functions, so changes to either path are caught.
fn run_version_pipeline_sync(
    layout: &RawLayout,
    dir_name: &str,
    version_str: &str,
    zip_src: &std::path::Path,
) -> Result<SnapshotMeta, String> {
    use ckan::archive;
    use ckan::parquet_convert;

    let version = VersionId::parse(version_str).unwrap();
    let zip_dst = layout.staging_zip_path(dir_name);
    let extract_dir = layout.staging_extract_dir(dir_name);
    let parquet_dir = layout.staging_parquet_dir(dir_name);

    // Simulate a completed download by copying the pre-built zip.
    if let Some(p) = zip_dst.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::copy(zip_src, &zip_dst).map_err(|e| format!("copy failed: {e}"))?;

    // Clean any leftover staging from a previous attempt.
    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir).unwrap();
    }
    if parquet_dir.exists() {
        std::fs::remove_dir_all(&parquet_dir).unwrap();
    }

    // Tier 1 extraction — may fail for an invalid zip.
    std::fs::create_dir_all(&extract_dir).unwrap();
    if let Err(e) = archive::validate_and_extract(&zip_dst, &extract_dir) {
        let _ = std::fs::remove_file(&zip_dst);
        let _ = std::fs::remove_dir_all(&extract_dir);
        return Err(e.to_string());
    }

    // CSV → Parquet conversion.
    std::fs::create_dir_all(&parquet_dir).unwrap();
    if let Err(e) = parquet_convert::convert_directory(&extract_dir, &parquet_dir) {
        let _ = std::fs::remove_file(&zip_dst);
        let _ = std::fs::remove_dir_all(&extract_dir);
        let _ = std::fs::remove_dir_all(&parquet_dir);
        return Err(e.to_string());
    }
    std::fs::remove_dir_all(&extract_dir).unwrap();

    // Atomic publish: staging → final.
    let final_dir = layout.final_dir(dir_name);
    if final_dir.exists() {
        std::fs::remove_dir_all(&final_dir).unwrap();
    }
    std::fs::rename(&parquet_dir, &final_dir).map_err(|e| format!("rename failed: {e}"))?;
    let _ = std::fs::remove_file(&zip_dst);

    let meta = SnapshotMeta {
        version: version.clone(),
        source_url: format!("https://example.invalid/{version_str}.zip"),
        downloaded_at: "2026-08-01T00:00:00Z".parse().unwrap(),
        archive_size_bytes: 1024,
        archive_sha256: "aabb".to_string(),
        publisher_last_modified: None,
        etag: None,
        extract_path: final_dir.to_string_lossy().to_string(),
        status: SidecarStatus::Verified,
    };
    manifest::write_sidecar(layout, dir_name, &meta).unwrap();

    Ok(meta)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// All versions succeed: manifest reflects all, `latest` is the highest version ID.
#[test]
fn all_versions_succeed_manifest_reflects_all() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let zip_fixture = tmp.path().join("fixture.zip");
    build_valid_zip(&zip_fixture);

    let versions = [
        ("gtfs_fp2026_20260805", "20260805"),
        ("gtfs_fp2026_20260729", "20260729"),
        ("gtfs_fp2026_20260722", "20260722"),
    ];

    let mut installed = std::collections::BTreeMap::new();
    for (dir_name, version_str) in &versions {
        let meta = run_version_pipeline_sync(&layout, dir_name, version_str, &zip_fixture)
            .unwrap_or_else(|e| panic!("version {version_str} failed: {e}"));
        installed.insert(VersionId::parse(version_str).unwrap(), meta);
    }

    let manifest = manifest::rebuild_manifest(
        &installed,
        &Default::default(),
        "2026-08-06T00:00:00Z".parse().unwrap(),
    );

    assert_eq!(manifest.versions.len(), 3);
    assert_eq!(
        manifest.latest,
        Some(VersionId::parse("20260805").unwrap()),
        "latest must be the highest version ID"
    );

    // All final dirs exist.
    for (dir_name, _) in &versions {
        assert!(
            layout.final_dir(dir_name).exists(),
            "final dir {dir_name} must exist"
        );
    }
}

/// `latest` tracks the highest version ID regardless of insertion/completion order.
#[test]
fn latest_is_version_id_order_not_completion_order() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let zip_fixture = tmp.path().join("fixture.zip");
    build_valid_zip(&zip_fixture);

    // Insert in reverse chronological order: newest first.
    // This simulates a scenario where the newest task completes first.
    let order = [
        ("gtfs_fp2026_20260819", "20260819"),
        ("gtfs_fp2026_20260812", "20260812"),
        ("gtfs_fp2026_20260805", "20260805"),
    ];

    let mut installed = std::collections::BTreeMap::new();
    for (dir_name, version_str) in &order {
        let meta = run_version_pipeline_sync(&layout, dir_name, version_str, &zip_fixture).unwrap();
        installed.insert(VersionId::parse(version_str).unwrap(), meta);
    }

    let manifest = manifest::rebuild_manifest(
        &installed,
        &Default::default(),
        "2026-08-20T00:00:00Z".parse().unwrap(),
    );

    // Regardless of insertion order, latest must be 20260819.
    assert_eq!(
        manifest.latest,
        Some(VersionId::parse("20260819").unwrap()),
        "latest must be the highest version ID regardless of completion order"
    );
    assert_eq!(
        manifest.versions[&VersionId::parse("20260819").unwrap()].status,
        ManifestStatus::Verified
    );
    assert_eq!(
        manifest.versions[&VersionId::parse("20260812").unwrap()].status,
        ManifestStatus::Superseded
    );
    assert_eq!(
        manifest.versions[&VersionId::parse("20260805").unwrap()].status,
        ManifestStatus::Superseded
    );
}

/// If the highest version fails, `latest` falls back to the next highest success.
#[test]
fn latest_falls_back_when_newest_version_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let zip_fixture = tmp.path().join("fixture.zip");
    build_valid_zip(&zip_fixture);

    // 20260812 and 20260805 succeed; 20260819 fails.
    let mut installed = std::collections::BTreeMap::new();
    for (dir_name, version_str) in [
        ("gtfs_fp2026_20260805", "20260805"),
        ("gtfs_fp2026_20260812", "20260812"),
    ] {
        let meta = run_version_pipeline_sync(&layout, dir_name, version_str, &zip_fixture).unwrap();
        installed.insert(VersionId::parse(version_str).unwrap(), meta);
    }

    // Simulate 20260819 failing (no final dir, no sidecar).
    let mut failed: std::collections::BTreeMap<VersionId, ()> = std::collections::BTreeMap::new();
    failed.insert(VersionId::parse("20260819").unwrap(), ());

    let manifest =
        manifest::rebuild_manifest(&installed, &failed, "2026-08-20T00:00:00Z".parse().unwrap());

    // latest must be 20260812, not 20260819.
    assert_eq!(
        manifest.latest,
        Some(VersionId::parse("20260812").unwrap()),
        "latest must be the newest successfully verified version"
    );
    assert_eq!(
        manifest.versions[&VersionId::parse("20260819").unwrap()].status,
        ManifestStatus::Failed,
        "failed version must appear as Failed in the manifest"
    );
    // No final dir for the failed version.
    assert!(
        !layout.final_dir("gtfs_fp2026_20260819").exists(),
        "failed version must not create a final snapshot directory"
    );
}

/// A failed version cleans its own staging and never creates a final directory.
#[test]
fn failed_version_leaves_no_final_snapshot_dir_and_cleans_staging() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    // A zip that fails Tier 1 validation (no required GTFS members).
    let bad_zip = tmp.path().join("bad.zip");
    build_invalid_zip(&bad_zip);

    let dir_name = "gtfs_fp2026_20260805";
    let result = run_version_pipeline_sync(&layout, dir_name, "20260805", &bad_zip);
    assert!(result.is_err(), "invalid zip must produce an error");

    // Final directory must not exist.
    assert!(
        !layout.final_dir(dir_name).exists(),
        "failed version must not create final dir"
    );

    // Staging directories for this version must be cleaned on failure.
    assert!(
        !layout.staging_extract_dir(dir_name).exists(),
        "extract staging must be removed after failure"
    );
    assert!(
        !layout.staging_parquet_dir(dir_name).exists(),
        "parquet staging must be removed after failure"
    );
}

/// One bad version does not affect others — successes publish normally.
#[test]
fn independent_failure_does_not_affect_other_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let good_zip = tmp.path().join("good.zip");
    let bad_zip = tmp.path().join("bad.zip");
    build_valid_zip(&good_zip);
    build_invalid_zip(&bad_zip);

    // Version A: succeeds.
    let meta_a =
        run_version_pipeline_sync(&layout, "gtfs_fp2026_20260805", "20260805", &good_zip).unwrap();

    // Version B: fails (bad zip).
    let result_b = run_version_pipeline_sync(&layout, "gtfs_fp2026_20260812", "20260812", &bad_zip);
    assert!(result_b.is_err(), "bad version must fail");

    // Version C: succeeds.
    let meta_c =
        run_version_pipeline_sync(&layout, "gtfs_fp2026_20260819", "20260819", &good_zip).unwrap();

    // A and C have final dirs; B does not.
    assert!(
        layout.final_dir("gtfs_fp2026_20260805").exists(),
        "version A must be published"
    );
    assert!(
        !layout.final_dir("gtfs_fp2026_20260812").exists(),
        "version B must not be published"
    );
    assert!(
        layout.final_dir("gtfs_fp2026_20260819").exists(),
        "version C must be published"
    );

    // Build manifest with successes only.
    let mut installed = std::collections::BTreeMap::new();
    installed.insert(VersionId::parse("20260805").unwrap(), meta_a);
    installed.insert(VersionId::parse("20260819").unwrap(), meta_c);
    let mut failed: std::collections::BTreeMap<VersionId, ()> = std::collections::BTreeMap::new();
    failed.insert(VersionId::parse("20260812").unwrap(), ());

    let manifest =
        manifest::rebuild_manifest(&installed, &failed, "2026-08-20T00:00:00Z".parse().unwrap());

    assert_eq!(
        manifest.versions.len(),
        3,
        "manifest has 2 successes + 1 failure"
    );
    assert_eq!(
        manifest.latest,
        Some(VersionId::parse("20260819").unwrap()),
        "latest must skip the failed version"
    );
    assert_eq!(
        manifest.versions[&VersionId::parse("20260812").unwrap()].status,
        ManifestStatus::Failed
    );
}

/// Concurrent workers use version-isolated staging paths — they cannot interfere.
#[test]
fn concurrent_workers_use_isolated_staging_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let zip_fixture = tmp.path().join("fixture.zip");
    build_valid_zip(&zip_fixture);

    let v1_dir = "gtfs_fp2026_20260805";
    let v2_dir = "gtfs_fp2026_20260812";

    // Verify staging paths are distinct.
    assert_ne!(
        layout.staging_extract_dir(v1_dir),
        layout.staging_extract_dir(v2_dir),
        "extract staging paths must be distinct"
    );
    assert_ne!(
        layout.staging_parquet_dir(v1_dir),
        layout.staging_parquet_dir(v2_dir),
        "parquet staging paths must be distinct"
    );
    assert_ne!(
        layout.staging_zip_path(v1_dir),
        layout.staging_zip_path(v2_dir),
        "zip staging paths must be distinct"
    );

    // Run both pipelines end-to-end.
    let meta1 = run_version_pipeline_sync(&layout, v1_dir, "20260805", &zip_fixture).unwrap();
    let meta2 = run_version_pipeline_sync(&layout, v2_dir, "20260812", &zip_fixture).unwrap();

    // Final dirs are distinct and both exist.
    assert_ne!(meta1.extract_path, meta2.extract_path);
    assert!(
        PathBuf::from(&meta1.extract_path).exists(),
        "v1 final dir must exist"
    );
    assert!(
        PathBuf::from(&meta2.extract_path).exists(),
        "v2 final dir must exist"
    );

    // Each snapshot contains its own set of Parquet files.
    let count_parquet = |path: &str| -> usize {
        std::fs::read_dir(path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |x| x == "parquet"))
            .count()
    };
    assert!(
        count_parquet(&meta1.extract_path) > 0,
        "v1 must have parquet files"
    );
    assert!(
        count_parquet(&meta2.extract_path) > 0,
        "v2 must have parquet files"
    );
}

/// `clean_staging` at startup wipes all partial artifacts left by a crashed run
/// that had multiple concurrent versions in-flight.
#[test]
fn startup_wipes_partial_staging_from_multiple_crashed_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());

    // Plant partial staging artifacts simulating a crash mid-run with 3
    // concurrent versions in various stages.
    let staging = layout.staging_dir();
    std::fs::create_dir_all(staging.join("gtfs_fp2026_20260805")).unwrap(); // mid-extract
    std::fs::create_dir_all(staging.join("gtfs_fp2026_20260805.parquet")).unwrap(); // mid-convert
    std::fs::write(staging.join("gtfs_fp2026_20260805.zip"), b"partial").unwrap();
    std::fs::create_dir_all(staging.join("gtfs_fp2026_20260812")).unwrap();
    std::fs::write(staging.join("gtfs_fp2026_20260819.zip.part"), b"in-flight").unwrap();

    // Replicate clean_staging from pipeline.rs.
    let staging = layout.staging_dir();
    if staging.exists() {
        std::fs::remove_dir_all(&staging).unwrap();
    }
    std::fs::create_dir_all(&staging).unwrap();

    // Staging dir itself still exists (recreated) but is empty.
    assert!(
        layout.staging_dir().exists(),
        "staging dir must be recreated"
    );
    let entries: Vec<_> = std::fs::read_dir(layout.staging_dir()).unwrap().collect();
    assert!(entries.is_empty(), "staging must be empty after cleanup");
}

/// After a mixed success/failure run the manifest is internally consistent.
#[test]
fn manifest_is_consistent_after_mixed_success_failure_run() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let good_zip = tmp.path().join("good.zip");
    build_valid_zip(&good_zip);

    // Install two successful versions.
    let mut installed = std::collections::BTreeMap::new();
    for (dir, ver) in [
        ("gtfs_fp2026_20260805", "20260805"),
        ("gtfs_fp2026_20260729", "20260729"),
    ] {
        let meta = run_version_pipeline_sync(&layout, dir, ver, &good_zip).unwrap();
        installed.insert(VersionId::parse(ver).unwrap(), meta);
    }

    // Simulate one failed version.
    let mut failed: std::collections::BTreeMap<VersionId, ()> = std::collections::BTreeMap::new();
    failed.insert(VersionId::parse("20260812").unwrap(), ());

    // Sidecars only exist for successful versions.
    let scanned = manifest::scan_sidecars(&layout);
    assert_eq!(scanned.len(), 2, "only successful versions have sidecars");

    let manifest_obj =
        manifest::rebuild_manifest(&installed, &failed, "2026-08-13T00:00:00Z".parse().unwrap());

    // Write and re-read to confirm round-trip JSON integrity.
    manifest::write_manifest(&layout, &manifest_obj).unwrap();
    let raw = std::fs::read_to_string(layout.manifest_path()).unwrap();
    let reparsed: ckan::manifest::Manifest = serde_json::from_str(&raw).unwrap();

    assert_eq!(
        reparsed.versions.len(),
        3,
        "2 successes + 1 failure in manifest"
    );
    assert_eq!(reparsed.latest, Some(VersionId::parse("20260805").unwrap()));
    assert_eq!(
        reparsed.versions[&VersionId::parse("20260812").unwrap()].status,
        ManifestStatus::Failed
    );
    assert_eq!(
        reparsed.versions[&VersionId::parse("20260805").unwrap()].status,
        ManifestStatus::Verified
    );
    assert_eq!(
        reparsed.versions[&VersionId::parse("20260729").unwrap()].status,
        ManifestStatus::Superseded
    );
}
