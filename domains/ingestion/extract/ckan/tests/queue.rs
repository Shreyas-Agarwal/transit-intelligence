//! Integration tests for the bounded in-process work queue (implementation
//! plan Phase 3): capacity, backpressure, worker consumption, draining,
//! graceful shutdown, no lost work, and no duplicate active work.
//!
//! These exercise `ckan::queue` directly with cheap stub workers — proving
//! the queue mechanism itself is correct, independent of the real
//! download/extract/convert pipeline (that wiring is Phase 4).

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ckan::queue::{self, QueueConfig};
use tokio::sync::{Mutex, Notify};

/// Drains every item off `rx` until the channel closes (all workers exited).
async fn drain_all<T, R>(mut rx: tokio::sync::mpsc::Receiver<(T, R)>) -> Vec<(T, R)> {
    let mut out = Vec::new();
    while let Some(pair) = rx.recv().await {
        out.push(pair);
    }
    out
}

// -- queue capacity / producer backpressure -------------------------------

/// With `max_active` workers and `max_queued` buffer slots, exactly
/// `max_active + max_queued` items can be accepted without any worker ever
/// completing one. The next `enqueue` call must not resolve until a worker
/// frees a slot — the producer blocks rather than the queue growing further.
#[tokio::test]
async fn enqueue_blocks_once_queue_and_workers_are_saturated() {
    let gate = Arc::new(Notify::new());
    let gate_worker = Arc::clone(&gate);

    let (tx, _rx, _workers) = queue::spawn(
        QueueConfig {
            max_queued: 2,
            max_active: 2,
        },
        move |item: u32| {
            let gate = Arc::clone(&gate_worker);
            async move {
                gate.notified().await;
                item
            }
        },
    );

    // 2 workers claim items 0 and 1 immediately and block on the gate.
    tx.enqueue(0).await.unwrap();
    tx.enqueue(1).await.unwrap();
    // 2 more items fill the buffer (max_queued = 2) without blocking.
    tx.enqueue(2).await.unwrap();
    tx.enqueue(3).await.unwrap();

    // A 5th item has nowhere to go: both workers are busy, the buffer is full.
    let saturated = tokio::time::timeout(Duration::from_millis(100), tx.enqueue(4)).await;
    assert!(
        saturated.is_err(),
        "enqueue must block once max_active + max_queued items are already accepted"
    );

    // Release the gate: workers 0 and 1 finish, pull 2 and 3 off the buffer,
    // freeing room for the previously-blocked enqueue to finally land.
    gate.notify_waiters();
    tokio::time::timeout(Duration::from_secs(1), tx.enqueue(4))
        .await
        .expect("enqueue must unblock once a worker frees a slot")
        .unwrap();
}

/// The blocked producer isn't deadlocked — once capacity frees up, the
/// pending `enqueue` call completes and the item is genuinely accepted (not
/// dropped or silently ignored).
#[tokio::test]
async fn blocked_producer_unblocks_and_the_item_is_still_processed() {
    let gate = Arc::new(Notify::new());
    let gate_worker = Arc::clone(&gate);

    let (tx, rx, _workers) = queue::spawn(
        QueueConfig {
            max_queued: 1,
            max_active: 1,
        },
        move |item: u32| {
            let gate = Arc::clone(&gate_worker);
            async move {
                if item == 0 {
                    gate.notified().await;
                }
                item
            }
        },
    );

    tx.enqueue(0).await.unwrap(); // claimed by the single worker, blocks on gate
    tx.enqueue(1).await.unwrap(); // fills the one buffer slot

    let tx_clone_task = {
        let tx = tx;
        tokio::spawn(async move {
            tx.enqueue(2).await.unwrap();
            tx // give the handle back so the caller can close it
        })
    };

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !tx_clone_task.is_finished(),
        "enqueue(2) must still be blocked while the queue is saturated"
    );

    gate.notify_waiters();
    let tx = tokio::time::timeout(Duration::from_secs(1), tx_clone_task)
        .await
        .expect("enqueue(2) must unblock")
        .unwrap();
    tx.close();

    let results = drain_all(rx).await;
    let mut items: Vec<u32> = results.into_iter().map(|(item, _)| item).collect();
    items.sort_unstable();
    assert_eq!(
        items,
        vec![0, 1, 2],
        "every enqueued item must be processed"
    );
}

// -- worker consumption ----------------------------------------------------

/// Items are actually picked up and run by the worker pool, and the number
/// of concurrently-processing items never exceeds `max_active` — proving the
/// fixed pool bounds *active* work independently of how much is queued.
#[tokio::test]
async fn worker_pool_processes_concurrently_without_exceeding_max_active() {
    const MAX_ACTIVE: usize = 3;
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    let (tx, rx, workers) = {
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        queue::spawn(
            QueueConfig {
                max_queued: 10,
                max_active: MAX_ACTIVE,
            },
            move |item: u32| {
                let active = Arc::clone(&active);
                let peak = Arc::clone(&peak);
                async move {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    item
                }
            },
        )
    };

    for i in 0..9u32 {
        tx.enqueue(i).await.unwrap();
    }
    tx.close();

    let mut workers = workers;
    let results = drain_all(rx).await;
    while workers.join_next().await.is_some() {}

    assert_eq!(results.len(), 9, "every enqueued item must be processed");
    assert_eq!(
        peak.load(Ordering::SeqCst),
        MAX_ACTIVE,
        "the pool must actually reach max_active concurrency, not less"
    );
}

