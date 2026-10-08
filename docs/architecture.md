# mod-events — architecture &amp; design rationale

This document explains *why* `mod-events` is built the way it is, not
what it does. It is the document you read when you want to understand
the tradeoffs, the design alternatives that were considered and
rejected, and the contracts the public API depends on.

The reference for *what* each public item does is the
[API reference](api-reference.md) and the rustdoc.

## Goals and non-goals

**Goals.** Sub-microsecond per-listener dispatch on commodity hardware.
Type-safe events with no runtime string keys. Thread-safe by default.
Async support that works with any executor (tokio, async-std, smol,
embassy, …) without locking the consumer to a runtime. Panic-safe on
both sync and async dispatch paths — a panicking listener becomes a
typed error, never crashes the dispatcher. Zero `unwrap()` /
`expect()` / `panic!` in library code. Lock-free metrics on the
dispatch hot path.

**Non-goals.** Cross-process pub/sub (use Redis / Kafka). Job
scheduling or retry semantics (use a job queue). Bounded queueing
between producers and consumers (use a channel). Built-in
serialization or schema evolution (the dispatcher takes typed Rust
values, not bytes). `no_std` (deferred until there is demonstrated
demand). Generic monomorphized dispatch path that avoids `Box<dyn Fn>`
indirection (deferred — the indirection has not been shown to be the
bottleneck).

## Topology

```
              ┌──────────────────────────────────────────────┐
              │              EventDispatcher                 │
              │                                              │
   subscribe ─┼─►  listeners:        ┌──────────────────┐   │
              │   RwLock<HashMap     │ Vec<Listener>    │   │
              │   <TypeId, Arc<      │  ▪ Arc<dyn Fn>   │   │
              │    Vec<Listener>>>>  │  ▪ Priority      │   │
              │                      │  ▪ ListenerId    │   │
              │                      └──────────────────┘   │
              │                                              │
   subscribe_async ─►  async_listeners: similar shape         │
              │                                              │
   add_middleware ─►  middleware:                              │
              │   RwLock<MiddlewareManager (Arc<Vec<..>>)>   │
              │                                              │
              │   metrics:                                   │
              │   RwLock<HashMap<TypeId, Arc<Counters>>>     │
              │       Counters: { AtomicU64 x2, Instant }    │
              └──────────────────────────────────────────────┘
                              ▲
                              │
              dispatch(event) │
              ────────────────┤  1. update metrics (read-lock map,
                              │     atomic updates)
                              │  2. middleware: skip if empty, else
                              │     snapshot the chain and run it
                              │  3. listeners.read().get(type_id) →
                              │     Arc::clone, release the lock,
                              │     iterate, catch_unwind each, push
                              │     errors lazily.
                              │  4. return DispatchResult.
                              ▼
```

Three locks, all `parking_lot::RwLock`. The hot path acquires only
read locks, and releases each one before any user code runs. The
metrics map's write lock is taken at most once per event type, ever:
the first dispatch that sees a new `TypeId`. After that, every
dispatch records into the per-type atomics under a read lock.

## Why these primitives

### `parking_lot::RwLock` instead of `std::sync::RwLock`

Three reasons.

1. **No lock poisoning.** `std::sync::RwLock` poisons if a thread
   panics while holding the guard. The poisoned state propagates as
   `Err(PoisonError)` to every subsequent acquisition, forcing
   library code to either `unwrap()` (which we forbid) or thread the
   error through every public method. `parking_lot::RwLock` does not
   poison. Combined with our `catch_unwind` wrapping of listener
   invocations, the dispatcher cannot end up in an unrecoverable
   state from a panicking listener.

2. **Faster acquisition under low contention.** parking_lot's lock
   acquisition is a single atomic CAS in the uncontended case versus
   std's mutex syscall.

3. **Smaller memory footprint.** Each `parking_lot::RwLock` is one
   `AtomicUsize` plus a parker; `std::sync::RwLock` is larger and
   carries the poisoning flag.

`parking_lot` is exact-pinned to `=0.12.5` in `Cargo.toml` because it
is on the dispatch hot path. REPS §Dependency Management requires
exact pins for critical crates.

### `Arc<EventMetricsCounters>` per event type

Per-event-type counters are stored as `Arc<EventMetricsCounters>` in
the outer metrics map. The dispatch hot path:

