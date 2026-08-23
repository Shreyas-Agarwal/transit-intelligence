//! Integration tests for OpenTelemetry-based observability (implementation
//! plan Phase 7): spans are actually recorded — for both successful and
//! failed processing — nest the way the module doc comments describe, stay
//! internally consistent, and don't get confused between two versions
//! processed at the same time. Uses `ti_common::observability::testing`'s
//! in-memory exporters rather than the real (stdout) one, so assertions run
//! against what was actually recorded, not against console output.

use std::io::Write as _;
use std::time::{Duration, Instant};

use ckan::concurrency::ResourcePermits;
use ckan::domain::{UpstreamResource, VersionId};
use ckan::paths::RawLayout;
use ckan::snapshot::{ProcessOutcome, process_snapshot};
use ckan::work_state::VersionWork;
use opentelemetry::trace::Status;
use opentelemetry_sdk::trace::SpanData;
use tracing::Instrument as _;

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

/// Same one-shot raw-TCP fixture server as `tests/snapshot.rs` — avoids
/// pulling in a mocking dependency for what's otherwise a single canned
/// HTTP response.
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

/// Mirrors what `pipeline::run`'s worker closure does: wrap one version's
/// whole `process_snapshot` call in its own `version` span, the same parent
/// every pipeline-stage span (download/verify/extract/convert/publish)
/// nests under in production.
async fn run_one_version_under_its_own_span(
    layout: &RawLayout,
    version: &'static str,
    url: String,
) -> ProcessOutcome {
    let resource = resource(version, url.clone());
    let work = queued_work(version, &url);
    let http = reqwest::Client::new();
    let permits = ResourcePermits::new(4, 4);
    let span = tracing::info_span!("version", version = %version);
    process_snapshot(&http, layout, &resource, &permits, work, None)
        .instrument(span)
        .await
}

fn span_named<'a>(spans: &'a [SpanData], name: &str) -> Option<&'a SpanData> {
    spans.iter().find(|s| s.name == name)
}

fn version_attr(span: &SpanData) -> Option<String> {
    span.attributes
        .iter()
        .find(|kv| kv.key.as_str() == "version")
        .map(|kv| kv.value.to_string())
}

/// A successful version leaves a full, correctly nested span tree behind:
/// one `version` span, with `download`, `verify`, `extract`, `convert`, and
/// `publish` all recorded as its direct children, and none of them —
/// including `version` itself — marked as errored.
#[tokio::test]
async fn spans_are_recorded_for_a_successful_version() {
    let (otel, subscriber) = ti_common::observability::testing::init("ckan-test");
    let _guard = tracing::subscriber::set_default(subscriber);

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();
    let url = serve_one_response(build_valid_zip_bytes()).await;

    let outcome = run_one_version_under_its_own_span(&layout, "20260805", url).await;
    assert!(outcome.meta.is_ok(), "fixture archive must publish cleanly");

    drop(_guard);
    otel.flush();
    let spans = otel.spans.get_finished_spans().unwrap();

    let version_span = span_named(&spans, "version").expect("a `version` span must be recorded");
    assert_eq!(version_span.status, Status::Unset);
    for stage in ["download", "verify", "extract", "convert", "publish"] {
        let stage_span =
            span_named(&spans, stage).unwrap_or_else(|| panic!("`{stage}` span must be recorded"));
        assert_eq!(
            stage_span.parent_span_id,
            version_span.span_context.span_id(),
            "`{stage}` must be a direct child of `version`"
        );
        assert_eq!(
            stage_span.status,
            Status::Unset,
            "`{stage}` succeeded and must not be marked as errored"
        );
    }
}

/// A structurally invalid archive fails inside Extract: `download` and
/// `verify` still ran and are recorded (nothing about a later failure erases
/// earlier measurements), `extract` is recorded as the span that was running
/// when the failure happened, and the stages that never ran — `convert`,
/// `publish` — are correctly absent, not recorded as empty/zero-duration.
/// The span where the failure actually happened (`extract`), and its parent
/// `version` span, must both be marked as errored — but a stage that
/// completed normally before the failure (`download`, `verify`) must not be.
#[tokio::test]
async fn spans_are_still_recorded_when_a_version_fails() {
    let (otel, subscriber) = ti_common::observability::testing::init("ckan-test");
    let _guard = tracing::subscriber::set_default(subscriber);

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();
    let url = serve_one_response(build_invalid_zip_bytes()).await;

    let outcome = run_one_version_under_its_own_span(&layout, "20260805", url).await;
    assert!(
        outcome.meta.is_err(),
        "structurally invalid archive must fail"
    );

    drop(_guard);
    otel.flush();
    let spans = otel.spans.get_finished_spans().unwrap();

    let download_span = span_named(&spans, "download").expect("download span must exist");
    let verify_span = span_named(&spans, "verify").expect("verify span must exist");
    let extract_span = span_named(&spans, "extract").expect("extract span must exist");
    assert!(
        span_named(&spans, "convert").is_none(),
        "Convert never ran; it must not appear as a span at all"
    );
    assert!(
        span_named(&spans, "publish").is_none(),
        "Publish never ran; it must not appear as a span at all"
    );

    assert_eq!(
        download_span.status,
        Status::Unset,
        "a stage that completed normally must not be marked as errored"
    );
    assert_eq!(
        verify_span.status,
        Status::Unset,
        "a stage that completed normally must not be marked as errored"
    );
    assert!(
        matches!(extract_span.status, Status::Error { .. }),
        "the stage where the failure actually happened must be marked as errored, got {:?}",
        extract_span.status
    );

    let version_span = span_named(&spans, "version").expect("version span must exist");
    assert!(
        matches!(version_span.status, Status::Error { .. }),
        "the version as a whole must be marked as errored when it fails, got {:?}",
        version_span.status
    );
}

