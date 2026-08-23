//! End-to-end V2 performance baseline (implementation plan Phase 8).
//!
//! Exercises the real `ckan::pipeline::run` entrypoint — discovery through
//! publish, through the real bounded queue and worker pool, with real
//! archive extraction and Parquet conversion — against a fixed, reproducible
//! local workload: a fixture CKAN `package_show` response plus one fixture
//! download server per version, each serving a synthetic but deterministic
//! GTFS archive. Nothing here talks to the live CKAN API or the live
//! opentransportdata.swiss servers, and no prior test in this crate actually
//! drives `pipeline::run` itself end to end (existing tests replay its
//! per-version pipeline directly, bypassing discovery) — this is the first
//! one that does.
//!
//! This is deliberately end-to-end, not a microbenchmark of any one stage:
//! the number that matters is what one real invocation actually takes, the
//! same shape of number an operator watching a scheduled run would see.
//!
//! Run with:
//!   cargo test -p ckan --test benchmark_e2e -- --ignored --nocapture
//!
//! Not run in CI by default (same convention as `benchmark_concurrent.rs`)
//! — this generates real synthetic archives and measures real elapsed
//! wall-clock time, which a shared CI machine's variable load makes
//! unsuitable as a pass/fail gate.
//!
//! # What this is NOT
//!
//! Not a comparison against V1 — no V1 benchmark exists; this run is V2's
//! first authoritative baseline, not a delta against anything. Not a
//! comparison against any distributed architecture — there is no
//! distributed architecture in V2. Future phases compare against *this*
//! baseline, not against anything from before it.

use std::io::Write as _;
use std::time::{Duration, Instant};

use ckan::ckan_client::CkanClient;
use ckan::paths::RawLayout;
use ckan::pipeline::ConcurrencyConfig;
use opentelemetry_sdk::trace::SpanData;
use ti_common::auth::TokenCredentials;

const REQUIRED_GTFS: &[&str] = &[
    "stops.txt",
    "trips.txt",
    "routes.txt",
    "stop_times.txt",
    "calendar_dates.txt",
];

// -- Fixed, reproducible workload (recorded in every run's own output) -----

/// Number of GTFS-S versions discovered and processed per iteration.
const WORKLOAD_VERSIONS: usize = 6;
/// Rows per GTFS member file per version — chosen to produce archives in the
/// tens-of-KiB range, large enough that Extract/Convert take measurable,
/// non-zero time without making the benchmark slow to run.
const ROWS_PER_FILE: usize = 50_000;
/// Independent repetitions of the whole invocation, to distinguish normal
/// variation from a real difference in a future comparison.
const REPETITIONS: usize = 5;

const CONCURRENCY: ConcurrencyConfig = ConcurrencyConfig {
    max_concurrent_versions: 4,
    max_queued_versions: 8,
    max_concurrent_downloads: 4,
    max_concurrent_processing: 4,
};

fn build_synthetic_zip_bytes(rows_per_file: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
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
    buf
}

/// One-shot raw-TCP fixture server — same pattern used throughout this
/// crate's tests (`tests/snapshot.rs`, `tests/observability.rs`): accepts
/// one connection, ignores the request, answers with `body` as a 200 OK.
/// Returns the server's base URL (`http://host:port`).
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
    format!("http://{addr}")
}

/// Starts one fixture download server per version and one fixture CKAN
/// `package_show` server listing all of them — the whole discoverable
/// workload for one benchmark iteration.
async fn serve_fixture_workload(zip_bytes: &[u8], versions: &[String]) -> String {
    let mut resources = Vec::with_capacity(versions.len());
    for version in versions {
        let base = serve_one_response(zip_bytes.to_vec()).await;
        resources.push(serde_json::json!({
            "url": format!("{base}/gtfs_fp2026_{version}.zip"),
            "format": "zip",
        }));
    }
    let package_show_body = serde_json::json!({
        "success": true,
        "result": { "resources": resources },
    })
    .to_string();
    serve_one_response(package_show_body.into_bytes()).await
}

struct IterationResult {
    wall_clock: Duration,
    bytes_downloaded: u64,
    succeeded: usize,
    failed: usize,
    stage_totals: StageTotals,
}

#[derive(Default)]
struct StageTotals {
    download: Duration,
    verify: Duration,
    extract: Duration,
    convert: Duration,
    publish: Duration,
}

fn sum_span_durations(spans: &[SpanData], name: &str) -> Duration {
    spans
        .iter()
        .filter(|s| s.name == name)
        .map(|s| {
            s.end_time
                .duration_since(s.start_time)
                .unwrap_or(Duration::ZERO)
        })
        .sum()
}

