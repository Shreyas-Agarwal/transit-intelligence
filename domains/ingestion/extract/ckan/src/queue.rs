//! Bounded in-process work queue (implementation plan Phase 3).
//!
//! Replaces the direct-spawn pattern `pipeline::run` uses today (one
//! `Arc<Semaphore>` gating a `JoinSet::spawn` call per pending version) with
//! an explicit producer/consumer split:
//!
//! ```text
//! producer (reconciler's eligible work)
//!     |
//!     v
//! bounded channel     capacity = MAX_QUEUED_VERSIONS
//!     |
//!     v
//! fixed worker pool   size     = MAX_ACTIVE_VERSIONS
//! ```
//!
//! The two bounds are independent knobs. `max_queued` caps how many items
//! may sit waiting for a worker before the producer blocks; `max_active`
//! caps how many versions are processed concurrently, via a *fixed* pool of
//! long-lived worker tasks spawned once up front, rather than one task per
//! item. Neither bound grows with load: there is no code path by which
//! enqueuing more work spawns more tasks. If the queue is already full, the
//! producer's `enqueue` call simply waits.
//!
//! This module is deliberately generic in the item and result types so it
//! can be exercised here (see `tests/queue.rs`) with a cheap stub worker,
//! independent of the real download/extract/convert pipeline. Wiring the
//! real `pipeline::process_version` stages through it is Phase 4's job.
//!
//! Per implementation-plan direction, the production item type will be
//! `VersionId` alone — never the full `UpstreamResource` — so the queue
//! never carries resource metadata (download URL, publisher hash, etc.)
//! through it. A worker that needs that metadata looks it up from the
//! discovery result already held in its own execution context (e.g. a
//! `HashMap<VersionId, UpstreamResource>` captured by the `process`
//! closure), rather than re-querying CKAN.
//!
//! This queue is content-blind: it does not deduplicate by item identity.
//! Each individually enqueued message is guaranteed to be delivered to
//! exactly one worker (never split or duplicated across the pool), but two
//! *separate* enqueue calls carrying the same logical `VersionId` are free
//! to run concurrently as far as this module is concerned. Avoiding that in
//! practice is a producer-discipline property — Phase 2's `reconcile`
//! offers each distinct eligible version at most once per pass, and a new
//! pass isn't started until the previous one's queue has drained — and,
//! later, the Phase 4 Claim step's job (recording ownership in durable work
//! state), not something this generic queue enforces itself.

use std::future::Future;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinSet;

#[derive(Debug, Clone, Copy)]
pub struct QueueConfig {
    /// Channel capacity: how many items may be buffered waiting for a
    /// worker before `enqueue` blocks. Coerced to at least 1.
    pub max_queued: usize,
    /// Number of long-lived worker tasks in the fixed pool. Coerced to at
    /// least 1.
    pub max_active: usize,
}

/// Every worker has exited (e.g. a prior panic took the whole pool down);
/// nothing remains to drain this item, so it will never be processed.
#[derive(Debug, thiserror::Error)]
#[error("queue is closed: every worker has already exited")]
pub struct QueueClosed;

/// The producer-side handle. Enqueuing blocks once `max_queued` items are
/// already buffered — the backpressure point required by the plan: the
/// producer never spawns extra tasks to work around a full queue, it waits.
pub struct WorkQueue<T> {
    tx: mpsc::Sender<T>,
}

impl<T> WorkQueue<T> {
    /// Enqueues `item`, blocking while the queue is at `max_queued`
    /// capacity and every worker is busy.
    pub async fn enqueue(&self, item: T) -> Result<(), QueueClosed> {
        self.tx.send(item).await.map_err(|_| QueueClosed)
    }

    /// Signals that no further work is coming. Already-buffered and
    /// in-flight items are still processed to completion — this is a drain,
    /// not a cancellation. Equivalent to dropping the handle; spelled out as
    /// a named method for readability at call sites.
    pub fn close(self) {
        drop(self);
    }
}

/// Spawns `config.max_active` worker tasks sharing one bounded channel of
/// capacity `config.max_queued`, each invoking `process` for one item at a
/// time. Returns the producer handle, a channel of `(item, result)` pairs
/// (delivered in completion order, not enqueue order), and the `JoinSet` of
/// worker tasks — join it to propagate a worker panic and to know every
/// worker has exited.
///
/// A worker calls `process`, forwards the `(item, result)` pair, and loops
/// back for the next item; it exits only once the shared receiver reports
/// the channel is both closed (every [`WorkQueue`] handle dropped) *and*
/// drained. That is this queue's graceful-shutdown story: closing the
/// producer side lets every already-queued and in-flight item finish before
/// any worker exits — nothing is aborted mid-item, and no enqueued item is
/// ever lost.
pub fn spawn<T, F, Fut, R>(
    config: QueueConfig,
    process: F,
) -> (WorkQueue<T>, mpsc::Receiver<(T, R)>, JoinSet<()>)
where
    T: Clone + Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let max_queued = config.max_queued.max(1);
    let max_active = config.max_active.max(1);

    let (tx, rx) = mpsc::channel::<T>(max_queued);
    let (out_tx, out_rx) = mpsc::channel::<(T, R)>(max_queued.max(max_active));
    let rx = Arc::new(Mutex::new(rx));
    let process = Arc::new(process);

    let mut workers = JoinSet::new();
    for _ in 0..max_active {
        let rx = Arc::clone(&rx);
        let process = Arc::clone(&process);
        let out_tx = out_tx.clone();
        workers.spawn(async move {
            loop {
                // The lock is held only long enough to pull one item off the
                // shared receiver; processing happens outside it, so at most
                // one worker is ever waiting on `recv` at a time but many
                // can process concurrently.
                let next = { rx.lock().await.recv().await };
                let Some(item) = next else {
                    break; // channel closed and drained: no more work, ever.
                };
                let result = process(item.clone()).await;
                if out_tx.send((item, result)).await.is_err() {
                    break; // every output receiver dropped; nothing to do.
                }
            }
        });
    }
    // Drop the template sender: each worker holds its own clone, so the
    // output channel only closes once every worker has actually exited.
    drop(out_tx);

    (WorkQueue { tx }, out_rx, workers)
}