1. `self.metrics.read()`: read lock on the outer `HashMap`.
2. `map.get(&type_id)`. If the entry is missing, drop the read lock
   and take the slow path: write lock, double-check via
   `entry().or_insert_with()`, insert. The double-check is what makes
   this race-free; the `loom` model checks in
   `tests/loom_concurrent.rs` prove it.
3. `counters.dispatch_count.fetch_add(1, Relaxed)`: lock-free.
4. `counters.last_dispatch_nanos.fetch_max(created.elapsed(), Relaxed)`:
   the last-dispatch time is stored as nanoseconds since the entry was
   created, because `Instant` has no atomic form. `fetch_max` keeps it
   monotonic when dispatches on different threads race. Earlier
   releases used a `parking_lot::Mutex<Instant>` here, which every
   dispatch of the same event type contended on.
5. Drop the read lock. No user code runs while it is held.

The alternative we rejected was `RwLock<HashMap<TypeId, EventMetadata>>`
where every dispatch took a write lock on the outer map to bump the
counter. That serialised every dispatch in the whole dispatcher and
was the bottleneck on the original 0.1.x design.

`listener_count` is **not** stored as a counter on `EventMetricsCounters`.
It is derived from the live registry at snapshot time inside
`metrics()`. Storing it would require keeping it in sync across
`subscribe`, `unsubscribe`, and `clear` — and a regression in 0.1.x
showed that drift is exactly what happens when the counter and the
registry are not the same value. Now the metric is always exactly
what `listener_count::<T>()` would return.

### Binary insertion (`Vec::partition_point` + `Vec::insert`)

`subscribe_with_priority` inserts the new listener at the position
where the listener vector remains sorted in descending priority,
preserving registration order (FIFO) within equal priority. It is
**O(n)** — `partition_point` is `O(log n)`, `insert` is `O(n)` because
of the shift.

The alternative was the original `push` + `sort_by_key` after each
subscribe, which was **O(n log n)**. For a registry with 100
listeners on a single event type, binary insertion is approximately
6× faster on subscribe. Dispatch cost is unchanged: both approaches
iterate the Vec in order during dispatch.

The exact predicate in the partition_point closure is
`existing.priority >= priority`. Using `>` instead of `>=` would
break FIFO within equal priority — equal-priority listeners would
prepend (LIFO) rather than append (FIFO). The
`prop_dispatch_respects_priority_then_fifo` property test enforces
the contract.

### Sequential async dispatch

`dispatch_async` awaits each listener future sequentially in
descending priority order. Concurrent execution via
`futures::future::join_all` would be faster wall-clock for I/O-bound
listeners, but it would lose the priority contract — a high-priority
listener could finish *after* a low-priority one. Many use cases
depend on the ordering (the high-priority listener is the one that
must observe the event first; downstream listeners depend on its
side effects).

Users who want concurrent execution can call `tokio::spawn` (or the
equivalent in their runtime) from inside the listener and return
`Ok(())` immediately. That fans out without changing the dispatcher's
contract.

### Lazy errors Vec in `DispatchResult`

`DispatchResult` stores `Vec<ListenerError>` (errors only) plus a
`usize` listener count, instead of `Vec<Result<(), ListenerError>>`
(one entry per listener). On the success path the Vec stays empty
and `Vec::new()` does not allocate, so a fully successful dispatch
of N listeners performs **zero heap allocations** for the result.
Only failing dispatches allocate, and they allocate proportional to
the number of failures, not the number of listeners.

Trade-off: `DispatchResult::errors()` returns `&[ListenerError]`
rather than `Vec<&ListenerError>`. This is a slight ergonomic change
from the 0.2.x signature; iterating still works the same way.

### `catch_unwind` on both dispatch paths

Sync dispatch wraps each listener call in `std::panic::catch_unwind`.
Async dispatch wraps each listener future in
`futures_util::future::FutureExt::catch_unwind`. A panicking listener
becomes a `ListenerError` with the message prefix
`"listener panicked: "` and the dispatch loop continues. Subsequent
listeners on the same dispatch still run; the dispatcher remains
usable for subsequent events.

`AssertUnwindSafe` is required because user closures close over
arbitrary state that is not necessarily `UnwindSafe`. The contract
the dispatcher promises: *if your listener panics, we catch it and
report it; you are responsible for not leaving captured state in a
broken condition before panicking*. Same contract as the
`std::panic::catch_unwind` boundary in any application.

