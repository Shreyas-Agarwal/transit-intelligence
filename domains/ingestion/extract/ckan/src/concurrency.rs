//! Resource-specific concurrency (implementation plan Phase 5).
//!
//! Before this phase, one limit — how many versions may be active at once
//! (the Phase 3/4 worker-pool size, `MAX_CONCURRENT_VERSIONS`) — was the only
//! thing bounding concurrency anywhere in the pipeline. A version occupying a
//! worker slot could be doing any of its stages; nothing distinguished "4
//! versions downloading at once" from "4 versions running CPU-heavy Parquet
//! conversion at once," even though those put very different load on the
//! network versus the CPU/disk.
//!
//! This module adds two more limits, orthogonal to the worker-pool size:
//!
//! ```text
//! Version worker (bounded by MAX_CONCURRENT_VERSIONS, unchanged)
//!     |
//!     +-- network permit      -> Download
//!     +-- CPU/disk permit     -> Extract
//!     +-- CPU/disk permit     -> Convert
//! ```
//!
//! Download draws from one pool (`MAX_CONCURRENT_DOWNLOADS`); Extract and
//! Convert both draw from a second, shared pool (`MAX_CONCURRENT_PROCESSING`)
//! — the plan's diagram shows two arrows into one CPU/disk pool, not two
//! separate ones, since both stages put the same kind of load (CPU + disk,
//! not network) on the host. There is still only one queue and one worker
//! pool (Phase 3/4); this only adds finer-grained limits *inside* what a
//! worker does with its slot, not new queues.
//!
//! No default value here is meant to be a measured, tuned number — see the
//! plan's own caution against over-specifying tuning values before Phase 11
//! actually measures anything. `ckan::config` defaults both new settings to
//! whatever `MAX_CONCURRENT_VERSIONS` already is, so an operator who never
//! touches the two new environment variables sees no behavior change at all.
//!
//! Phase 7 adds one more thing: each pool reports how many permits are
//! currently in use as an OpenTelemetry gauge-like counter, so "how much of
//! the configured download/processing concurrency is actually being used"
//! is answered by a metric, not just by reading the configured limit. This
//! module records its own utilization — that's what it manages — but knows
//! nothing about GTFS, spans, or why the permits exist; see `ckan::telemetry`
//! for the pipeline-level metrics built on top.

use std::future::Future;
use std::sync::Arc;

use opentelemetry::metrics::UpDownCounter;
use tokio::sync::Semaphore;

/// Two independent resource pools, shared by every worker in a run. Cheap to
/// clone — each field is reference-counted — so every worker gets its own
/// handle to the same two pools rather than the pools being recreated per
/// worker or per version.
#[derive(Clone)]
pub struct ResourcePermits {
    download: Arc<Semaphore>,
    processing: Arc<Semaphore>,
    download_in_use: UpDownCounter<i64>,
    processing_in_use: UpDownCounter<i64>,
}

/// Decrements `counter` when dropped — released on every exit path (normal
/// return, early error return, or the holding task being cancelled) the same
/// way the semaphore permit itself is, since both are just values going out
/// of scope.
struct InUseGuard<'a> {
    counter: &'a UpDownCounter<i64>,
}

impl<'a> InUseGuard<'a> {
    fn enter(counter: &'a UpDownCounter<i64>) -> Self {
        counter.add(1, &[]);
        Self { counter }
    }
}

impl Drop for InUseGuard<'_> {
    fn drop(&mut self) {
        self.counter.add(-1, &[]);
    }
}

impl ResourcePermits {
    pub fn new(max_concurrent_downloads: usize, max_concurrent_processing: usize) -> Self {
        let meter = opentelemetry::global::meter("ckan");
        Self {
            download: Arc::new(Semaphore::new(max_concurrent_downloads.max(1))),
            processing: Arc::new(Semaphore::new(max_concurrent_processing.max(1))),
            download_in_use: meter
                .i64_up_down_counter("gtfs_s.concurrency.download_permits_in_use")
                .with_description("downloads currently in flight, out of the configured limit")
                .build(),
            processing_in_use: meter
                .i64_up_down_counter("gtfs_s.concurrency.processing_permits_in_use")
                .with_description(
                    "extract/convert stages currently running, out of the configured limit",
                )
                .build(),
        }
    }

