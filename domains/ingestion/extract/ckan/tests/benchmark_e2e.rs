//! End-to-end V2 performance baseline (implementation plan Phase 8).
//!
//! Exercises the real `ckan::pipeline::run` entrypoint — discovery through
//! publish, through the real bounded queue and worker pool, with real
//! archive extraction and Parquet conversion — against fixed, reproducible
//! local workloads: a fixture CKAN `package_show` response plus one fixture
//! download server per version, each serving a synthetic but
//! size-representative GTFS archive. Nothing here talks to the live CKAN API
//! or the live opentransportdata.swiss servers, and no other test in this
//! crate actually drives `pipeline::run` itself end to end (existing tests
//! replay its per-version pipeline directly, bypassing discovery) — this is
//! the first one that does.
//!
//! This is deliberately end-to-end, not a microbenchmark of any one stage:
//! the number that matters is what one real invocation actually takes, the
//! same shape of number an operator watching a scheduled run would see.
//!
//! # Two fixed workloads, not one
//!
//! [`REPRESENTATIVE`] approximates a normal catch-up run: a handful of
//! real-sized GTFS-S archives, at the default worker-pool concurrency.
//! [`SATURATION`] approximates a large backlog: enough versions to fill the
//! bounded queue to capacity *and* keep every worker busy at once, so the
//! producer is guaranteed to block at least once — the one thing
//! `REPRESENTATIVE`'s small version count never exercises. Both use the same
//! per-archive size on purpose, so version *count* is the only thing that
//! differs between them; conflating "bigger archives" and "more of them" in
//! one workload would make it impossible to tell which one caused a change
//! in a future comparison.
//!
//! Archive size is anchored to `docs/design/gtfs-static-auto-downloader.md`
//! §"Benchmark", which documents real GTFS-S archives from
//! opentransportdata.swiss as ~150-300 MB each — not to the much smaller
//! (~3 MB) synthetic fixture size this crate's tests and
//! `benchmark_concurrent.rs` use elsewhere for speed. An earlier version of
//! this benchmark used that same ~3 MB fixture size, which is fine for
//! correctness tests but understates real download/staging cost by roughly
//! two orders of magnitude — not a bug in the numbers it reported, but a
//! workload that was never representative of production traffic in the
//! first place. `REPRESENTATIVE` and `SATURATION` both target the low end of
//! the documented range (~150 MB), a deliberate reproducibility/runtime
//! trade-off recorded in the implementation log rather than assumed.
//!
//! Run with:
//!   cargo test -p ckan --test benchmark_e2e -- --ignored --nocapture
//!
//! Not run in CI by default (same convention as `benchmark_concurrent.rs`)
//! — this generates real, sizable synthetic archives and measures real
//! elapsed wall-clock time, which a shared CI machine's variable load makes
//! unsuitable as a pass/fail gate. `SATURATION` in particular is slow by
//! design (see below) and is meant to be run deliberately, not routinely.
//!
//! # What this is NOT
//!
//! Not a comparison against V1 — no V1 benchmark exists; these runs are
//! V2's first authoritative baseline, not a delta against anything. Not a
//! comparison against any distributed architecture — there is no
//! distributed architecture in V2. Future phases (notably Phase 11's tuning)
//! compare against *these* baselines, not against anything from before them.

use std::io::Write as _;
use std::path::Path;
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

const DEFAULT_CONCURRENCY: ConcurrencyConfig = ConcurrencyConfig {
    max_concurrent_versions: 4,
    max_queued_versions: 8,
    max_concurrent_downloads: 4,
    max_concurrent_processing: 4,
};

