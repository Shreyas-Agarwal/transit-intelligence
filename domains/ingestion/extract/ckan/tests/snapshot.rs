//! Integration tests for the explicit per-version processing pipeline
//! (implementation plan Phase 4): `process_snapshot`'s Claim/Complete/Fail
//! wiring into the durable control-plane state, and a full
//! download → verify → extract → validate → convert → publish run through a
//! real (local) HTTP download — nothing stubbed.

use std::io::Write as _;

use ckan::concurrency::ResourcePermits;
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
    let permits = ResourcePermits::new(4, 4);

    let outcome = process_snapshot(
        &http,
        &layout,
        &resource,
        &permits,
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
    let permits = ResourcePermits::new(4, 4);

    let outcome = process_snapshot(&http, &layout, &resource, &permits, work, None).await;

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
    let permits = ResourcePermits::new(4, 4);

    let outcome = process_snapshot(&http, &layout, &resource, &permits, work, None).await;

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

    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(outcome.meta.is_err());
    assert_eq!(
        outcome.work.state,
        WorkState::Discovered,
        "an invalid claim must not silently mutate state or attempt processing"
    );
}

/// Two full real pipeline runs, sharing one tightly-capped `ResourcePermits`
/// (capacity 1 for both pools), run one after another: if either the
/// Download permit or the processing permit leaked (never released), the
/// second run would starve. Proves the Phase 5 wiring — not just the
/// mechanism in isolation (see `concurrency::tests`) — doesn't leak permits
/// across a real download → verify → extract → validate → convert → publish
/// pass.
#[tokio::test]
async fn permits_are_not_leaked_across_real_pipeline_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();
    let permits = ResourcePermits::new(1, 1);
    let http = reqwest::Client::new();

    for version in ["20260805", "20260812"] {
        let url = serve_one_response(build_valid_zip_bytes()).await;
        let resource = resource(version, url.clone());
        let work = queued_work(version, &url);

        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            process_snapshot(&http, &layout, &resource, &permits, work, None),
        )
        .await
        .expect("a leaked permit would hang this call forever; it must not");

        assert!(outcome.meta.is_ok(), "each run must still succeed");
        assert_eq!(permits.available_download_permits(), 1);
        assert_eq!(permits.available_processing_permits(), 1);
    }
}

// ===========================================================================
// Phase 6: stage-aware crash recovery — failure-injection tests.
//
// A real process crash can't be triggered from a test; each test instead
// directly constructs the exact filesystem state a crash at that point would
// have left (matching the pre-existing style in tests/pipeline_concurrent.rs),
// then runs the real `process_snapshot` against it and checks recovery.
//
// Several tests give the version an unreachable download URL on purpose: if
// `process_snapshot` still succeeds, that's direct proof the Download stage
// was actually skipped — a network call to that address cannot succeed.
// ===========================================================================

fn unreachable_url() -> String {
    // Port 0 is never a real destination; connecting to it fails immediately.
    "http://127.0.0.1:0/fixture.zip".to_string()
}

fn read_parquet_column(path: &std::path::Path, column: &str) -> Vec<String> {
    use arrow_array::{Array, StringArray};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let file = std::fs::File::open(path).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let mut values = Vec::new();
    for batch in reader {
        let batch = batch.unwrap();
        let idx = batch.schema().index_of(column).unwrap();
        let col = batch
            .column(idx)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        for i in 0..col.len() {
            values.push(col.value(i).to_string());
        }
    }
    values
}

fn write_extraction(dir: &std::path::Path, col_b_value: &str) {
    std::fs::create_dir_all(dir).unwrap();
    for name in REQUIRED_GTFS {
        std::fs::write(dir.join(name), format!("col_a,col_b\n1,{col_b_value}\n")).unwrap();
    }
}

fn write_conversion(
    extract_dir: &std::path::Path,
    parquet_dir: &std::path::Path,
    col_b_value: &str,
) {
    write_extraction(extract_dir, col_b_value);
    std::fs::create_dir_all(parquet_dir).unwrap();
    ckan::parquet_convert::convert_directory(extract_dir, parquet_dir).unwrap();
}

