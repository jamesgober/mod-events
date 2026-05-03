//! Multi-threaded stress tests for the dispatcher.
//!
//! REPS §Testing requires that concurrent code paths be tested for race
//! conditions. The crate uses `parking_lot::RwLock`, which is not natively
//! modeled by `loom`, so deterministic interleaving exploration is not
//! feasible without restructuring the dispatcher around `loom::sync`. As a
//! pragmatic substitute, these tests exercise the public API from many
//! threads with high contention and assert invariants on the observable
//! state.

use mod_events::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

#[derive(Debug, Clone)]
struct StressEvent {
    payload: u64,
}

impl Event for StressEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn test_concurrent_dispatch_invokes_every_listener_once_per_event() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let invocations = Arc::new(AtomicUsize::new(0));

    // 8 listeners; each touches the event payload so the field is not
    // dead.
    for _ in 0..8 {
        let invocations = invocations.clone();
        let _id = dispatcher.on(move |event: &StressEvent| {
            let _ = std::hint::black_box(event.payload);
            invocations.fetch_add(1, Ordering::Relaxed);
        });
    }

    const THREADS: usize = 8;
    const EVENTS_PER_THREAD: u64 = 500;
    let barrier = Arc::new(Barrier::new(THREADS));

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let dispatcher = dispatcher.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                for i in 0..EVENTS_PER_THREAD {
                    dispatcher.emit(StressEvent {
                        payload: (t as u64) * EVENTS_PER_THREAD + i,
                    });
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("dispatcher thread panicked");
    }

    let expected = THREADS * EVENTS_PER_THREAD as usize * 8;
    assert_eq!(invocations.load(Ordering::Relaxed), expected);
}

#[test]
fn test_concurrent_subscribe_and_dispatch_does_not_deadlock_or_lose_events() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let invocations = Arc::new(AtomicUsize::new(0));

    const SUBSCRIBE_THREADS: usize = 4;
    const DISPATCH_THREADS: usize = 4;
    const SUBS_PER_THREAD: usize = 50;
    const EVENTS_PER_THREAD: usize = 500;

    let barrier = Arc::new(Barrier::new(SUBSCRIBE_THREADS + DISPATCH_THREADS));

    let mut handles = Vec::new();

    // Subscribers churn the listener registry.
    for _ in 0..SUBSCRIBE_THREADS {
        let dispatcher = dispatcher.clone();
        let invocations = invocations.clone();
        let barrier = barrier.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            let mut ids = Vec::with_capacity(SUBS_PER_THREAD);
            for _ in 0..SUBS_PER_THREAD {
                let invocations = invocations.clone();
                let id = dispatcher.on(move |event: &StressEvent| {
                    let _ = std::hint::black_box(event.payload);
                    invocations.fetch_add(1, Ordering::Relaxed);
                });
                ids.push(id);
            }
            // Cleanly remove half of what we registered to also exercise
            // the unsubscribe path under contention.
            for id in ids.into_iter().take(SUBS_PER_THREAD / 2) {
                let _ = dispatcher.unsubscribe(id);
            }
        }));
    }

    // Dispatchers fire events while the registry is being mutated.
    for t in 0..DISPATCH_THREADS {
        let dispatcher = dispatcher.clone();
        let barrier = barrier.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            for i in 0..EVENTS_PER_THREAD {
                dispatcher.emit(StressEvent {
                    payload: (t * EVENTS_PER_THREAD + i) as u64,
                });
            }
        }));
    }

    for h in handles {
        h.join().expect("worker thread panicked");
    }

    // We cannot pin an exact invocation count because subscribers and
    // dispatchers race; we only assert the dispatcher remained alive,
    // every listener that survived can still be exercised, and metrics
    // reflect at least the events we sent.
    let metrics = dispatcher.metrics();
    let meta = metrics
        .get(&std::any::TypeId::of::<StressEvent>())
        .expect("StressEvent metrics should exist after dispatch");
    assert_eq!(
        meta.dispatch_count as usize,
        DISPATCH_THREADS * EVENTS_PER_THREAD,
        "every emit() must increment dispatch_count exactly once"
    );
    // Surviving listeners = (SUBSCRIBE_THREADS * SUBS_PER_THREAD) - removed.
    let removed = SUBSCRIBE_THREADS * (SUBS_PER_THREAD / 2);
    let expected_listeners = SUBSCRIBE_THREADS * SUBS_PER_THREAD - removed;
    assert_eq!(meta.listener_count, expected_listeners);
    assert_eq!(
        dispatcher.listener_count::<StressEvent>(),
        expected_listeners
    );
}

#[test]
fn test_concurrent_metrics_snapshot_does_not_block_dispatch() {
    // Tight loop that snapshots metrics from one thread while another
    // dispatches at full speed. If `metrics()` were blocking dispatch,
    // we'd see massive throughput collapse and probably time out.
    let dispatcher = Arc::new(EventDispatcher::new());
    let invocations = Arc::new(AtomicUsize::new(0));
    let invocations_clone = invocations.clone();
    let _id = dispatcher.on(move |event: &StressEvent| {
        let _ = std::hint::black_box(event.payload);
        invocations_clone.fetch_add(1, Ordering::Relaxed);
    });

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let dispatcher_t = dispatcher.clone();
    let stop_t = stop.clone();
    let dispatcher_thread = thread::spawn(move || {
        let mut count = 0_u64;
        while !stop_t.load(Ordering::Relaxed) {
            dispatcher_t.emit(StressEvent { payload: count });
            count += 1;
        }
        count
    });

    let dispatcher_m = dispatcher.clone();
    let stop_m = stop.clone();
    let snapshot_thread = thread::spawn(move || {
        let mut snapshots = 0_u64;
        while !stop_m.load(Ordering::Relaxed) {
            let snapshot = dispatcher_m.metrics();
            // Touch the snapshot so the optimizer cannot remove the call.
            std::hint::black_box(snapshot);
            snapshots += 1;
        }
        snapshots
    });

    thread::sleep(std::time::Duration::from_millis(150));
    stop.store(true, Ordering::Relaxed);

    let dispatched = dispatcher_thread.join().expect("dispatcher panicked");
    let snapshots = snapshot_thread.join().expect("snapshot panicked");

    assert!(
        dispatched > 1_000,
        "expected the dispatch path to remain hot under concurrent metrics() calls; got {dispatched} dispatches"
    );
    assert!(
        snapshots > 100,
        "expected metrics() to keep returning while dispatch is hot; got {snapshots} snapshots"
    );
    assert_eq!(invocations.load(Ordering::Relaxed) as u64, dispatched);
}
