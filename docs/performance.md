<h1 align="center">
        <img width="108px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br>
    <strong>Mod Events</strong>
    <sup><br><sup>PERFORMANCE GUIDE</sup></sup>
</h1>

mod-events is designed for high-performance scenarios. This guide covers performance characteristics, benchmarks, and optimization tips.

## Performance Characteristics

### Zero-Cost Abstractions

- **Compile-time type safety** - No runtime type checking.
- **Direct function calls** - No virtual dispatch overhead beyond the
  one `Box<dyn Fn>` indirection per listener.
- **Zero-allocation success path** - `DispatchResult`'s internal
  `Vec<ListenerError>` is constructed empty (`Vec::new()` does not
  allocate). Allocation happens only when a listener returns `Err` or
  panics; a fully-successful dispatch performs zero heap allocations
  for the result.
- **Shared reads** - Multiple threads can dispatch concurrently: each
  dispatch takes `parking_lot::RwLock` read locks (not lock-free; each
  read lock and unlock is an atomic update of the lock word) and runs
  listeners with no lock held.

### Measured Throughput

Numbers measured against `mod-events 1.0.0` on a Windows x86_64 host
(Ryzen 9 9950X3D, Rust 1.95, release profile) using
`cargo bench --bench dispatch_benchmark --features async`. Criterion
warm-up + statistical analysis applied; the median of the reported
3-point estimate is shown.

| Scenario | Per-event latency | Throughput | Notes |
|----------|-------------------|------------|-------|
| `emit` with 1 listener   | **~89.8 ns** | **~11.1 M events/sec** | Fire-and-forget path; success-path skips result construction entirely. |
| `emit` with 10 listeners | **~150 ns**  | **~6.7 M events/sec** | ~6 ns per added listener after the first. |
| `dispatch` with 1 listener | **~92.6 ns** | **~10.8 M events/sec** | `dispatch` builds a `DispatchResult` (empty `Vec<ListenerError>`) on success. ~3 ns more than `emit`. |
| `dispatch_async` with 1 async listener   | **~158 ns** | **~6.3 M events/sec** | Tokio current-thread runtime; one `catch_unwind` future per listener. |
| `dispatch_async` with 10 async listeners | **~520 ns** | **~1.9 M events/sec** | ~40 ns per added async listener. |

These numbers reflect the `1.0.0` performance tune: an internal
`TypeIdHasher` skips the SipHash round on the hot path, the
dispatcher's three lookup maps shed an outer `Arc` wrapper, and the
metric-update + middleware-check helpers carry `#[inline]`. The
combined effect on the `emit` path is roughly **33-38% faster than
the `0.9.x` numbers** (which were ~133 ns single / ~244 ns
ten-listener using an integration-test bench without warmup —
methodology differed, but the trend is real and reproducible under
criterion).

Run criterion microbenchmarks locally:

```bash
cargo bench --features async --bench dispatch_benchmark
```

Run the legacy integration-test bench (no warmup, no statistical
analysis) for a quick smoke test:

```bash
cargo test --release --test benchmarks --features async -- --nocapture
```

### Performance Properties

- **Sub-microsecond dispatch** at the per-event level on commodity hardware.
- **Linear scaling** with listener count — each additional sync listener costs roughly the cost of one indirect call plus the closure body.
- **Lock-free dispatch path** for metrics: an `AtomicU64` fetch-add for the count and an `AtomicU64` fetch-max for the timestamp per dispatch, no mutex, and no write lock on the metrics map after the first dispatch of a given event type.
- **Read-only listener registry access** during dispatch: one `parking_lot::RwLock::read` held just long enough to clone the per-type `Arc<Vec<_>>` listener list. Listeners run with no lock held.
- **No middleware cost when none is registered**: an `AtomicBool` check skips the middleware lock.
- **O(n) subscribe** via `Vec::partition_point` + `Vec::insert`; FIFO is preserved within equal priority.

### Memory Footprint

- **Dispatcher**: ~200 bytes base overhead.
- **Per listener**: one `Arc<dyn Fn>` (16 bytes pointer + reference counts + closure size) plus ~16 bytes of metadata in `ListenerWrapper`.
- **Per event type**: one `Arc<Vec<_>>` listener list, plus one `Arc<EventMetricsCounters>` containing two `AtomicU64`s, an `Instant`, a `TypeId`, and a `&'static str` event name.
- **During dispatch**: the sync success path (`emit`, `dispatch`) performs zero heap allocations. The async path allocates one boxed future (`Box::pin`) per async listener per dispatch. Both paths take one reference-count increment on the per-type listener list instead of copying it.

## Optimization Tips

