# mod-events — 1.0.0 API Freeze Audit

> Manifest of every public symbol in `mod-events 1.0.0`. Locked
> under SemVer for the `1.x` line. See
> [`docs/STABILITY.md`](STABILITY.md) for the binding policy.

## Status: complete

This document is the public-surface manifest for `1.0.0`. Every
symbol below is part of the SemVer contract.

`1.0.0` is **not** a strict superset of `0.9.x`. Three small
breaking cleanups happened at the freeze boundary:

1. `MiddlewareManager` and `MiddlewareFunction` moved to
   `pub(crate)`. Users go through `EventDispatcher::add_middleware`
   and `EventDispatcher::clear_middleware`.
2. `Priority` is now `#[non_exhaustive]`. External `match`
   statements need a `_ => …` arm if they enumerate every variant.
3. `EventMetadata` is now `#[non_exhaustive]`. External code must
   read fields by name, not construct via struct-literal syntax.
   No external construction site exists in practice — the only
   way to obtain an `EventMetadata` is `EventDispatcher::metrics()`.

These three changes are the entirety of the breakage from `0.9.x`.
Everything else is preserved.

---

## Public surface

### Always available (no feature flag required)

#### Module `mod_events`

```rust
pub mod prelude;            // Convenience re-exports
```

#### Re-exports

```rust
// From core.rs
pub trait Event: Any + Send + Sync + Debug { /* ... */ }
pub struct ListenerId { /* private */ }

// From dispatcher.rs
pub struct EventDispatcher { /* private */ }
impl EventDispatcher {
    pub fn new() -> Self;
    pub fn on<T, F>(&self, listener: F) -> ListenerId
    where T: Event + 'static, F: Fn(&T) + Send + Sync + 'static;
    pub fn subscribe<T, F>(&self, listener: F) -> ListenerId
    where T: Event + 'static, F: Fn(&T) -> Result<(), ListenerError> + Send + Sync + 'static;
    pub fn subscribe_with_priority<T, F>(&self, listener: F, priority: Priority) -> ListenerId
    where T: Event + 'static, F: Fn(&T) -> Result<(), ListenerError> + Send + Sync + 'static;
    pub fn dispatch<T: Event>(&self, event: T) -> DispatchResult;
    pub fn emit<T: Event>(&self, event: T);
    pub fn unsubscribe(&self, listener_id: ListenerId) -> bool;
    pub fn listener_count<T: Event + 'static>(&self) -> usize;
    pub fn metrics(&self) -> HashMap<TypeId, EventMetadata>;
    pub fn clear(&self);
    pub fn clear_middleware(&self);
    pub fn add_middleware<F>(&self, middleware: F)
    where F: Fn(&dyn Event) -> bool + Send + Sync + 'static;
}
impl Default for EventDispatcher;

// From error.rs
pub struct ListenerError(/* private */);
impl ListenerError {
    pub fn new<E>(error: E) -> Self
    where E: Error + Send + Sync + 'static;
    pub fn message<S: Into<String>>(msg: S) -> Self;
    pub fn inner(&self) -> &(dyn Error + Send + Sync + 'static);
    pub fn into_inner(self) -> Box<dyn Error + Send + Sync + 'static>;
}
impl Debug for ListenerError;
impl Display for ListenerError;
impl Error for ListenerError;
impl From<Box<dyn Error + Send + Sync + 'static>> for ListenerError;
impl From<&str> for ListenerError;
impl From<String> for ListenerError;

// From listener.rs
pub trait EventListener<T: Event>: Send + Sync {
    fn handle(&self, event: &T) -> Result<(), ListenerError>;
    fn priority(&self) -> Priority { Priority::Normal }
}

// From metrics.rs
#[non_exhaustive]
pub struct EventMetadata {
    pub event_name: &'static str,
    pub type_id: TypeId,
    pub last_dispatch: Instant,
    pub dispatch_count: u64,
    pub listener_count: usize,
}
impl EventMetadata {
    pub fn time_since_last_dispatch(&self) -> Duration;
}
impl Debug for EventMetadata;
impl Clone for EventMetadata;

// From priority.rs
#[non_exhaustive]
pub enum Priority {
    Lowest = 0,
    Low = 25,
    #[default] Normal = 50,
    High = 75,
    Highest = 100,
    Critical = 125,
}
impl Priority {
    pub fn all() -> &'static [Priority];
}
impl Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default for Priority;

// From result.rs
pub struct DispatchResult { /* private */ }
#[must_use = "..."]
impl DispatchResult {
    pub fn is_blocked(&self) -> bool;
    pub fn listener_count(&self) -> usize;
    pub fn success_count(&self) -> usize;
    pub fn error_count(&self) -> usize;
    pub fn errors(&self) -> &[ListenerError];
    pub fn all_succeeded(&self) -> bool;
    pub fn has_errors(&self) -> bool;
}
impl Debug for DispatchResult;
```