/// A frozen, named benchmark workload — see the module doc comment for what
/// distinguishes `REPRESENTATIVE` from `SATURATION` and why.
struct Workload {
    name: &'static str,
    /// GTFS-S versions discovered and processed per invocation.
    versions: usize,
    /// Rows per GTFS member file per version. Tuned empirically (see the
    /// implementation log) so the resulting *compressed* archive lands near
    /// this workload's documented real-world target — not chosen for a
    /// round row count.
    rows_per_file: usize,
    /// Independent repetitions of the whole invocation.
    repetitions: usize,
    concurrency: ConcurrencyConfig,
}

/// A normal catch-up run: a handful of real-sized archives, at the default
/// worker-pool concurrency (`max_concurrent_versions` below matches
/// `ckan::config`'s own default of `min(4, available_parallelism)`).
const REPRESENTATIVE: Workload = Workload {
    name: "representative",
    versions: 4,
    rows_per_file: 2_600_000,
    repetitions: 3,
    concurrency: DEFAULT_CONCURRENCY,
};

/// A backlog large enough to fill the bounded queue to capacity while every
/// worker is simultaneously busy: `max_queued_versions + max_concurrent_versions`
/// = 8 + 4 = 12. This is the smallest version count that guarantees the
/// producer actually blocks on `enqueue` at least once — `REPRESENTATIVE`'s
/// 4 versions never fill an 8-slot queue, so it never exercises backpressure
/// at all. Same per-archive size as `REPRESENTATIVE`, deliberately (see the
/// module doc comment) — only one repetition, because this workload's
/// purpose is observing behavior under load, not gathering a percentile
/// distribution over a run that already takes several minutes once.
const SATURATION: Workload = Workload {
    name: "saturation",
    versions: 12,
    rows_per_file: 2_600_000,
    repetitions: 1,
    concurrency: DEFAULT_CONCURRENCY,
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

async fn run_one_iteration(
    workload: &Workload,
    iteration: usize,
    zip_bytes: &[u8],
) -> IterationResult {
    let versions: Vec<String> = (1..=workload.versions)
        .map(|i| format!("2026{:02}{:02}", (iteration % 12) + 1, i))
        .collect();
    let api_url = serve_fixture_workload(zip_bytes, &versions).await;

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
    let summary = ckan::pipeline::run(&layout, &ckan_client, &http, None, workload.concurrency)
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

// -- Environment record (implementation plan Phase 8, review round 1) ------
//
// Practical, not a laboratory benchmark: this captures what machine/OS/disk
// a run happened on, in enough detail to explain a gross difference between
// two runs (different CPU, different filesystem, a throttled governor) —
// not enough to control for every micro-variable a dedicated benchmarking
// rig would. Instantaneous CPU frequency is deliberately not captured: it
// changes constantly under normal turbo/thermal behavior and recording one
// sample at startup would imply a precision this benchmark doesn't have and
// doesn't need.

fn read_proc_field(path: &str, field: &str) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    contents.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == field).then(|| value.trim().to_string())
    })
}

fn read_cpu_governor() -> String {
    std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown (not exposed by this kernel/container)".to_string())
}

fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn read_filesystem_type(path: &Path) -> String {
    run_command("df", &["-T", &path.to_string_lossy()])
        .and_then(|text| {
            text.lines()
                .nth(1)
                .and_then(|line| line.split_whitespace().nth(1).map(str::to_string))
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn describe_environment(workload_root: &Path) -> String {
    let cpu_model =
        read_proc_field("/proc/cpuinfo", "model name").unwrap_or_else(|| "unknown".to_string());
    let cores_note = match (
        read_proc_field("/proc/cpuinfo", "cpu cores"),
        read_proc_field("/proc/cpuinfo", "siblings"),
    ) {
        (Some(cores), Some(threads)) => {
            format!(", {cores} physical cores / {threads} threads per socket")
        }
        _ => String::new(),
    };
    let logical_cpus = std::thread::available_parallelism().map_or(0, std::num::NonZero::get);
    let ram = read_proc_field("/proc/meminfo", "MemTotal")
        .and_then(|s| s.trim_end_matches(" kB").parse::<u64>().ok())
        .map_or("unknown".to_string(), |kib| {
            format!("{:.1} GiB", kib as f64 / 1024.0 / 1024.0)
        });
    let kernel = run_command("uname", &["-r"]).unwrap_or_else(|| "unknown".to_string());
    let fs_type = read_filesystem_type(workload_root);
    let governor = read_cpu_governor();

    format!(
        "{} {}, kernel {kernel}\n  \
         CPU:          {cpu_model} ({logical_cpus} logical CPUs{cores_note})\n  \
         RAM:          {ram}\n  \
         storage:      {fs_type} filesystem (at benchmark tempdir)\n  \
         CPU governor: {governor} (sampled once at startup, not tracked per-iteration)",
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

fn run_workload(workload: &Workload) {
    let zip_bytes = build_synthetic_zip_bytes(workload.rows_per_file);
    let zip_kib = zip_bytes.len() / 1024;

    println!(
        "\n=== GTFS-S downloader V2 — {} workload (Phase 8) ===",
        workload.name
    );
    println!(
        "workload:      {} versions/run, {} rows/file, ~{} KiB/archive ({:.1} MiB)",
        workload.versions,
        workload.rows_per_file,
        zip_kib,
        zip_kib as f64 / 1024.0
    );
    println!("repetitions:   {}", workload.repetitions);
    println!(
        "concurrency:   max_versions={} max_queued={} max_downloads={} max_processing={}",
        workload.concurrency.max_concurrent_versions,
        workload.concurrency.max_queued_versions,
        workload.concurrency.max_concurrent_downloads,
        workload.concurrency.max_concurrent_processing
    );
    let tmp = tempfile::tempdir().unwrap();
    println!("environment:   {}", describe_environment(tmp.path()));
    println!("revision:      {}", git_revision());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let mut wall_clocks = Vec::with_capacity(workload.repetitions);
    let mut totals = StageTotals::default();
    let mut bytes_total = 0u64;
    for i in 0..workload.repetitions {
        let result = rt.block_on(run_one_iteration(workload, i, &zip_bytes));
        assert_eq!(
            result.failed, 0,
            "the fixed benchmark workload must always succeed — a failure here is a bug, not noise"
        );
        assert_eq!(result.succeeded, workload.versions);
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
    let total_secs: f64 = wall_clocks.iter().sum();
    let aggregate_throughput_bps = if total_secs > 0.0 {
        bytes_total as f64 / total_secs
    } else {
        0.0
    };

    println!("--- results (wall-clock, whole invocation) ---");
    println!("median:  {median:.3}s");
    println!("p95:     {p95:.3}s");
    println!("min:     {min:.3}s");
    println!("max:     {max:.3}s");
    println!(
        "aggregate throughput: {:.1} MiB/s ({} bytes total over {} run(s))",
        aggregate_throughput_bps / 1024.0 / 1024.0,
        bytes_total,
        workload.repetitions
    );
    println!(
        "--- stage totals, summed across all runs and versions ({} version-runs) ---",
        workload.repetitions * workload.versions
    );
    println!("download: {:.3}s", totals.download.as_secs_f64());
    println!("verify:   {:.3}s", totals.verify.as_secs_f64());
    println!("extract:  {:.3}s", totals.extract.as_secs_f64());
    println!("convert:  {:.3}s", totals.convert.as_secs_f64());
    println!("publish:  {:.3}s", totals.publish.as_secs_f64());
}

#[test]
#[ignore = "wall-clock E2E benchmark; run with -- --ignored --nocapture representative_workload_baseline"]
fn representative_workload_baseline() {
    run_workload(&REPRESENTATIVE);
}

#[test]
#[ignore = "wall-clock E2E benchmark; slow by design (12 real-sized archives) — run with -- --ignored --nocapture saturation_workload_baseline"]
fn saturation_workload_baseline() {
    run_workload(&SATURATION);
}
