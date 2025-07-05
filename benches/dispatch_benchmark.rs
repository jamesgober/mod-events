use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mod_events::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct BenchEvent {
    _id: u64,
    _data: String,
}

impl Event for BenchEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn bench_single_listener(c: &mut Criterion) {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    dispatcher.on(move |_: &BenchEvent| {
        counter_clone.fetch_add(1, Ordering::Relaxed);
    });

    c.bench_function("single_listener", |b| {
        b.iter(|| {
            dispatcher.emit(black_box(BenchEvent {
                _id: 1,
                _data: "benchmark".to_string(),
            }));
        })
    });
}

fn bench_multiple_listeners(c: &mut Criterion) {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // Add 10 listeners
    for _ in 0..10 {
        let counter_clone = counter.clone();
        dispatcher.on(move |_: &BenchEvent| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
    }

    c.bench_function("multiple_listeners", |b| {
        b.iter(|| {
            dispatcher.emit(black_box(BenchEvent {
                _id: 1,
                _data: "benchmark".to_string(),
            }));
        })
    });
}

criterion_group!(benches, bench_single_listener, bench_multiple_listeners);
criterion_main!(benches);
