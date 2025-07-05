//! Benchmarks for mod-events

use mod_events::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
fn benchmark_single_listener() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    dispatcher.on(move |_: &BenchmarkEvent| {
        counter_clone.fetch_add(1, Ordering::Relaxed);
    });

    let start = std::time::Instant::now();

    for i in 0..10000 {
        dispatcher.emit(BenchmarkEvent {
            id: i,
            data: format!("event_{i}"),
        });
    }

    println!("Single listener: {:?} for 10,000 events", start.elapsed());
    assert_eq!(counter.load(Ordering::Relaxed), 10000);
}

#[test]
fn benchmark_multiple_listeners() {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // Add 10 listeners
    for _ in 0..10 {
        let counter_clone = counter.clone();
        dispatcher.on(move |_: &BenchmarkEvent| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
    }

    let start = std::time::Instant::now();

    for i in 0..1000 {
        dispatcher.emit(BenchmarkEvent {
            id: i,
            data: format!("event_{i}"),
        });
    }

    let duration = start.elapsed();
    println!("Multiple listeners: {duration:?} for 1,000 events with 10 listeners");
    assert_eq!(counter.load(Ordering::Relaxed), 10000);
}
