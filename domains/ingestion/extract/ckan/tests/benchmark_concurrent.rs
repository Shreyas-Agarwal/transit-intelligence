//! Wall-clock benchmark: sequential (concurrency=1) vs concurrent processing
//! of 4 synthetic GTFS versions through the real archive extraction and
//! CSV→Parquet pipeline.
//!
//! Run with:
//!   cargo test -p ckan --test benchmark_concurrent -- --nocapture --ignored
//!
//! (Marked `#[ignore]` so they don't run in CI by default — these generate
//! large synthetic files and measure real elapsed time.)

use std::io::Write as _;
use std::sync::Arc;
use std::time::Instant;

use ckan::archive;
use ckan::domain::VersionId;
use ckan::manifest::{self, SidecarStatus, SnapshotMeta};
use ckan::parquet_convert;
use ckan::paths::RawLayout;

const REQUIRED_GTFS: &[&str] = &[
    "stops.txt",
    "trips.txt",
    "routes.txt",
    "stop_times.txt",
    "calendar_dates.txt",
];

/// Build a synthetic GTFS zip with `rows_per_file` rows per member file.
fn build_synthetic_zip(path: &std::path::Path, rows_per_file: usize) {
    let file = std::fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for name in REQUIRED_GTFS {
        w.start_file(*name, opts).unwrap();
        w.write_all(b"id,value_a,value_b,value_c,value_d,value_e\n")
            .unwrap();
        for i in 0..rows_per_file {
            w.write_all(
                format!("{i},val_{i},{},{},{i},{}\n", i * 2, i * 3, i % 100).as_bytes(),
            )
            .unwrap();
        }
    }
    w.finish().unwrap();
}

/// Run the full archive+parquet pipeline for one version (synchronous).
fn pipeline_one_sync(
    layout: &RawLayout,
    dir_name: &str,
    version_str: &str,
    zip_src: &std::path::Path,
) -> SnapshotMeta {
    let version = VersionId::parse(version_str).unwrap();
    let zip_dst = layout.staging_zip_path(dir_name);
    let extract_dir = layout.staging_extract_dir(dir_name);
    let parquet_dir = layout.staging_parquet_dir(dir_name);

    if let Some(p) = zip_dst.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::copy(zip_src, &zip_dst).unwrap();
    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir).unwrap();
    }
    if parquet_dir.exists() {
        std::fs::remove_dir_all(&parquet_dir).unwrap();
    }

    std::fs::create_dir_all(&extract_dir).unwrap();
    archive::validate_and_extract(&zip_dst, &extract_dir).unwrap();
    std::fs::create_dir_all(&parquet_dir).unwrap();
    parquet_convert::convert_directory(&extract_dir, &parquet_dir).unwrap();
    std::fs::remove_dir_all(&extract_dir).unwrap();

    let final_dir = layout.final_dir(dir_name);
    if final_dir.exists() {
        std::fs::remove_dir_all(&final_dir).unwrap();
    }
    std::fs::rename(&parquet_dir, &final_dir).unwrap();
    let _ = std::fs::remove_file(&zip_dst);

    let meta = SnapshotMeta {
        version: version.clone(),
        source_url: format!("bench://{version_str}"),
        downloaded_at: "2026-08-01T00:00:00Z".parse().unwrap(),
        archive_size_bytes: 0,
        archive_sha256: "bench".to_string(),
        publisher_last_modified: None,
        etag: None,
        extract_path: final_dir.to_string_lossy().to_string(),
        status: SidecarStatus::Verified,
    };
    manifest::write_sidecar(layout, dir_name, &meta).unwrap();
    meta
}

/// Sequential baseline: 4 versions processed one after another.
#[test]
#[ignore = "wall-clock benchmark; run with -- --ignored --nocapture"]
fn bench_sequential_4_versions() {
    const ROWS: usize = 50_000;
    const N: usize = 4;

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let zip_fixture = tmp.path().join("fixture.zip");
    build_synthetic_zip(&zip_fixture, ROWS);
    let zip_kb = std::fs::metadata(&zip_fixture).unwrap().len() / 1024;

    let versions: Vec<(String, String)> = (0..N)
        .map(|i| {
            let v = format!("2026080{}", i + 1);
            (format!("gtfs_fp2026_{v}"), v)
        })
        .collect();

    println!(
        "\n[SEQ  N={N}] {ROWS} rows/file × {} files | zip={zip_kb} KiB",
        REQUIRED_GTFS.len()
    );

    let t = Instant::now();
    for (dir, ver) in &versions {
        pipeline_one_sync(&layout, dir, ver, &zip_fixture);
    }
    let elapsed = t.elapsed();

    println!(
        "[SEQ  N={N}] total={:.2}s  avg/version={:.2}s",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() / N as f64
    );
}

