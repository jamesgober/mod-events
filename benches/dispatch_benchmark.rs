use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mod_events::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct BenchEvent {
    id: u64,
    data: String,
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

    let _id = dispatcher.on(move |event: &BenchEvent| {
        // Touch the fields so they cannot be optimized away.
        let _ = black_box(event.id);
        let _ = black_box(event.data.len());
        counter_clone.fetch_add(1, Ordering::Relaxed);
    });

    c.bench_function("single_listener", |b| {
        b.iter(|| {
            dispatcher.emit(black_box(BenchEvent {
                id: 1,
                data: "benchmark".to_string(),
            }));
        })
    });
}

fn bench_multiple_listeners(c: &mut Criterion) {
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // Register ten listeners; each touches the event payload.
    for _ in 0..10 {
        let counter_clone = counter.clone();
        let _id = dispatcher.on(move |event: &BenchEvent| {
            let _ = black_box(event.id);
            let _ = black_box(event.data.len());
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
    }

    c.bench_function("multiple_listeners", |b| {
        b.iter(|| {
            dispatcher.emit(black_box(BenchEvent {
                id: 1,
                data: "benchmark".to_string(),
            }));
        })
    });
}

criterion_group!(benches, bench_single_listener, bench_multiple_listeners);
criterion_main!(benches);