#### `prelude`

```rust
pub mod prelude {
    pub use crate::{Event, EventDispatcher, ListenerError, Priority};
    #[cfg(feature = "async")]
    pub use crate::AsyncEventListener;
}
```

### `async` feature (default)

```rust
// From async_support.rs
pub type AsyncEventResult<'a> =
    Pin<Box<dyn Future<Output = Result<(), ListenerError>> + Send + 'a>>;

pub trait AsyncEventListener<T: Event>: Send + Sync {
    fn handle<'a>(&'a self, event: &'a T) -> AsyncEventResult<'a>;
    fn priority(&self) -> Priority { Priority::Normal }
}

// Additional methods on EventDispatcher under `cfg(feature = "async")`:
impl EventDispatcher {
    pub fn subscribe_async<T, F, Fut>(&self, listener: F) -> ListenerId
    where T: Event + 'static,
          F: Fn(&T) -> Fut + Send + Sync + 'static,
          Fut: Future<Output = Result<(), ListenerError>> + Send + 'static;

    pub fn subscribe_async_with_priority<T, F, Fut>(&self, listener: F, priority: Priority) -> ListenerId
    where T: Event + 'static,
          F: Fn(&T) -> Fut + Send + Sync + 'static,
          Fut: Future<Output = Result<(), ListenerError>> + Send + 'static;

    pub async fn dispatch_async<T: Event>(&self, event: T) -> DispatchResult;
}
```

---

## Trait implementations

| Type | `Debug` | `Clone` | `Copy` | `Default` | `PartialEq` | `Eq` | `Hash` | `PartialOrd` | `Ord` | Other |
|------|---------|---------|--------|-----------|-------------|------|--------|--------------|-------|-------|
| `EventDispatcher` | – | – | – | ✓ | – | – | – | – | – | – |
| `Event` (trait bound) | required | – | – | – | – | – | – | – | – | `Any + Send + Sync` |
| `ListenerId` | ✓ | ✓ | ✓ | – | ✓ | ✓ | ✓ | – | – | – |
| `ListenerError` | ✓ | – | – | – | – | – | – | – | – | `Error + Display` |
| `Priority` | ✓ | ✓ | ✓ | ✓ (`Normal`) | ✓ | ✓ | – | ✓ | ✓ | – |
| `EventMetadata` | ✓ | ✓ | – | – | – | – | – | – | – | – |
| `DispatchResult` | ✓ | – | – | – | – | – | – | – | – | `#[must_use]` |
| `AsyncEventResult<'a>` | – | – | – | – | – | – | – | – | – | type alias |

Adding new auto-derived impls (e.g. `PartialEq` on `EventMetadata`) is a minor-version addition. Removing any of these is `2.0.0`.

---

## Algorithm-stability commitments

Locked under the `1.x` SemVer contract:

1. **Sync `dispatch` invokes every registered listener for the
   event type in descending-priority order, FIFO within equal
   priority.** Each listener call is wrapped in
   `std::panic::catch_unwind`; a panic becomes a `ListenerError`
   with the message prefix `"listener panicked: "` and the next
   listener still runs.

2. **`emit` shares the same listener-execution path and panic
   wrapping but skips `DispatchResult` construction.**
   Fire-and-forget. Caller cannot observe per-listener outcomes.