/// Concurrent: 4 versions processed with max_concurrent=4 via spawn_blocking tasks.
/// This mirrors exactly what `pipeline::run` does in the new implementation.
#[test]
#[ignore = "wall-clock benchmark; run with -- --ignored --nocapture"]
fn bench_concurrent_4_versions_max4() {
    const ROWS: usize = 50_000;
    const N: usize = 4;
    const MAX: usize = 4;

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let zip_fixture = tmp.path().join("fixture.zip");
    build_synthetic_zip(&zip_fixture, ROWS);
    let zip_kb = std::fs::metadata(&zip_fixture).unwrap().len() / 1024;

    let versions: Vec<(String, String)> = (0..N)
        .map(|i| {
            let v = format!("2026090{}", i + 1);
            (format!("gtfs_fp2026_{v}"), v)
        })
        .collect();

    println!(
        "\n[CON  N={N} max={MAX}] {ROWS} rows/file × {} files | zip={zip_kb} KiB",
        REQUIRED_GTFS.len()
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let t = Instant::now();
    rt.block_on(async {
        use tokio::sync::Semaphore;
        use tokio::task::JoinSet;

        let sem = Arc::new(Semaphore::new(MAX));
        let mut set: JoinSet<()> = JoinSet::new();

        for (dir, ver) in versions.clone() {
            let permit = Arc::clone(&sem).acquire_owned().await.unwrap();
            let layout = layout.clone();
            let zip = zip_fixture.clone();
            set.spawn(async move {
                tokio::task::spawn_blocking(move || {
                    pipeline_one_sync(&layout, &dir, &ver, &zip);
                    drop(permit);
                })
                .await
                .unwrap();
            });
        }
        while set.join_next().await.is_some() {}
    });
    let elapsed = t.elapsed();

    println!(
        "[CON  N={N} max={MAX}] total={:.2}s  apparent/version={:.2}s",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() / N as f64
    );
}

/// Concurrent with max=2 to show partial concurrency benefit.
#[test]
#[ignore = "wall-clock benchmark; run with -- --ignored --nocapture"]
fn bench_concurrent_4_versions_max2() {
    const ROWS: usize = 50_000;
    const N: usize = 4;
    const MAX: usize = 2;

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let zip_fixture = tmp.path().join("fixture.zip");
    build_synthetic_zip(&zip_fixture, ROWS);
    let zip_kb = std::fs::metadata(&zip_fixture).unwrap().len() / 1024;

    let versions: Vec<(String, String)> = (0..N)
        .map(|i| {
            let v = format!("2026100{}", i + 1);
            (format!("gtfs_fp2026_{v}"), v)
        })
        .collect();

    println!(
        "\n[CON  N={N} max={MAX}] {ROWS} rows/file × {} files | zip={zip_kb} KiB",
        REQUIRED_GTFS.len()
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let t = Instant::now();
    rt.block_on(async {
        use tokio::sync::Semaphore;
        use tokio::task::JoinSet;

        let sem = Arc::new(Semaphore::new(MAX));
        let mut set: JoinSet<()> = JoinSet::new();

        for (dir, ver) in versions.clone() {
            let permit = Arc::clone(&sem).acquire_owned().await.unwrap();
            let layout = layout.clone();
            let zip = zip_fixture.clone();
            set.spawn(async move {
                tokio::task::spawn_blocking(move || {
                    pipeline_one_sync(&layout, &dir, &ver, &zip);
                    drop(permit);
                })
                .await
                .unwrap();
            });
        }
        while set.join_next().await.is_some() {}
    });
    let elapsed = t.elapsed();

    println!(
        "[CON  N={N} max={MAX}] total={:.2}s  apparent/version={:.2}s",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() / N as f64
    );
}