/// Crash point: Download. Only a `.zip.part` is left — never resumable — so
/// this must restart Download from scratch and still succeed via the real
/// server.
#[tokio::test]
async fn recovery_at_download_crash_point_restarts_download() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let url = serve_one_response(build_valid_zip_bytes()).await;
    let dir_name = "gtfs_fp2026_20260805";
    std::fs::write(
        layout.staging_part_path(dir_name),
        b"an interrupted, unusable partial download",
    )
    .unwrap();

    let resource = resource("20260805", url.clone());
    let work = queued_work("20260805", &url);
    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(
        outcome.meta.is_ok(),
        "must recover by restarting the download"
    );
    assert_eq!(outcome.work.state, WorkState::Published);
    assert!(!layout.staging_part_path(dir_name).exists());
}

/// Crash point: right after Download, before/during Verify. A complete
/// `.zip` exists; recovery must re-verify it from disk and skip re-fetching
/// it — proven here by giving the version an unreachable URL.
#[tokio::test]
async fn recovery_at_verify_crash_point_reverifies_without_redownloading() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let dir_name = "gtfs_fp2026_20260805";
    std::fs::write(layout.staging_zip_path(dir_name), build_valid_zip_bytes()).unwrap();

    let resource = resource("20260805", unreachable_url());
    let work = queued_work("20260805", &resource.download_url);
    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(
        outcome.meta.is_ok(),
        "an unreachable URL must not matter — the archive was already downloaded: {:?}",
        outcome.meta.err()
    );
    assert_eq!(outcome.work.state, WorkState::Published);
}

/// Crash point: Extract. An incomplete extraction is left behind; recovery
/// must discard it and re-extract from the (already-verified) zip.
#[tokio::test]
async fn recovery_at_extract_crash_point_discards_and_reextracts() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let dir_name = "gtfs_fp2026_20260805";
    std::fs::write(layout.staging_zip_path(dir_name), build_valid_zip_bytes()).unwrap();
    let extract_dir = layout.staging_extract_dir(dir_name);
    std::fs::create_dir_all(&extract_dir).unwrap();
    std::fs::write(extract_dir.join("stops.txt"), b"col_a,col_b\n1,2\n").unwrap();
    // The other 4 required members are missing: an incomplete extraction.

    let resource = resource("20260805", unreachable_url());
    let work = queued_work("20260805", &resource.download_url);
    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(
        outcome.meta.is_ok(),
        "must recover by re-extracting: {:?}",
        outcome.meta.err()
    );
    let final_dir = layout.final_dir(dir_name);
    for name in REQUIRED_GTFS {
        let stem = name.trim_end_matches(".txt");
        assert!(
            final_dir.join(format!("{stem}.parquet")).exists(),
            "{stem}.parquet must exist after a full re-extraction"
        );
    }
}

/// Crash point: right after Extract, before/during Validate. A complete,
/// valid extraction exists; recovery must trust it as-is (skip Download and
/// Extract entirely) rather than re-extracting from the zip — proven by
/// planting an extraction whose *content* differs from what the zip would
/// actually produce, and confirming the published output reflects the
/// planted content, not the zip's.
#[tokio::test]
async fn recovery_at_validate_crash_point_resumes_from_existing_extraction_without_reextracting() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let dir_name = "gtfs_fp2026_20260805";
    // The zip, if actually re-extracted, would produce col_b = "from-zip".
    let mut zip_buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
        let opts = zip::write::SimpleFileOptions::default();
        for name in REQUIRED_GTFS {
            w.start_file(*name, opts).unwrap();
            w.write_all(b"col_a,col_b\n1,from-zip\n").unwrap();
        }
        w.finish().unwrap();
    }
    std::fs::write(layout.staging_zip_path(dir_name), zip_buf).unwrap();
    // The planted extraction has different, distinguishable content.
    write_extraction(
        &layout.staging_extract_dir(dir_name),
        "from-resumed-extraction",
    );

    let resource = resource("20260805", unreachable_url());
    let work = queued_work("20260805", &resource.download_url);
    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(outcome.meta.is_ok(), "{:?}", outcome.meta.err());
    let final_dir = layout.final_dir(dir_name);
    let values = read_parquet_column(&final_dir.join("stops.parquet"), "col_b");
    assert_eq!(
        values,
        vec!["from-resumed-extraction".to_string()],
        "the planted extraction must be reused as-is, not re-extracted from the zip"
    );
}

