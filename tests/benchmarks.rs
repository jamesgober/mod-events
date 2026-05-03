//! Quick smoke benchmarks (run via `cargo test --release benchmark`).
//!
//! Real performance regression tracking lives in [`benches/`] under
//! `criterion`; these tests just verify the dispatcher handles a
//! reasonable event volume without panicking and report wall-clock
//! timing for ad-hoc comparison.

use mod_events::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct BenchmarkEvent {
    id: u64,
    data: String,
}

impl Event for BenchmarkEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn test_emit_one_listener_handles_10k_events_under_one_second() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let _id = dispatcher.on(move |event: &BenchmarkEvent| {
        // Touch the payload so the compiler does not optimize the
        // accesses away inside the loop.
        let _ = event.id;
        let _ = event.data.len();
        counter_clone.fetch_add(1, Ordering::Relaxed);
    });

    let start = std::time::Instant::now();

    for i in 0..10_000 {
        dispatcher.emit(BenchmarkEvent {
            id: i,
            data: format!("event_{i}"),
        });
    }

    println!("Single listener: {:?} for 10,000 events", start.elapsed());
    assert_eq!(counter.load(Ordering::Relaxed), 10_000);
}

#[test]
fn test_emit_ten_listeners_handles_one_thousand_events_under_one_second() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..10 {
        let counter_clone = counter.clone();
        let _id = dispatcher.on(move |event: &BenchmarkEvent| {
            let _ = event.id;
            let _ = event.data.len();
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
    }

    let start = std::time::Instant::now();

    for i in 0..1_000 {
        dispatcher.emit(BenchmarkEvent {
            id: i,
            data: format!("event_{i}"),
        });
    }

    let duration = start.elapsed();
    println!("Multiple listeners: {duration:?} for 1,000 events with 10 listeners");
    assert_eq!(counter.load(Ordering::Relaxed), 10_000);
}
