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
- **Lock-free reads** - Multiple threads can read concurrently
  through `parking_lot::RwLock::read`.

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
- **Lock-free dispatch path** for metrics: `AtomicU64` fetch-add per dispatch, no write lock on the metrics map after the first dispatch of a given event type.
- **Read-only listener registry access** during dispatch: a single `parking_lot::RwLock::read` for the duration of the dispatch loop.
- **O(n) subscribe** via `Vec::partition_point` + `Vec::insert`; FIFO is preserved within equal priority.

### Memory Footprint

- **Dispatcher**: ~200 bytes base overhead.
- **Per listener**: one `Box<dyn Fn>` (16 bytes pointer + boxed closure size) plus ~16 bytes of metadata in `ListenerWrapper`.
- **Per event type**: one `Arc<EventMetricsCounters>` containing an `AtomicU64`, a `Mutex<Instant>`, and a `&'static str` event name — about 64 bytes plus the `Arc` overhead.
- **During dispatch**: zero allocations on the sync path. The async path clones the per-handler `Arc` for each listener, which is a refcount bump (no heap traffic).

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

Our benchmarks show:

```
Criterion (optimized):
- Single listener: 262ns (3.8M events/sec)
- 10 listeners: 344ns (2.9M events/sec)

Manual tests (debug mode):
- Single listener: 1.07μs (928K events/sec)
- 10 listeners: 1.96μs (510K events/sec)
```

**Key Takeaway:** Release builds are **3-4x faster** than debug builds.

## Comparing to Other Solutions

| Feature | mod-events | Channel-based | Callback-based | Node.js | Redis |
|---------|------------|---------------|----------------|---------|-------|
| **Latency** | **262ns-1μs** | 1-10μs | 1-5μs | 2-5μs | 100-500μs |
| **Throughput** | **3.8M/sec** | 100K-1M/sec | 1-5M/sec | 200K-500K/sec | 100K/sec |
| **Type Safety** | Compile-time | Runtime | Runtime | Runtime | Runtime |
| **Performance** | Zero-cost | Allocation overhead | Direct calls | V8 overhead | Network overhead |
| **Async Support** | Native | Native | Manual | Native | Native |
| **Priority System** | Built-in | Manual | Manual | Manual | Manual |
| **Thread Safety** | Built-in | Built-in | Manual | Single-threaded | Built-in |

## Real-World Performance

### Game Engine (60 FPS)
```rust
// Handle 1000+ events per frame
for _ in 0..1000 {
    dispatcher.emit(PlayerMoved { ... });
}
// Completes in ~1ms (1000 × 1μs)
// Leaves 15.67ms for other game logic
```

### Web Server (High Throughput)
```rust
// Handle user actions
dispatcher.emit(UserAction { ... });
// ~1μs per dispatch
// Can handle 1000 events per millisecond
```

### IoT System (Resource Constrained)
```rust
// Minimal memory footprint
// ~200 bytes base + ~100 bytes per listener
// < 1KB total overhead for typical usage
```

### High-Frequency Trading
```rust
// Sub-microsecond latency requirement
dispatcher.emit(MarketUpdate { ... });
// 262ns latency leaves 9.738μs for other processing
// in a 10μs budget
```

## Performance Characteristics by Use Case

### Latency-Critical Applications
- **Best choice**: Use `emit()` for fire-and-forget
- **Expected latency**: 262ns - 1μs
- **Suitable for**: Game engines, HFT, real-time systems

### High-Throughput Applications
- **Best choice**: Batch operations with `emit()`
- **Expected throughput**: 1M+ events/second
- **Suitable for**: Analytics, logging, monitoring

### Mixed Workloads
- **Best choice**: Combine `emit()` and `dispatch()` as needed
- **Expected performance**: 500K+ events/second
- **Suitable for**: Web servers, microservices

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

mod-events delivers **exceptional performance** for in-process event handling:

- **Sub-microsecond latency** (262ns-1μs)
- **Multi-million events per second** throughput
- **Minimal memory overhead** (~200 bytes base)
- **Zero-cost abstractions** with compile-time optimization
- **Thread-safe concurrent access** with minimal contention

It excels in scenarios requiring:
- **Ultra-low latency** (real-time systems, gaming, HFT)
- **High throughput** (analytics, monitoring, logging)
- **Type safety** (compile-time guarantees)
- **Minimal resource usage** (embedded, IoT, resource-constrained)
- **Async/await compatibility** (modern Rust applications)

**Bottom Line:** mod-events is one of the fastest event systems available in any language, making it ideal for performance-critical applications where every nanosecond counts.


<br>

## Read More

- Get Started [Quick Start Guide](quick-start.md)
- Read the [API Reference](api-reference.md)
- Check out more [Examples](examples.md)
- Learn [Best Practices](best-practices.md)
- See the [Migration Guide](migration.md)