/// Crash point: Convert. An incomplete conversion is left behind; recovery
/// must discard it and reconvert from the (already-valid) extraction.
#[tokio::test]
async fn recovery_at_convert_crash_point_discards_incomplete_conversion_and_reconverts() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let dir_name = "gtfs_fp2026_20260805";
    std::fs::write(layout.staging_zip_path(dir_name), build_valid_zip_bytes()).unwrap();
    write_extraction(&layout.staging_extract_dir(dir_name), "extracted-value");
    let parquet_dir = layout.staging_parquet_dir(dir_name);
    std::fs::create_dir_all(&parquet_dir).unwrap();
    std::fs::write(
        parquet_dir.join("stops.parquet"),
        b"not a real parquet file",
    )
    .unwrap();
    // The other 4 required members were never converted: incomplete.

    let resource = resource("20260805", unreachable_url());
    let work = queued_work("20260805", &resource.download_url);
    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(
        outcome.meta.is_ok(),
        "must recover by reconverting: {:?}",
        outcome.meta.err()
    );
    let final_dir = layout.final_dir(dir_name);
    let values = read_parquet_column(&final_dir.join("stops.parquet"), "col_b");
    assert_eq!(
        values,
        vec!["extracted-value".to_string()],
        "stops.parquet must be the freshly (and correctly) reconverted file, not the garbage placeholder"
    );
    for name in REQUIRED_GTFS {
        let stem = name.trim_end_matches(".txt");
        assert!(final_dir.join(format!("{stem}.parquet")).exists());
    }
}

/// Crash point: after Convert, before Publish. A fully complete conversion
/// exists and `extract_staging` has already been cleaned up — exactly what
/// `run_stages`'s own successful-conversion cleanup leaves behind. Recovery
/// must skip straight to Publish: proven by an unreachable URL (Download
/// skipped) and by there being no `extract_staging` to re-extract from.
#[tokio::test]
async fn recovery_after_conversion_crash_point_resumes_straight_to_publish() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let dir_name = "gtfs_fp2026_20260805";
    std::fs::write(layout.staging_zip_path(dir_name), build_valid_zip_bytes()).unwrap();
    let extract_dir = layout.staging_extract_dir(dir_name);
    let parquet_dir = layout.staging_parquet_dir(dir_name);
    write_conversion(&extract_dir, &parquet_dir, "converted-before-crash");
    std::fs::remove_dir_all(&extract_dir).unwrap(); // matches run_stages's own post-conversion cleanup

    let resource = resource("20260805", unreachable_url());
    let work = queued_work("20260805", &resource.download_url);
    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(outcome.meta.is_ok(), "{:?}", outcome.meta.err());
    assert_eq!(outcome.work.state, WorkState::Published);
    let final_dir = layout.final_dir(dir_name);
    let values = read_parquet_column(&final_dir.join("stops.parquet"), "col_b");
    assert_eq!(values, vec!["converted-before-crash".to_string()]);
    assert!(
        !layout.staging_parquet_dir(dir_name).exists(),
        "staging must be gone — moved into place by Publish, not copied"
    );
}

/// Already-published data is never corrupted: attempting to reprocess a
/// version whose control-plane record is already PUBLISHED is rejected at
/// Claim, before any filesystem work happens — the existing published
/// snapshot must be completely untouched.
#[tokio::test]
async fn already_published_data_is_never_touched_by_a_reprocessing_attempt() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let url = serve_one_response(build_valid_zip_bytes()).await;
    let good_resource = resource("20260805", url.clone());
    let work = queued_work("20260805", &url);
    let permits = ResourcePermits::new(4, 4);
    let first = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &good_resource,
        &permits,
        work,
        None,
    )
    .await;
    assert_eq!(first.work.state, WorkState::Published);

    let final_dir = layout.final_dir("gtfs_fp2026_20260805");
    let before = read_parquet_column(&final_dir.join("stops.parquet"), "col_b");

    // A caller bug tries to reprocess it, pointed at a resource that would
    // produce different content if it actually ran.
    let poisoned_resource = resource("20260805", unreachable_url());
    let second = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &poisoned_resource,
        &permits,
        first.work,
        None,
    )
    .await;

    assert!(
        second.meta.is_err(),
        "a PUBLISHED record must reject reprocessing"
    );
    assert_eq!(
        second.work.state,
        WorkState::Published,
        "state must be unchanged"
    );
    let after = read_parquet_column(&final_dir.join("stops.parquet"), "col_b");
    assert_eq!(
        before, after,
        "the already-published snapshot must be byte-for-byte untouched"
    );
}

