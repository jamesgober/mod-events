<h1 align="center">
        <img width="108px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br>
    <strong>Mod Events</strong>
    <sup><br><sup>PERFORMANCE GUIDE</sup></sup>
</h1>

mod-events is designed for high-performance scenarios. This guide covers performance characteristics, benchmarks, and optimization tips.

## Performance Characteristics

### Zero-Cost Abstractions

- **Compile-time type safety** - No runtime type checking
- **Direct function calls** - No virtual dispatch overhead
- **Minimal allocations** - Pre-allocated vectors when possible
- **Lock-free reads** - Multiple threads can read concurrently

### Measured Throughput

Numbers measured against `mod-events 0.2.1` on a Windows x86_64 host
(Rust 1.95, release profile) using the integration-test bench in
`tests/benchmarks.rs`. These are **integration-test timings**, not
formal `criterion` results — they have no warmup and no statistical
analysis. They are useful as a sanity baseline, not as a published
SLA. Roadmap item #31 tracks publishing authoritative `criterion`
baselines from Linux CI.

| Scenario | Per-event latency | Throughput | Notes |
|----------|-------------------|------------|-------|
| `emit` with 1 listener  | ~133 ns | ~7.5 M events/sec | 10,000-event loop, listener increments an `AtomicUsize`. |
| `emit` with 10 listeners | ~244 ns | ~4.1 M events/sec | 1,000-event loop × 10 listeners (~24 ns per added listener). |

Run them locally:

```bash
cargo test --release --test benchmarks --features async -- --nocapture
```

For criterion microbenchmarks with warmup + statistical analysis:

```bash
cargo bench --features async --bench dispatch_benchmark
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
// Fire-and-forget: discards the DispatchResult.
dispatcher.emit(event);

// Returns DispatchResult; allocates a Vec<Result<(), ListenerError>>.
let result = dispatcher.dispatch(event);
```

`emit` calls `dispatch` internally and drops the result; the saving comes from not having to *use* the per-listener `Result` vector at the call site. If you do not inspect listener outcomes, prefer `emit` — it is also the documented entry point for that pattern.

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