The async path uses `futures-util` instead of a hand-rolled
`CatchUnwind` future because the hand-rolled version requires an
`unsafe` block for pin projection. Pulling in the well-audited
`futures-util` implementation is preferable to growing the unsafe
surface — REPS §Code Quality minimises `unsafe`. The `futures-util`
dependency is gated behind the `async` cargo feature so consumers
who disable async pay nothing for it.

### Runtime-agnostic async

`mod-events` does not depend on tokio at runtime. The `async`
cargo feature enables `subscribe_async`, `dispatch_async`, and the
`AsyncEventListener` trait, all of which use only `std::future::Future`
and `std::pin::Pin`. The crate works with tokio, async-std, smol,
embassy, or any other executor that polls a `Future`.

The `[dev-dependencies]` block does include tokio with `features = ["full"]`
because the integration tests and examples use `#[tokio::main]` and
`#[tokio::test]` for convenience. Dev-dependencies are not propagated
to consumers.

## Concurrency model

Three lock domains, each held for the smallest possible scope:

1. **Listener registry.** Each event type maps to a copy-on-write
   `Arc<Vec<Listener>>`. Dispatch read-locks the map only long enough
   to clone that `Arc` (one reference-count increment, no allocation),
   releases the lock, and then runs the listeners. Subscribe,
   unsubscribe, and clear take the write lock and use
   `Arc::make_mut`, which edits the vector in place when no dispatch
   holds a snapshot and clones it otherwise. Because no lock is held
   while user code runs, listeners can call back into the dispatcher:
   emit other events, subscribe, unsubscribe themselves, or clear.
   Releases up to `1.0.0` held the read lock for the whole dispatch
   loop, which deadlocked on any of those calls (and on a nested
   dispatch while another thread was waiting to subscribe, since
   `parking_lot`'s fair `RwLock` queues new readers behind a waiting
   writer). `tests/reentrancy.rs` covers each case.

   The trade-off: a dispatch works from the listener list as it was
   when the dispatch started. A listener added mid-dispatch first runs
   on the next dispatch, and a listener removed while a dispatch is in
   progress (on this or another thread) can still be invoked by that
   in-flight dispatch.

2. **Async listener registry.** Same shape as the sync registry.
   `dispatch_async` clones the per-type `Arc` under the read lock and
   releases it before the first `await`. Holding a `parking_lot` guard
   across an `await` would make the future `!Send` and stall writers
   for as long as the slowest listener, so the lock is never held
   there.

3. **Metrics map.** Read-locked on every dispatch. Write-locked only
   on the first dispatch of a new event type to insert the per-type
   `Arc<EventMetricsCounters>`. After that, every dispatch is
   read-only against the map and lock-free against the per-type
   counters.

The middleware chain uses its own `RwLock<MiddlewareManager>` with
the same copy-on-write `Arc<Vec<..>>` layout: dispatch snapshots the
chain under the read lock and runs it after releasing the lock, so
middleware may also call back into the dispatcher. An `AtomicBool`
mirrors whether the chain is non-empty, so the common no-middleware
dispatch skips the lock entirely. `add_middleware` and
`clear_middleware` take the write lock.

## Why some things were rejected

### `DashMap` for the listener registry

DashMap shards the map into N stripes, allowing concurrent writes to
different stripes. It would help if subscribe-during-dispatch were
the bottleneck.

Profiling has not shown that. The dispatch path holds only a *read*
lock on the listener registry; concurrent dispatches do not contend
with each other. The only contention is subscribe-vs-dispatch, and
that is rare in practice (most callers register all listeners at
startup, then dispatch).

DashMap also has its own per-shard overhead and a non-trivial
transitive dependency tree. We will revisit when there is profiling
evidence that the global RwLock is the bottleneck.

### Per-listener async timeout

A `dispatch_async_with_timeout(event, Duration)` API would bound how
long the dispatch can take. Users who need this can wrap their own
listener future with their runtime's timeout primitive
(`tokio::time::timeout(d, do_work())`). One line in the listener
body. Adding a built-in API would lock us to one runtime's timer
abstraction; staying out of the timeout business keeps us
runtime-agnostic.

### Generic monomorphized dispatch (`TypedDispatcher<E>`)

