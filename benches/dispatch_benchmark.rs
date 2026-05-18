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

fn bench_dispatch_with_result(c: &mut Criterion) {
    // Measures the `dispatch` path (returns `DispatchResult`) vs.
    // `emit` (fire-and-forget). The success-path Vec stays empty in
    // both, so the gap reflects result-struct construction cost.
    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let _id = dispatcher.on(move |event: &BenchEvent| {
        let _ = black_box(event.id);
        let _ = black_box(event.data.len());
        counter_clone.fetch_add(1, Ordering::Relaxed);
    });

    c.bench_function("dispatch_with_result", |b| {
        b.iter(|| {
            let result = dispatcher.dispatch(black_box(BenchEvent {
                id: 1,
                data: "benchmark".to_string(),
            }));
            let _ = black_box(result.listener_count());
        })
    });
}

#[cfg(feature = "async")]
fn bench_dispatch_async_single_listener(c: &mut Criterion) {
    use tokio::runtime::Builder;

    // Build a dedicated current-thread tokio runtime for the bench;
    // criterion drives `to_async(...)` against it.
    let rt = Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build tokio runtime for async bench");

    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let _id = dispatcher.subscribe_async(move |event: &BenchEvent| {
        let id = event.id;
        let data_len = event.data.len();
        let counter = counter_clone.clone();
        async move {
            let _ = black_box(id);
            let _ = black_box(data_len);
            counter.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    });

    c.bench_function("dispatch_async_single_listener", |b| {
        b.to_async(&rt).iter(|| async {
            let result = dispatcher
                .dispatch_async(black_box(BenchEvent {
                    id: 1,
                    data: "benchmark".to_string(),
                }))
                .await;
            let _ = black_box(result.listener_count());
        })
    });
}

#[cfg(feature = "async")]
fn bench_dispatch_async_ten_listeners(c: &mut Criterion) {
    use tokio::runtime::Builder;

    let rt = Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build tokio runtime for async bench");

    let dispatcher = EventDispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..10 {
        let counter_clone = counter.clone();
        let _id = dispatcher.subscribe_async(move |event: &BenchEvent| {
            let id = event.id;
            let data_len = event.data.len();
            let counter = counter_clone.clone();
            async move {
                let _ = black_box(id);
                let _ = black_box(data_len);
                counter.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        });
    }

    c.bench_function("dispatch_async_ten_listeners", |b| {
        b.to_async(&rt).iter(|| async {
            let result = dispatcher
                .dispatch_async(black_box(BenchEvent {
                    id: 1,
                    data: "benchmark".to_string(),
                }))
                .await;
            let _ = black_box(result.listener_count());
        })
    });
}

#[cfg(feature = "async")]
criterion_group!(
    benches,
    bench_single_listener,
    bench_multiple_listeners,
    bench_dispatch_with_result,
    bench_dispatch_async_single_listener,
    bench_dispatch_async_ten_listeners,
);

#[cfg(not(feature = "async"))]
criterion_group!(
    benches,
    bench_single_listener,
    bench_multiple_listeners,
    bench_dispatch_with_result,
);

criterion_main!(benches);