3. **`dispatch_async` awaits listeners sequentially in
   descending-priority order**, FIFO within equal priority. Each
   listener future is wrapped in
   `futures_util::future::FutureExt::catch_unwind`; the panic
   contract matches the sync path. **Concurrent execution is
   intentionally not provided** — it would lose ordering. Users
   wanting concurrent execution drive `spawn` from inside each
   listener.

4. **Middleware blocks dispatch iff any middleware returns
   `false`.** Middleware runs in registration order. A blocking
   middleware short-circuits the chain. `DispatchResult::is_blocked()`
   reflects this.

5. **`listener_count` and `metrics().listener_count` are derived
   from the live registry at observation time** — they cannot
   drift on `unsubscribe` or `clear`. Verified by `loom` model
   checks and concurrent stress tests.

6. **Subscribe is O(n) via `Vec::partition_point` + `Vec::insert`**,
   preserving descending-priority order with FIFO within equal
   priority. The previous full re-sort behaviour is not coming
   back.

7. **`DispatchResult` success-path allocation is zero.** The
   internal `Vec<ListenerError>` stays empty (`Vec::new()` does
   not allocate). Allocation only happens proportional to failure
   count.

---

## Symbols out of 1.0 — explicitly NOT added

These do not exist in 1.0. Adding them later is a minor-version
addition; listed here so they are not assumed present:

- A `tracing` feature with instrumentation hooks. Candidate for
  `1.x` minor.
- `tokio_util::sync::CancellationToken` integration on async
  dispatch. Candidate for `1.x` minor.
- A `tracing` / `log` macro for the dispatcher itself.
- Serde integration for `EventMetadata`.
- Code-coverage tooling integration.
- `no_std` support. Significant work; only if demand surfaces.
- A `wasm32-unknown-unknown` target build.
- Allocation flamegraph review and any architectural changes
  derived from it.
- A "configurable hash algorithm" knob — the internal
  `TypeIdHasher` is fixed.

---

## Internal items (NOT part of the public surface)

These exist in `src/` but are deliberately `pub(crate)` or
crate-private. They may change between any two `1.x` releases
without notice.

- `src/middleware.rs::MiddlewareManager` (was `pub` in `0.9.x`,
  now `pub(crate)`).
- `src/middleware.rs::MiddlewareFunction` (was `pub` in `0.9.x`,
  now `pub(crate)`).
- `src/type_id_map.rs::*` — internal no-op hasher specialised for
  `TypeId`.
- `src/listener.rs::ListenerWrapper`, `ListenerHandler`.
- `src/async_support.rs::AsyncListenerWrapper`,
  `AsyncEventHandler`.
- `src/metrics.rs::EventMetricsCounters`.
- `src/error.rs::panic_payload_to_listener_error`.

---

## Surface totals

| Category | `0.9.x` | `1.0.0` | Δ |
|----------|---------|---------|---|
| Re-exported modules at crate root | 1 (`prelude`) | 1 (`prelude`) | – |
| Public traits | 3 (`Event`, `EventListener`, `AsyncEventListener`) | 3 | – |
| Public structs | 5 (`EventDispatcher`, `ListenerId`, `ListenerError`, `EventMetadata`, `DispatchResult`, `MiddlewareManager`) | 5 (`MiddlewareManager` removed) | -1 |
| Public type aliases | 2 (`MiddlewareFunction`, `AsyncEventResult`) | 1 (`AsyncEventResult`; `MiddlewareFunction` removed) | -1 |
| Public enums | 1 (`Priority`) | 1 (now `#[non_exhaustive]`) | – |
| `#[non_exhaustive]` markers | 0 | 2 (`Priority`, `EventMetadata`) | +2 |
| Total public items | ~12 | ~10 | -2 |

The `1.0` surface is intentionally smaller than `0.9.x` — the two
removed items (`MiddlewareManager`, `MiddlewareFunction`) were
never useful for external callers. Tightening them now means the
SemVer contract does not lock in dead surface.