    pub fn available_download_permits(&self) -> usize {
        self.download.available_permits()
    }

    pub fn available_processing_permits(&self) -> usize {
        self.processing.available_permits()
    }

    /// Runs `f` while holding one network permit. The permit is acquired
    /// immediately before `f`'s future starts and released the instant that
    /// future stops running — whether it finished successfully, finished
    /// with an error, or was itself dropped mid-flight because the caller
    /// (e.g. the surrounding worker task) was cancelled. All three cases are
    /// the same code path here: a value going out of scope releases what it
    /// holds, regardless of why the scope ended.
    pub async fn with_download_permit<F, Fut, T>(&self, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let _permit = self
            .download
            .acquire()
            .await
            .expect("download permit pool is never closed");
        let _in_use = InUseGuard::enter(&self.download_in_use);
        f().await
    }

    /// Same guarantee as [`Self::with_download_permit`], against the
    /// CPU/disk (processing) pool instead — used around both Extract and
    /// Convert.
    pub async fn with_processing_permit<F, Fut, T>(&self, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let _permit = self
            .processing
            .acquire()
            .await
            .expect("processing permit pool is never closed");
        let _in_use = InUseGuard::enter(&self.processing_in_use);
        f().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// The download pool caps concurrent network work at exactly its
    /// configured size, regardless of how many callers are waiting.
    #[tokio::test]
    async fn download_permits_cap_concurrency_independently() {
        let permits = ResourcePermits::new(2, 100);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..6 {
            let permits = permits.clone();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                permits
                    .with_download_permit(|| async {
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(
            peak.load(Ordering::SeqCst),
            2,
            "must reach, but never exceed, the download cap"
        );
    }

    /// Same proof, against the processing pool, with its own independent size.
    #[tokio::test]
    async fn processing_permits_cap_concurrency_independently() {
        let permits = ResourcePermits::new(100, 3);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let permits = permits.clone();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                permits
                    .with_processing_permit(|| async {
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 3);
    }

    /// A version doing CPU/disk-heavy work must not block a *different*
    /// version's download — the whole point of separating the two pools.
    #[tokio::test]
    async fn a_saturated_processing_pool_does_not_block_a_concurrent_download() {
        let permits = ResourcePermits::new(1, 1);

        let permits_a = permits.clone();
        let processing_task = tokio::spawn(async move {
            permits_a
                .with_processing_permit(|| async {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                })
                .await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await; // let it acquire first

        let start = std::time::Instant::now();
        permits
            .with_download_permit(|| async { "downloaded" })
            .await;
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "a download must not wait on a saturated, unrelated processing pool"
        );

        processing_task.await.unwrap();
    }

    #[tokio::test]
    async fn permit_is_released_after_a_successful_call() {
        let permits = ResourcePermits::new(1, 1);
        permits.with_download_permit(|| async { 42 }).await;
        assert_eq!(permits.available_download_permits(), 1);
    }

    #[tokio::test]
    async fn permit_is_released_after_a_failing_call() {
        let permits = ResourcePermits::new(1, 1);
        let _: Result<(), &str> = permits
            .with_processing_permit(|| async { Err("boom") })
            .await;
        assert_eq!(permits.available_processing_permits(), 1);
    }

    /// If the task holding a permit is cancelled outright (aborted, not just
    /// erroring), the permit still comes back — nothing about resource
    /// cleanup here depends on the holder finishing normally.
    #[tokio::test]
    async fn permit_is_released_if_the_holding_task_is_cancelled() {
        let permits = ResourcePermits::new(1, 1);
        let permits_task = permits.clone();

        let handle = tokio::spawn(async move {
            permits_task
                .with_download_permit(|| async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                })
                .await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await; // let it acquire
        assert_eq!(
            permits.available_download_permits(),
            0,
            "the permit must be held while the task is mid-flight"
        );

        handle.abort();
        let _ = handle.await; // resolves once the task's future is actually dropped

        assert_eq!(
            permits.available_download_permits(),
            1,
            "aborting the task must release the permit it was holding"
        );
    }
}