// -- queue drain / graceful shutdown ----------------------------------------

/// Closing the producer while items are still buffered or in flight does not
/// cancel them: every item is still processed to completion, and the output
/// channel only closes (workers only exit) once everything has drained.
///
/// Enqueues from a background task while draining concurrently in the
/// foreground — draining only after every item is enqueued would require the
/// (bounded) output channel to hold more results than its capacity while
/// nothing reads it, which can deadlock the producer against the very
/// workers it's waiting on. Concurrent drain is also how this queue is meant
/// to be used in practice.
#[tokio::test]
async fn closing_the_queue_drains_remaining_work_before_workers_exit() {
    let (tx, rx, mut workers) = queue::spawn(
        QueueConfig {
            max_queued: 5,
            max_active: 2,
        },
        |item: u32| async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            item * 10
        },
    );

    let producer = tokio::spawn(async move {
        for i in 0..8u32 {
            tx.enqueue(i).await.unwrap();
        }
        // Graceful shutdown: stop producing immediately. Items 0..8 are all
        // already queued/in-flight and must still be delivered.
        tx.close();
    });

    let results = drain_all(rx).await;
    producer.await.unwrap();
    let mut items: Vec<u32> = results.iter().map(|(item, _)| *item).collect();
    items.sort_unstable();
    assert_eq!(
        items,
        (0..8).collect::<Vec<_>>(),
        "no queued item is lost on close"
    );

    for (item, result) in &results {
        assert_eq!(
            *result,
            item * 10,
            "each item must still be fully processed"
        );
    }

    // All workers must have actually exited (not hung) once the channel drained.
    let mut exited = 0;
    while workers.join_next().await.is_some() {
        exited += 1;
    }
    assert_eq!(
        exited, 2,
        "both workers must exit after the drain completes"
    );
}

// -- no lost work ------------------------------------------------------------

/// A larger run through a queue much smaller than the item count: every item
/// is delivered exactly once, none dropped, none duplicated.
///
/// Producing and draining run concurrently (see the comment on
/// `closing_the_queue_drains_remaining_work_before_workers_exit` for why
/// draining only after every item is enqueued isn't safe here).
#[tokio::test]
async fn no_item_is_lost_across_many_more_items_than_queue_capacity() {
    const N: usize = 50;
    let (tx, rx, _workers) = queue::spawn(
        QueueConfig {
            max_queued: 3,
            max_active: 4,
        },
        |item: usize| async move { item },
    );

    let producer = tokio::spawn(async move {
        for i in 0..N {
            tx.enqueue(i).await.unwrap();
        }
        tx.close();
    });

    let results = drain_all(rx).await;
    producer.await.unwrap();

    let mut seen: Vec<usize> = results.into_iter().map(|(item, _)| item).collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..N).collect::<Vec<_>>());
}

// -- no duplicate active work -------------------------------------------------

/// Every individually enqueued item is handed to exactly one worker, never
/// two — the shared-`Mutex`-guarded `recv` means a message can't be
/// delivered twice, no matter how many workers race to pull the next item.
///
/// This proves single-delivery *per enqueued message*, all 20 of which
/// carry distinct values here. It intentionally does not claim that two
/// *separate* messages carrying the same logical identity (e.g. the same
/// `VersionId` enqueued twice) can never run concurrently — this queue is
/// content-blind and performs no identity-level deduplication; avoiding
/// that scenario is a producer-discipline property (Phase 2's `reconcile`
/// only ever offers each distinct eligible version once per pass) and,
/// later, the Phase 4 Claim step's responsibility, not this queue's.
#[tokio::test]
async fn no_two_workers_ever_process_the_same_enqueued_item_concurrently() {
    let in_flight: Arc<Mutex<HashSet<u32>>> = Arc::new(Mutex::new(HashSet::new()));
    let violation = Arc::new(AtomicUsize::new(0));

    let (tx, rx, _workers) = {
        let in_flight = Arc::clone(&in_flight);
        let violation = Arc::clone(&violation);
        queue::spawn(
            QueueConfig {
                max_queued: 4,
                max_active: 4,
            },
            move |item: u32| {
                let in_flight = Arc::clone(&in_flight);
                let violation = Arc::clone(&violation);
                async move {
                    let inserted = in_flight.lock().await.insert(item);
                    if !inserted {
                        violation.fetch_add(1, Ordering::SeqCst);
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    in_flight.lock().await.remove(&item);
                    item
                }
            },
        )
    };

    // Every one of these 20 values is globally unique (round * 100 + i): if
    // the queue ever delivered the *same message* to two workers at once,
    // `insert` above would observe that value already present and record a
    // violation. Producing and draining run concurrently for the same reason
    // as the other multi-item tests in this file.
    let producer = tokio::spawn(async move {
        for round in 0..5u32 {
            for i in 0..4u32 {
                tx.enqueue(round * 100 + i).await.unwrap();
            }
        }
        tx.close();
    });

    let results = drain_all(rx).await;
    producer.await.unwrap();

    assert_eq!(results.len(), 20);
    assert_eq!(
        violation.load(Ordering::SeqCst),
        0,
        "no item should ever be observed as already in-flight when a worker claims it"
    );
}