/// Every recorded span's own timing is self-consistent (never ends before it
/// starts), and every pipeline-stage span's window falls within its parent
/// `version` span's window — a stage cannot appear to run before its version
/// even started or after it finished.
#[tokio::test]
async fn stage_span_durations_are_consistent_with_the_parent_version_span() {
    let (otel, subscriber) = ti_common::observability::testing::init("ckan-test");
    let _guard = tracing::subscriber::set_default(subscriber);

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();
    let url = serve_one_response(build_valid_zip_bytes()).await;

    run_one_version_under_its_own_span(&layout, "20260805", url).await;

    drop(_guard);
    otel.flush();
    let spans = otel.spans.get_finished_spans().unwrap();
    assert!(!spans.is_empty());

    for span in &spans {
        assert!(
            span.end_time >= span.start_time,
            "`{}` must not end before it starts",
            span.name
        );
    }

    let version_span = span_named(&spans, "version").unwrap();
    for stage in ["download", "verify", "extract", "convert", "publish"] {
        let stage_span = span_named(&spans, stage).unwrap();
        assert!(
            stage_span.start_time >= version_span.start_time
                && stage_span.end_time <= version_span.end_time,
            "`{stage}` must fall entirely within its `version` span's own window"
        );
    }
}

/// Two versions processed at the same time — one that fails, one that
/// succeeds — must not corrupt each other's measurements, and observability
/// itself must not serialize what would otherwise run concurrently: both
/// spawned tasks complete without one blocking on the other, each ends up
/// with its own correctly attributed and independently complete span tree,
/// and the failed version's absence of a `publish` span never leaks onto the
/// successful one or vice versa.
#[tokio::test]
async fn a_failed_version_does_not_corrupt_measurements_for_a_concurrent_successful_one() {
    let (otel, subscriber) = ti_common::observability::testing::init("ckan-test");
    let _guard = tracing::subscriber::set_default(subscriber);

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    std::fs::create_dir_all(layout.staging_dir()).unwrap();

    let good_url = serve_one_response(build_valid_zip_bytes()).await;
    let bad_url = serve_one_response(build_invalid_zip_bytes()).await;

    let good_layout = layout.clone();
    let bad_layout = layout.clone();
    let started = Instant::now();
    let good = tokio::spawn(async move {
        run_one_version_under_its_own_span(&good_layout, "20260801", good_url).await
    });
    let bad = tokio::spawn(async move {
        run_one_version_under_its_own_span(&bad_layout, "20260802", bad_url).await
    });
    let (good_outcome, bad_outcome) = tokio::join!(good, bad);
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "concurrent processing must not be serialized by observability instrumentation \
         (took {elapsed:?})"
    );
    assert!(good_outcome.unwrap().meta.is_ok());
    assert!(bad_outcome.unwrap().meta.is_err());

    drop(_guard);
    otel.flush();
    let spans = otel.spans.get_finished_spans().unwrap();

    let version_spans: Vec<_> = spans.iter().filter(|s| s.name == "version").collect();
    assert_eq!(
        version_spans.len(),
        2,
        "each version gets its own `version` span"
    );

    let good_version_span = version_spans
        .iter()
        .find(|s| version_attr(s).as_deref() == Some("20260801"))
        .expect("the successful version's span must be recorded");
    let bad_version_span = version_spans
        .iter()
        .find(|s| version_attr(s).as_deref() == Some("20260802"))
        .expect("the failed version's span must be recorded");

    let publish_spans: Vec<_> = spans.iter().filter(|s| s.name == "publish").collect();
    assert_eq!(
        publish_spans.len(),
        1,
        "only the successful version ever reaches Publish"
    );
    assert_eq!(
        publish_spans[0].parent_span_id,
        good_version_span.span_context.span_id(),
        "the one `publish` span must belong to the successful version, not the failed one"
    );

    let extract_spans: Vec<_> = spans.iter().filter(|s| s.name == "extract").collect();
    assert_eq!(extract_spans.len(), 2, "both versions reach Extract");
    assert!(
        extract_spans
            .iter()
            .any(|s| s.parent_span_id == bad_version_span.span_context.span_id()),
        "the failed version's own `extract` span must still be recorded under its own version"
    );
}
