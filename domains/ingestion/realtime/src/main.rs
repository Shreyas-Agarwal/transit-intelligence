//! GTFS-RT realtime ingestion worker entrypoint.
//!
//! `realtime run` — the production pipeline: fetch → decode → publish to
//! Redpanda on a fixed poll interval (ADR 0007, ADR 0008).
//! `realtime explore` — a one-shot fetch + decode that logs a structured
//! summary of the feed's contents and writes it to
//! `feed-exploration-output.json`, for domain-mapping documentation.

use std::time::Instant;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde_json::json;

use realtime::config::RealtimeConfig;
use realtime::decoder::decode_feed_buffer;
use realtime::fetcher::fetch_feed_buffer;
use realtime::model::build_messages;
use realtime::producer::RedpandaProducer;
use realtime::topics::SNAPSHOTS_RAW;

#[derive(Parser)]
#[command(
    name = "realtime",
    about = "Fetches, decodes, and publishes GTFS-RT feed snapshots to Redpanda"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Poll the feed on a fixed interval and publish decoded entities to Redpanda (default).
    Run,
    /// Fetch and decode the feed once; log and write a summary of its contents.
    Explore,
}

#[tokio::main]
async fn main() -> Result<()> {
    ti_common::logging::init();

    let cli = Cli::parse();
    let cfg = RealtimeConfig::from_env()?;

    match cli.command.unwrap_or(Command::Run) {
        Command::Run => run(cfg).await,
        Command::Explore => explore(cfg).await,
    }
}

fn feed_timestamp_iso(header: &realtime::proto::FeedHeader) -> String {
    header
        .timestamp
        .and_then(|ts| chrono::DateTime::from_timestamp(ts as i64, 0))
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

async fn run(cfg: RealtimeConfig) -> Result<()> {
    tracing::info!(
        feed_url = %cfg.feed_url,
        poll_interval_ms = cfg.poll_interval.as_millis() as u64,
        brokers = ?cfg.redpanda_brokers,
        "GTFS-RT publisher starting"
    );

    let http = ti_common::http::build_client(cfg.feed_connect_timeout, cfg.feed_request_timeout)?;

    let mut producer =
        RedpandaProducer::connect(cfg.redpanda_brokers.clone(), cfg.client_id.clone()).await?;
    producer.ensure_topics().await?;

    let mut cycle: u64 = 0;
    loop {
        cycle += 1;
        tracing::info!(cycle, "poll cycle starting");

        if let Err(err) = poll_and_publish(&http, &cfg, &mut producer).await {
            // Continue polling on error — do not crash the loop.
            tracing::error!(cycle, error = %err, "poll cycle failed");
        }

        tokio::time::sleep(cfg.poll_interval).await;
    }
}

async fn poll_and_publish(
    http: &reqwest::Client,
    cfg: &RealtimeConfig,
    producer: &mut RedpandaProducer,
) -> Result<()> {
    let fetch_start = Instant::now();
    let buffer = fetch_feed_buffer(http, &cfg.feed_url, &cfg.feed_api_token).await?;
    let fetch_ms = fetch_start.elapsed().as_millis();

    let decode_start = Instant::now();
    let feed = decode_feed_buffer(&buffer)?;
    let decode_ms = decode_start.elapsed().as_millis();

    let feed_timestamp_iso = feed_timestamp_iso(&feed.header);
    let ingestion_timestamp = Utc::now().to_rfc3339();
    let messages = build_messages(
        &feed.entity,
        &feed_timestamp_iso,
        &feed.header.gtfs_realtime_version,
        &ingestion_timestamp,
    );

    let vehicle_count = feed.entity.iter().filter(|e| e.vehicle.is_some()).count();
    let trip_update_count = feed
        .entity
        .iter()
        .filter(|e| e.trip_update.is_some())
        .count();
    let alert_count = feed.entity.iter().filter(|e| e.alert.is_some()).count();

    tracing::info!(
        payload_bytes = buffer.len(),
        entity_count = feed.entity.len(),
        vehicle_positions = vehicle_count,
        trip_updates = trip_update_count,
        alerts = alert_count,
        message_count = messages.len(),
        fetch_ms,
        decode_ms,
        "message batch built"
    );

    let publish_start = Instant::now();
    let published = messages.len();
    producer.publish(SNAPSHOTS_RAW, messages).await?;
    let publish_ms = publish_start.elapsed().as_millis();

    tracing::info!(
        published_messages = published,
        fetch_ms,
        decode_ms,
        publish_ms,
        topic = SNAPSHOTS_RAW,
        "poll cycle complete"
    );

    Ok(())
}

async fn explore(cfg: RealtimeConfig) -> Result<()> {
    tracing::info!(feed_url = %cfg.feed_url, "starting GTFS-RT feed exploration");

    let http = ti_common::http::build_client(cfg.feed_connect_timeout, cfg.feed_request_timeout)?;

    let fetch_start = Instant::now();
    let buffer = fetch_feed_buffer(&http, &cfg.feed_url, &cfg.feed_api_token).await?;
    let fetch_ms = fetch_start.elapsed().as_millis();
    tracing::info!(bytes = buffer.len(), fetch_latency_ms = fetch_ms, "feed fetched");

    let decode_start = Instant::now();
    let feed = decode_feed_buffer(&buffer)?;
    let decode_ms = decode_start.elapsed().as_millis();
    tracing::info!(decode_latency_ms = decode_ms, "feed decoded");

    let feed_timestamp_iso = feed_timestamp_iso(&feed.header);
    tracing::info!(
        gtfs_realtime_version = %feed.header.gtfs_realtime_version,
        timestamp_utc = %feed_timestamp_iso,
        timestamp_posix = feed.header.timestamp,
        "feed header"
    );

    let vehicle_count = feed.entity.iter().filter(|e| e.vehicle.is_some()).count();
    let trip_update_count = feed
        .entity
        .iter()
        .filter(|e| e.trip_update.is_some())
        .count();
    let alert_count = feed.entity.iter().filter(|e| e.alert.is_some()).count();
    let unknown_count = feed.entity.len() - vehicle_count - trip_update_count - alert_count;

    tracing::info!(
        total = feed.entity.len(),
        vehicle_positions = vehicle_count,
        trip_updates = trip_update_count,
        alerts = alert_count,
        unknown = unknown_count,
        "entity counts"
    );

    let output = json!({
        "explored_at": Utc::now().to_rfc3339(),
        "feed_url": cfg.feed_url,
        "fetch_latency_ms": fetch_ms,
        "decode_latency_ms": decode_ms,
        "payload_bytes": buffer.len(),
        "header": {
            "gtfs_realtime_version": feed.header.gtfs_realtime_version,
            "timestamp_posix": feed.header.timestamp,
            "timestamp_utc": feed_timestamp_iso,
        },
        "entity_counts": {
            "total": feed.entity.len(),
            "vehicle_positions": vehicle_count,
            "trip_updates": trip_update_count,
            "alerts": alert_count,
            "unknown": unknown_count,
        },
    });

    let output_path = "feed-exploration-output.json";
    std::fs::write(output_path, serde_json::to_string_pretty(&output)?)
        .with_context(|| format!("failed to write {output_path}"))?;
    tracing::info!(path = output_path, "exploration output written");

    Ok(())
}