/// Recovery converges to the same final state as an uninterrupted run: a
/// fresh, uninterrupted pipeline and one resumed from an "after conversion"
/// crash point both end up PUBLISHED with equivalent data.
#[tokio::test]
async fn resumed_recovery_converges_to_the_same_final_state_as_an_uninterrupted_run() {
    let permits = ResourcePermits::new(4, 4);

    // -- uninterrupted run --------------------------------------------------
    let tmp_fresh = tempfile::tempdir().unwrap();
    let layout_fresh = RawLayout::new(tmp_fresh.path().to_path_buf());
    std::fs::create_dir_all(layout_fresh.staging_dir()).unwrap();
    let url = serve_one_response(build_valid_zip_bytes()).await;
    let fresh_outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout_fresh,
        &resource("20260805", url.clone()),
        &permits,
        queued_work("20260805", &url),
        None,
    )
    .await;

    // -- resumed run, crashed "after conversion" -----------------------------
    let tmp_resumed = tempfile::tempdir().unwrap();
    let layout_resumed = RawLayout::new(tmp_resumed.path().to_path_buf());
    std::fs::create_dir_all(layout_resumed.staging_dir()).unwrap();
    let dir_name = "gtfs_fp2026_20260805";
    std::fs::write(
        layout_resumed.staging_zip_path(dir_name),
        build_valid_zip_bytes(),
    )
    .unwrap();
    let extract_dir = layout_resumed.staging_extract_dir(dir_name);
    let parquet_dir = layout_resumed.staging_parquet_dir(dir_name);
    write_conversion(&extract_dir, &parquet_dir, "2"); // matches build_valid_zip_bytes's "1,2"
    std::fs::remove_dir_all(&extract_dir).unwrap();
    let resumed_outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout_resumed,
        &resource("20260805", unreachable_url()),
        &permits,
        queued_work("20260805", &unreachable_url()),
        None,
    )
    .await;

    let fresh_meta = fresh_outcome.meta.expect("uninterrupted run must succeed");
    let resumed_meta = resumed_outcome.meta.expect("resumed run must succeed");

    assert_eq!(fresh_outcome.work.state, WorkState::Published);
    assert_eq!(resumed_outcome.work.state, WorkState::Published);
    assert_eq!(
        fresh_meta.archive_sha256, resumed_meta.archive_sha256,
        "both runs downloaded/re-verified the identical archive"
    );

    let fresh_values = read_parquet_column(
        &layout_fresh.final_dir(dir_name).join("stops.parquet"),
        "col_b",
    );
    let resumed_values = read_parquet_column(
        &layout_resumed.final_dir(dir_name).join("stops.parquet"),
        "col_b",
    );
    assert_eq!(
        fresh_values, resumed_values,
        "the published data must be equivalent regardless of how it got there"
    );
}

/// No duplicate snapshots: after a resumed run publishes successfully, there
/// is exactly one final directory and no leftover staging artifacts for that
/// version.
#[tokio::test]
async fn resuming_does_not_leave_duplicate_or_stray_staging_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let dir_name = "gtfs_fp2026_20260805";
    std::fs::write(layout.staging_zip_path(dir_name), build_valid_zip_bytes()).unwrap();
    write_extraction(&layout.staging_extract_dir(dir_name), "2");

    let resource = resource("20260805", unreachable_url());
    let work = queued_work("20260805", &resource.download_url);
    let permits = ResourcePermits::new(4, 4);
    let outcome = process_snapshot(
        &reqwest::Client::new(),
        &layout,
        &resource,
        &permits,
        work,
        None,
    )
    .await;

    assert!(outcome.meta.is_ok(), "{:?}", outcome.meta.err());

    let final_entries: Vec<_> = std::fs::read_dir(layout.root())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy() == dir_name)
        .collect();
    assert_eq!(
        final_entries.len(),
        1,
        "exactly one final directory for this version"
    );

    assert!(!layout.staging_zip_path(dir_name).exists());
    assert!(!layout.staging_extract_dir(dir_name).exists());
    assert!(!layout.staging_parquet_dir(dir_name).exists());
}