async fn run_one_iteration(iteration: usize) -> IterationResult {
    let zip_bytes = build_synthetic_zip_bytes(ROWS_PER_FILE);
    let versions: Vec<String> = (1..=WORKLOAD_VERSIONS)
        .map(|i| format!("2026{:02}{:02}", (iteration % 12) + 1, i))
        .collect();
    let api_url = serve_fixture_workload(&zip_bytes, &versions).await;

    let tmp = tempfile::tempdir().unwrap();
    let layout = RawLayout::new(tmp.path().to_path_buf());
    let http = reqwest::Client::new();
    let ckan_client = CkanClient::new(
        http.clone(),
        api_url,
        "gtfs-static".to_string(),
        TokenCredentials::new("bench-token", "bench-hash"),
    );

    let (otel, subscriber) = ti_common::observability::testing::init("ckan-benchmark");
    let _guard = tracing::subscriber::set_default(subscriber);

    let started = Instant::now();
    let summary = ckan::pipeline::run(&layout, &ckan_client, &http, None, CONCURRENCY)
        .await
        .expect("benchmark workload must run to completion");
    let wall_clock = started.elapsed();

    drop(_guard);
    otel.flush();
    let spans = otel.spans.get_finished_spans().unwrap_or_default();

    IterationResult {
        wall_clock,
        bytes_downloaded: summary.bytes_downloaded,
        succeeded: summary.succeeded,
        failed: summary.failed,
        stage_totals: StageTotals {
            download: sum_span_durations(&spans, "download"),
            verify: sum_span_durations(&spans, "verify"),
            extract: sum_span_durations(&spans, "extract"),
            convert: sum_span_durations(&spans, "convert"),
            publish: sum_span_durations(&spans, "publish"),
        },
    }
}

fn percentile(sorted_secs: &[f64], p: f64) -> f64 {
    if sorted_secs.is_empty() {
        return 0.0;
    }
    let rank = (p * (sorted_secs.len() - 1) as f64).round() as usize;
    sorted_secs[rank.min(sorted_secs.len() - 1)]
}

fn git_revision() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[test]
#[ignore = "wall-clock E2E benchmark; run with -- --ignored --nocapture"]
fn e2e_baseline() {
    // Current-thread, deliberately: `ti_common::observability::testing`'s
    // in-memory capture is scoped via a thread-local default subscriber
    // (`tracing::subscriber::set_default`), which only reaches a task
    // polled on this same OS thread. `pipeline::run`'s worker pool
    // `tokio::spawn`s one task per version; under a genuinely
    // multi-threaded runtime those tasks can be polled on a different
    // thread, where a `version`/`download`/etc. span would be created
    // against the ambient (no-op) default instead of ours, and silently
    // go uncaptured. A current-thread runtime has only one OS thread for
    // everything it runs, spawned tasks included, so this can't happen.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let zip_size_bytes = build_synthetic_zip_bytes(ROWS_PER_FILE).len();
    println!("\n=== GTFS-S downloader V2 — end-to-end baseline (Phase 8) ===");
    println!(
        "workload:      {WORKLOAD_VERSIONS} versions/run, {ROWS_PER_FILE} rows/file, ~{} KiB/archive",
        zip_size_bytes / 1024
    );
    println!("repetitions:   {REPETITIONS}");
    println!(
        "concurrency:   max_versions={} max_queued={} max_downloads={} max_processing={}",
        CONCURRENCY.max_concurrent_versions,
        CONCURRENCY.max_queued_versions,
        CONCURRENCY.max_concurrent_downloads,
        CONCURRENCY.max_concurrent_processing
    );
    println!(
        "environment:   {} {}, {} logical CPUs",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(0, std::num::NonZero::get)
    );
    println!("revision:      {}", git_revision());

    let mut wall_clocks = Vec::with_capacity(REPETITIONS);
    let mut totals = StageTotals::default();
    let mut bytes_total = 0u64;
    for i in 0..REPETITIONS {
        let result = rt.block_on(run_one_iteration(i));
        assert_eq!(
            result.failed, 0,
            "the fixed benchmark workload must always succeed — a failure here is a bug, not noise"
        );
        assert_eq!(result.succeeded, WORKLOAD_VERSIONS);
        println!(
            "  run {}: {:.3}s ({} bytes)",
            i + 1,
            result.wall_clock.as_secs_f64(),
            result.bytes_downloaded
        );
        wall_clocks.push(result.wall_clock.as_secs_f64());
        bytes_total += result.bytes_downloaded;
        totals.download += result.stage_totals.download;
        totals.verify += result.stage_totals.verify;
        totals.extract += result.stage_totals.extract;
        totals.convert += result.stage_totals.convert;
        totals.publish += result.stage_totals.publish;
    }

    wall_clocks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = percentile(&wall_clocks, 0.5);
    let p95 = percentile(&wall_clocks, 0.95);
    let min = wall_clocks.first().copied().unwrap_or(0.0);
    let max = wall_clocks.last().copied().unwrap_or(0.0);
    let aggregate_throughput_bps = bytes_total as f64 / wall_clocks.iter().sum::<f64>();

    println!("--- results (wall-clock, whole invocation) ---");
    println!("median:  {median:.3}s");
    println!("p95:     {p95:.3}s");
    println!("min:     {min:.3}s");
    println!("max:     {max:.3}s");
    println!(
        "aggregate throughput: {:.1} KiB/s ({} bytes total over {} runs)",
        aggregate_throughput_bps / 1024.0,
        bytes_total,
        REPETITIONS
    );
    println!("--- stage totals, summed across all runs and versions ---");
    println!("download: {:.3}s", totals.download.as_secs_f64());
    println!("verify:   {:.3}s", totals.verify.as_secs_f64());
    println!("extract:  {:.3}s", totals.extract.as_secs_f64());
    println!("convert:  {:.3}s", totals.convert.as_secs_f64());
    println!("publish:  {:.3}s", totals.publish.as_secs_f64());
}