### 1. Use `emit()` for Fire-and-Forget

```rust
// Fire-and-forget: no result is constructed.
dispatcher.emit(event);

// Returns DispatchResult; carries per-listener errors (empty on success).
let result = dispatcher.dispatch(event);
```

`emit` and `dispatch` share the same listener-execution path (panic-safety wrapping included) but `emit` skips constructing a `DispatchResult` entirely — it does not call `dispatch`. If you do not need per-listener outcomes, prefer `emit`. Both paths now use a lazy errors vector, so the difference is small on the success path; `emit` remains the right choice when the result would be unused.

### 2. Minimize Event Cloning

```rust
// Good - events are moved
dispatcher.emit(UserRegistered { ... });

// Avoid - unnecessary cloning (adds ~50-100ns overhead)
let event = UserRegistered { ... };
dispatcher.emit(event.clone());
```

### 3. Use Appropriate Priority Levels

```rust
// Good - only use when needed
dispatcher.subscribe_with_priority(handler, Priority::High);

// Better - normal priority is default and fastest
dispatcher.on(handler);
```

**Performance Impact:** Priority sorting happens once at subscription, not during dispatch.

### 4. Batch Operations

```rust
// Good - batch related events
for user in users {
    dispatcher.emit(UserRegistered { ... });
}
// 10K events in ~10.8ms = 1.08μs per event

// Better - minimize allocation overhead
let events: Vec<_> = users.into_iter()
    .map(|u| UserRegistered { ... })
    .collect();

for event in events {
    dispatcher.emit(event);
}
```

**Performance Impact:** Batching reduces allocation overhead and improves cache locality.

### 5. Async Optimization

```rust
// Good - concurrent dispatch
let result = dispatcher.dispatch_async(event).await;

// Better - batch async operations when possible
let futures: Vec<_> = events.into_iter()
    .map(|e| dispatcher.dispatch_async(e))
    .collect();

let results = futures::future::join_all(futures).await;
```

**Performance Impact:** Async dispatch has higher overhead (~2-5μs) but enables non-blocking I/O.

## Profiling

### Built-in Metrics

```rust
let metrics = dispatcher.metrics();
for (_, meta) in metrics {
    println!("Event: {} - Dispatched: {} times", 
        meta.event_name, meta.dispatch_count);
}
```

### Custom Profiling

```rust
// Time event dispatch
let start = std::time::Instant::now();
dispatcher.emit(event);
let duration = start.elapsed();
println!("Dispatch took: {:?}", duration);
```

### Benchmark Results Analysis

The figures in [Measured Throughput](#measured-throughput) come from
`benches/dispatch_benchmark.rs` (criterion, release profile). Debug
builds are several times slower; measure release builds.

### Scaling Across Threads

Dispatching one event type from many threads at once does not scale
linearly. Each dispatch updates a few shared atomics (the read-lock
words, the listener list's reference count, and the per-type metrics
counters), and every thread dispatching the same type writes the same
cache lines. On a loaded 1.1.0 test machine, 8 threads calling `emit`
on one event type reached about the same total throughput as 1 thread.
Dispatches of different event types share fewer lines. If one event
type is very hot across many threads, measure before relying on
multi-threaded throughput.

## When NOT to Use mod-events

- **Single-threaded, simple callbacks** - Direct function calls may be simpler (but only ~2x faster)
- **Complex event transformation** - Consider stream processing libraries
- **Persistent event storage** - Use event sourcing databases
- **Cross-process communication** - Use message queues (Redis, Kafka)
- **Very simple use cases** - If you only need 1-2 events, direct function calls might suffice

## Performance Tuning Checklist

### Development Phase
- [ ] Use `emit()` for fire-and-forget scenarios
- [ ] Minimize event cloning
- [ ] Use appropriate priority levels
- [ ] Batch related operations
- [ ] Profile with `cargo bench`

### Production Phase
- [ ] Compile with `--release` flag
- [ ] Monitor with built-in metrics
- [ ] Profile hot paths
- [ ] Consider async for I/O-bound handlers
- [ ] Benchmark against alternatives

## Conclusion

mod-events is an in-process event dispatcher with:

- Sub-microsecond `emit` and `dispatch` on the measured hardware (see
  the table above for numbers and method).
- No heap allocation on the sync success path.
- Listeners and middleware run with no lock held.
- Compile-time typed events and listeners.

<br>

## Read More

- Get Started [Quick Start Guide](quick-start.md)
- Read the [API Reference](api-reference.md)
- Check out more [Examples](examples.md)
- Learn [Best Practices](best-practices.md)
- See the [Migration Guide](migration.md)