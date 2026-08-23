//! Integration tests for the explicit per-version processing pipeline
//! (implementation plan Phase 4): `process_snapshot`'s Claim/Complete/Fail
//! wiring into the durable control-plane state, and a full
//! download → verify → extract → validate → convert → publish run through a
//! real (local) HTTP download — nothing stubbed.

use std::io::Write as _;

use ckan::domain::{UpstreamResource, VersionId};
use ckan::paths::RawLayout;
use ckan::snapshot::process_snapshot;
use ckan::work_state::{self, VersionWork, WorkState};

const REQUIRED_GTFS: &[&str] = &[
    "stops.txt",
    "trips.txt",
    "routes.txt",
    "stop_times.txt",
    "calendar_dates.txt",
];

fn build_valid_zip_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        for name in REQUIRED_GTFS {
            w.start_file(*name, opts).unwrap();
            w.write_all(b"col_a,col_b\n1,2\n").unwrap();
        }
        w.finish().unwrap();
    }
    buf
}

fn build_invalid_zip_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("README.txt", opts).unwrap();
        w.write_all(b"not a gtfs feed\n").unwrap();
        w.finish().unwrap();
    }
    buf
}

/// A minimal single-request HTTP/1.1 server: accepts one connection, ignores
/// the request, and answers with `body` as a 200 OK with a matching
/// Content-Length. Enough for `reqwest` to parse; avoids pulling in a mocking
/// dependency for what's otherwise a one-shot fixture server.
async fn serve_one_response(body: Vec<u8>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            if n == 0 || buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(header.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}/fixture.zip")
}

fn resource(version: &str, url: String) -> UpstreamResource {
    UpstreamResource {
        version: VersionId::parse(version).unwrap(),
        name_prefix: "gtfs_fp2026".to_string(),
        download_url: url,
        original_filename: format!("GTFS_FP2026_{version}.zip"),
        publisher_last_modified: None,
        upstream_hash: None,
    }
}

fn queued_work(version: &str, url: &str) -> VersionWork {
    let mut work = VersionWork::discovered(VersionId::parse(version).unwrap(), url.to_string());
    work.queue().unwrap();
    work
}

/// The complete pipeline, end to end, through a real local HTTP download:
/// claim → download → verify → extract → validate → convert → publish →
/// complete. This is the plan's required integration test — every stage
/// runs for real.
#[tokio::test]
async fn full_pipeline_download_through_publish_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let url = serve_one_response(build_valid_zip_bytes()).await;
    let resource = resource("20260805", url.clone());
    let work = queued_work("20260805", &url);
    let http = reqwest::Client::new();

    let outcome = process_snapshot(
        &http,
        &layout,
        &resource,
        work,
        Some("test-worker".to_string()),
    )
    .await;

    let meta = outcome
        .meta
        .expect("the pipeline must succeed for a structurally valid archive");
    assert_eq!(outcome.work.state, WorkState::Published);
    assert_eq!(outcome.work.attempt, 1);
    assert!(
        outcome.work.worker_id.is_none(),
        "ownership must be released on publish"
    );
    assert!(outcome.work.completed_at.is_some());

    let final_dir = layout.final_dir("gtfs_fp2026_20260805");
    assert!(
        final_dir.exists(),
        "the snapshot must actually be published"
    );
    assert!(final_dir.join("stops.parquet").exists());
    assert_eq!(meta.extract_path, final_dir.to_string_lossy());
    assert!(
        final_dir.join(".snapshot-meta.json").exists(),
        "the sidecar must be written as part of Complete"
    );

    // The control-plane record was actually persisted, not just returned.
    let persisted = work_state::scan_work_states(&layout);
    assert_eq!(
        persisted[&VersionId::parse("20260805").unwrap()].state,
        WorkState::Published
    );
}

/// A structurally invalid archive fails at the Extract+Validate stage: the
/// control plane records FAILED (not PUBLISHED, and never left RUNNING), and
/// no final snapshot directory is ever created.
#[tokio::test]
async fn a_structurally_invalid_archive_fails_and_records_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let url = serve_one_response(build_invalid_zip_bytes()).await;
    let resource = resource("20260805", url.clone());
    let work = queued_work("20260805", &url);
    let http = reqwest::Client::new();

    let outcome = process_snapshot(&http, &layout, &resource, work, None).await;

    assert!(outcome.meta.is_err());
    assert_eq!(outcome.work.state, WorkState::Failed);
    assert!(outcome.work.last_error.is_some());
    assert!(!layout.final_dir("gtfs_fp2026_20260805").exists());

    let persisted = work_state::scan_work_states(&layout);
    assert_eq!(
        persisted[&VersionId::parse("20260805").unwrap()].state,
        WorkState::Failed
    );
}

/// A download that never connects at all (nothing listening) still fails
/// cleanly, at the Download stage, and is recorded as FAILED — never left
/// stuck in RUNNING with no way to know it needs recovery.
#[tokio::test]
async fn a_download_failure_is_recorded_as_failed_not_left_running() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    // Port 0 in a URL is never a real destination; connecting fails immediately.
    let resource = resource("20260805", "http://127.0.0.1:0/fixture.zip".to_string());
    let work = queued_work("20260805", &resource.download_url);
    let http = reqwest::Client::new();

    let outcome = process_snapshot(&http, &layout, &resource, work, None).await;

    assert!(outcome.meta.is_err());
    assert_eq!(outcome.work.state, WorkState::Failed);
    let persisted = work_state::scan_work_states(&layout);
    assert_eq!(
        persisted[&VersionId::parse("20260805").unwrap()].state,
        WorkState::Failed
    );
}

/// Claiming a version that isn't actually QUEUED (a caller bug, never a
/// normal runtime condition) is rejected outright rather than silently
/// processed from the wrong state.
#[tokio::test]
async fn claiming_a_non_queued_version_is_rejected_without_processing() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());

    let resource = resource("20260805", "http://127.0.0.1:1/unused".to_string());
    // Still DISCOVERED — never queued — so Claim must reject it.
    let work = VersionWork::discovered(
        VersionId::parse("20260805").unwrap(),
        resource.download_url.clone(),
    );

    let outcome = process_snapshot(&reqwest::Client::new(), &layout, &resource, work, None).await;

    assert!(outcome.meta.is_err());
    assert_eq!(
        outcome.work.state,
        WorkState::Discovered,
        "an invalid claim must not silently mutate state or attempt processing"
    );
}
