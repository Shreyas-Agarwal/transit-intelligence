//! Entrypoint. Intended to be invoked once per scheduled run (design doc §10
//! leaves the scheduling mechanism — cron, GitHub Actions, etc. — outside this
//! binary's scope). Each invocation performs one full recovery + discover +
//! download + publish pass and exits.

use clap::Parser;

use ckan::config::CkanConfig;
use ckan::paths::RawLayout;

#[derive(Parser)]
#[command(
    name = "ckan",
    about = "Detects and downloads new GTFS-S snapshots from opentransportdata.swiss's CKAN catalog"
)]
struct Cli {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();

    ti_common::logging::init();

    let cfg = CkanConfig::from_env()?;
    let layout = RawLayout::new(cfg.raw_dir.clone());

    let api_http = ti_common::http::build_client(cfg.api_connect_timeout, cfg.api_request_timeout)?;
    let download_http =
        ti_common::http::build_client(cfg.download_connect_timeout, cfg.download_request_timeout)?;

    let ckan_client = ckan::ckan_client::CkanClient::new(
        api_http,
        cfg.ckan_api_url.clone(),
        cfg.dataset_id.clone(),
        cfg.credentials.clone(),
    );

    let summary = ckan::pipeline::run(
        &layout,
        &ckan_client,
        &download_http,
        cfg.cutoff_version.as_ref(),
        cfg.max_concurrent_versions,
        cfg.max_queued_versions,
    )
    .await
    .inspect_err(|e| tracing::error!(error = %e, "updater run failed"))?;

    // The run itself succeeded (recovery, discovery, and manifest bookkeeping
    // all completed) even if individual versions failed — those are recorded
    // and retried next run, not fatal to the process. A non-zero exit here
    // just makes CI/cron notice a partial failure instead of it only showing
    // up in the summary log.
    anyhow::ensure!(
        summary.failed == 0,
        "{} version(s) failed this run; see log above",
        summary.failed
    );

    Ok(())
}
