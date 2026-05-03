//! Loom model checks for the dispatcher's concurrency invariants.
//!
//! `loom` only models `std::sync` primitives, not `parking_lot`. These
//! tests therefore do not exercise [`mod_events::EventDispatcher`]
//! directly; they reproduce the *algorithm* the dispatcher uses on
//! `loom::sync::RwLock` so that loom's exhaustive interleaving
//! exploration covers the pattern. If the algorithm is sound here, it
//! remains sound when transplanted onto the equivalent
//! `parking_lot::RwLock` primitives in production code (parking_lot's
//! own correctness is its own concern, separately audited).
//!
//! The model focuses on [`EventDispatcher::counters_for`] — the only
//! double-checked-locking pattern in the crate, and the only place
//! where a TOCTOU race could plausibly cause torn metrics state.
//!
//! # Running
//!
//! Loom tests do not run under ordinary `cargo test`. Invoke explicitly:
//!
//! ```text
//! RUSTFLAGS="--cfg loom" cargo test --release --test loom_concurrent
//! ```
//!
//! `--release` keeps loom's exploration tractable; debug-mode runs are
//! noticeably slower without changing the conclusions.

#![cfg(loom)]

use loom::sync::atomic::{AtomicUsize, Ordering};
use loom::sync::{Arc, RwLock};
use std::collections::HashMap;

/// Mirror of `EventDispatcher::counters_for<T>`: read-locked fast path
/// returning the existing `Arc` if any, write-locked slow path that
/// double-checks via `entry().or_insert_with()` before creating.
///
/// Each thread that calls this function then increments the returned
/// counter exactly once.
fn increment_via_double_checked(map: &Arc<RwLock<HashMap<u8, Arc<AtomicUsize>>>>) {
    // Fast path: read lock, return clone of existing entry if any.
    let counter = {
        let read = map.read().unwrap();
        read.get(&0).cloned()
    };

    // Slow path: write lock, double-check, insert if missing.
    let counter = counter.unwrap_or_else(|| {
        let mut write = map.write().unwrap();
        Arc::clone(
            write
                .entry(0)
                .or_insert_with(|| Arc::new(AtomicUsize::new(0))),
        )
    });

    let _previous = counter.fetch_add(1, Ordering::Relaxed);
}

#[test]
fn loom_counters_for_inserts_exactly_once_under_concurrent_first_dispatch() {
    loom::model(|| {
        let map: Arc<RwLock<HashMap<u8, Arc<AtomicUsize>>>> = Arc::new(RwLock::new(HashMap::new()));

        let map1 = map.clone();
        let t1 = loom::thread::spawn(move || increment_via_double_checked(&map1));
        let map2 = map.clone();
        let t2 = loom::thread::spawn(move || increment_via_double_checked(&map2));

        t1.join().unwrap();
        t2.join().unwrap();

        // Whichever thread inserted the counter, both threads must see
        // the same `Arc` and both increments must land. If the slow
        // path's double-check were missing, the test would observe a
        // count of 1 in interleavings where both threads pass the read
        // check before either writes.
        let map = map.read().unwrap();
        let counter = map
            .get(&0)
            .expect("counter must exist after either thread inserts");
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    });
}

#[test]
fn loom_counters_for_returns_same_arc_across_concurrent_callers() {
    // Stronger invariant: not only does insertion happen exactly once,
    // but every concurrent caller sees the same `Arc<AtomicUsize>` —
    // there is no scenario in which two threads end up holding distinct
    // counter pointers for the same key.
    loom::model(|| {
        let map: Arc<RwLock<HashMap<u8, Arc<AtomicUsize>>>> = Arc::new(RwLock::new(HashMap::new()));

        let map1 = map.clone();
        let map2 = map.clone();

        let t1 = loom::thread::spawn(move || acquire_counter(&map1));
        let t2 = loom::thread::spawn(move || acquire_counter(&map2));

        let c1 = t1.join().unwrap();
        let c2 = t2.join().unwrap();

        assert!(
            Arc::ptr_eq(&c1, &c2),
            "concurrent callers received distinct Arcs for the same key"
        );
    });
}

fn acquire_counter(map: &Arc<RwLock<HashMap<u8, Arc<AtomicUsize>>>>) -> Arc<AtomicUsize> {
    let counter = {
        let read = map.read().unwrap();
        read.get(&0).cloned()
    };
    counter.unwrap_or_else(|| {
        let mut write = map.write().unwrap();
        Arc::clone(
            write
                .entry(0)
                .or_insert_with(|| Arc::new(AtomicUsize::new(0))),
        )
    })
}