A parallel dispatcher type generic over the event type would
eliminate the `Box<dyn Fn>` indirection on the dispatch path. It
would double the public API surface and double the compile time of
crates that use both flavours. We will add this only if profiling
shows the indirection is the dominant cost.

### `cargo-fuzz` harness

Fuzzers earn their keep on parsers and codecs. An event dispatcher
takes typed Rust values, not untrusted bytes. The proptest suite
covers our threat model; fuzz adds little beyond it.

### `no_std` support

A meaningful undertaking — we depend on `std::collections::HashMap`,
`std::sync::Arc`, `std::time::Instant`, `parking_lot`, and
`futures-util`. All have `alloc`-only equivalents but adopting them
is a project of its own. Deferred until there is demonstrated demand
from embedded users.

## Test architecture

Five test surfaces, each catching a different class of bug.

| File | Catches |
|------|---------|
| `tests/integration_tests.rs` | Public-API behavior, feature-by-feature. Hand-written, covers the documented contract. |
| `tests/property_tests.rs` | Invariants across randomised inputs. Catches edge cases the author would not write by hand. |
| `tests/concurrent.rs` | Multi-threaded stress under contention. Caught the listener-count drift bug in 0.2.x. |
| `tests/loom_concurrent.rs` | Exhaustive interleaving of the double-checked-locking pattern in `counters_for`. Proves the algorithm correct under every legal schedule. |
| `tests/benchmarks.rs` | Smoke benchmarks. Real performance tracking lives in `benches/dispatch_benchmark.rs` (criterion). |

CI runs all five surfaces on every push. ASAN runs the integration
+ property + concurrent suites under sanitiser instrumentation on
nightly Linux.

## Memory profile

Per `EventDispatcher::new`:
- One `Arc` allocation for the empty middleware chain. The `RwLock`s
  and their maps live inline in the dispatcher; empty maps do not
  allocate.
- One `AtomicUsize` for `next_id` and one `AtomicBool` for the
  middleware fast path.

Per `subscribe` call:
- One `Arc<dyn Fn>` for the wrapped handler.
- Possible Vec growth (geometric reallocation) when the per-event-type
  listener vector exceeds capacity.
- A full copy of that event type's listener vector if a dispatch of the
  same type is in progress at that moment (copy-on-write).
- First-ever subscribe of an event type: one `Arc<Vec>` for the
  listener slot and one `Arc<EventMetricsCounters>` for the metrics
  slot.

Per `dispatch` / `dispatch_async` call (success path):
- **Zero heap allocations** after the first dispatch of a given event
  type. The listener list is borrowed through an `Arc` clone; the lazy
  errors Vec stays empty, and `Vec::new()` does not allocate. (Before
  this change `dispatch_async` collected the handlers into a fresh
  `Vec` on every call.)
- Per failing listener: one `ListenerError` (the user's listener
  constructed it; it is moved into the errors vector).

Per `emit` call:
- Same allocation profile as a successful dispatch, minus the
  `DispatchResult` itself. `emit` skips the result-building path
  entirely — it does not even maintain a listener count or errors
  vector. Pure fire-and-forget.

Per `metrics` call:
- One `HashMap` allocation for the snapshot, sized to the number of
  registered event types.
- Three or four read-lock acquisitions (listeners, async listeners
  if the feature is enabled, and the metrics map).

## What changes between minor versions

The `1.x` line is covered by [`STABILITY.md`](STABILITY.md), which is
the binding policy. In short, what is stable for all of `1.x`:
- The shape of `EventDispatcher`, `Event`, `EventListener`,
  `AsyncEventListener`, `Priority`, `ListenerId`, `DispatchResult`,
  `EventMetadata`, `ListenerError`, `AsyncEventResult`.
- The behavioral contracts of `dispatch`, `dispatch_async`, `emit`,
  `subscribe`, `subscribe_with_priority`, `subscribe_async`,
  `subscribe_async_with_priority`, `add_middleware`, `unsubscribe`,
  `listener_count`, `metrics`, `clear`, `clear_middleware`.
- The `async` cargo feature being on by default.
- MSRV being Rust 1.81 or earlier.

What may evolve before `1.0`:
- Internal field types behind public accessors.
- Performance characteristics (the contract is "sub-microsecond per
  listener", not "exactly N nanoseconds").
- New optional cargo features.
