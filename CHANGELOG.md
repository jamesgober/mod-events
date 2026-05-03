# Changelog

All notable changes to `mod-events` are recorded in this file.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-05-03

First stable release. Brings the dispatcher in line with the project's REPS engineering standards: typed listener errors, lock-free metrics on the dispatch hot path, no `unwrap()` in library code, full lint coverage, MSRV pinned to 1.75, cross-OS CI, and `loom` model checks for the only double-checked-locking pattern in the crate.

### Added
- `REPS.md` — Rust Efficiency and Performance Standards. Defines the engineering rules every change in this crate is audited against (performance, concurrency, security, error handling, code quality, testing, documentation, dependency management, observability, CI/CD).
- `ListenerError`, a public newtype that wraps any error a listener returns. Constructable via `ListenerError::new(err)`, `ListenerError::message("…")`, or `Into` from `&str`, `String`, and `Box<dyn Error + Send + Sync>`. Re-exported through `prelude`.
- `parking_lot` runtime dependency, used for the dispatcher's listener, metrics, and middleware locks, plus a per-event-type `Mutex<Instant>` for the last-dispatch timestamp. Chosen because it does not poison on panic, which removes an entire class of recoverable-error variants from the public API.
- `#[must_use]` annotations on `DispatchResult` and on every accessor that returns a `bool`, count, or borrowed view. Callers that drop these values now get a compiler warning.
- Crate-root `#![deny(...)]` lint block covering `warnings`, `missing_docs`, `unsafe_op_in_unsafe_fn`, `unused_must_use`, `unused_results`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::todo`, `clippy::unimplemented`, `clippy::print_stdout`, `clippy::print_stderr`, `clippy::dbg_macro`, `clippy::unreachable`, `clippy::undocumented_unsafe_blocks`, and `clippy::missing_safety_doc`. Library code that drifts from the REPS standards now fails the build.
- Runnable `# Examples` blocks on `subscribe_async`, `subscribe_async_with_priority`, `dispatch_async`, `unsubscribe`, `listener_count`, `metrics`, and `clear`. Doc-test count grew from 12 to 19.
- `rust-toolchain.toml` pinning the channel to `1.95.0` with `rustfmt` and `clippy` components. Every developer and every CI run picks up the same toolchain.
- `rust-version = "1.75"` declared in `Cargo.toml`. Conservative MSRV; CI verifies the crate still builds on 1.75.0 in a dedicated job.
- `.github/workflows/ci.yml` — matrix CI (`ubuntu-latest`, `macos-latest`, `windows-latest`) running `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo build --all-features`, `cargo test --all-features`, and `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS=-D warnings`. Separate jobs run the MSRV build, `cargo audit` against the RustSec database, `cargo deny check all`, and the loom model checks.
- `deny.toml` — license allowlist (Apache-2.0 / MIT / BSD-2/3 / ISC / MPL-2.0 / Unicode / Zlib), wildcards forbidden, multiple-versions warned, only `crates.io` accepted as a registry source.
- `tests/concurrent.rs` — multi-threaded stress tests. Three scenarios cover (1) eight dispatcher threads emitting against eight listeners with no event lost, (2) subscribers and dispatchers churning simultaneously without deadlock or count drift, and (3) `metrics()` snapshot under hot dispatch staying out of the dispatch path's way.
- Edge-case integration tests covering: dispatching with zero listeners, equal-priority FIFO ordering, unsubscribing an unknown id, async listener removal, middleware chain short-circuit ordering, and a 64-listener priority-ordering stress test.
- `tests/loom_concurrent.rs` — `loom` model checks for the dispatcher's double-checked-locking pattern. Two tests prove that `counters_for` (a) inserts the per-event-type counter exactly once under any interleaving of two concurrent first-dispatchers, and (b) returns the same `Arc` to every concurrent caller. Loom does not natively model `parking_lot`, so the tests reproduce the algorithm on `loom::sync::RwLock` and `loom::sync::Arc`. Run with `RUSTFLAGS="--cfg loom" cargo test --release --test loom_concurrent`.
- `loom = "0.7"` added as a dev-dependency. Only compiles when `--cfg loom` is set, so ordinary `cargo test` is unaffected.
- `[lints.rust] unexpected_cfgs = { ..., check-cfg = ['cfg(loom)'] }` in `Cargo.toml` so `cargo clippy --all-targets` recognises the `cfg(loom)` gate on the loom test file.

### Changed
- **Breaking:** every listener-facing handler signature now returns `Result<(), ListenerError>` instead of `Result<(), Box<dyn std::error::Error + Send + Sync>>`. Affects `EventListener::handle`, `AsyncEventListener::handle`, all closure signatures accepted by `subscribe`, `subscribe_with_priority`, `subscribe_async`, `subscribe_async_with_priority`, and the slice returned by `DispatchResult::errors`. Existing code that did `Err("msg".into())` or `Err(my_err.into())` continues to compile via the provided `From` impls.
- **Breaking:** `EventMetadata::dispatch_count` is now `u64` (was `usize`). The counter is backed by an `AtomicU64` on the hot path, so the type matches the live storage.
- **Breaking:** crate version bumped from `0.1.0-beta` (the previous git tag) to `0.2.0`. Per REPS §Versioning, a breaking change in any library crate MUST result in a MAJOR-position bump; for a `0.x` crate that means bumping MINOR.
- Dispatcher locks moved from `std::sync::RwLock` to `parking_lot::RwLock`. Lock acquisition is now infallible at every call site; nineteen `.unwrap()` calls were removed.
- Per-event-type dispatch metrics moved from a single write-locked `HashMap<TypeId, EventMetadata>` to per-type `Arc<EventMetricsCounters>` holding an `AtomicU64` and a tiny `parking_lot::Mutex<Instant>`. The dispatch path no longer takes any write lock on the metrics map (only on first-ever creation of an entry). Snapshotting via `EventDispatcher::metrics` now derives `listener_count` from the live registry, so it can never drift on `unsubscribe` or `clear`.
- Listener registration uses binary insertion (`Vec::partition_point`) instead of `push` + full re-sort. Subscription cost goes from O(n log n) to O(n) per call. Equal-priority listeners still execute in registration order (FIFO).
- `EventDispatcher::emit` documents the deliberate `DispatchResult` discard inline. Callers that need per-listener outcomes use `dispatch`.
- `tokio` runtime dependency tightened from `features = ["full"]` to `default-features = false, features = ["sync"]`. The library never imports `tokio` directly — only the cargo feature wiring needs to resolve. The full runtime stays in `[dev-dependencies]` for examples and tests.
- `parking_lot` is now exact-pinned to `=0.12.4` per REPS §Dependency Management ("Critical crates MUST have exact version pins").
- `Cargo.toml` now carries a `# why:` justification comment above every dependency entry, per REPS §Dependency Management.
- Integration tests renamed to the REPS `test_<subject>_<condition>_<expected>` convention. The two smoke benchmarks under `tests/benchmarks.rs` were renamed similarly.
- `.gitignore` excludes `/.dev/`, so working notes and the internal compliance roadmap stay out of the published tree.

### Removed
- `unsafe impl Send for EventDispatcher` and `unsafe impl Sync for EventDispatcher`. Both auto-derive from the field types; the manual impls had no safety annotation and no behavioral effect.
- `thiserror` runtime dependency. It was declared in `Cargo.toml` but never imported. It will return when the dispatcher grows a real `DispatchError` enum.
- All emoji from `README.md`, `examples/basic_usage.rs`, `examples/async_usage.rs`, `docs/examples.md`, `docs/migration.md`, and `docs/performance.md`. REPS §Documentation forbids emoji in technical documentation.
- Banned-word usage (`comprehensive`, `robust`, `leverage`) from `README.md`, `docs/api-reference.md`, `docs/best-practices.md`, and `docs/examples.md`. Replaced with concrete descriptions.
- `_`-prefixed unused fields from `examples/basic_usage.rs`, `benches/dispatch_benchmark.rs`, and `tests/benchmarks.rs` (`_timestamp`, `_user_id`, `_id`, `_data`). Each field now carries real meaning or has been removed. Bench listeners explicitly touch the payload via `black_box` so the access cannot be optimized away.

### Fixed
- Two `clippy::unnecessary_sort_by` errors on listener registration. Sort now uses `sort_by_key(|w| std::cmp::Reverse(w.priority))`.
- Lock-drop dance in `subscribe_with_priority` and `subscribe_async_with_priority` is no longer racy: the registry write lock is fully released before the metrics path is entered.
- Listener-count drift on `unsubscribe` and `clear`. Previously the metric was a stored counter that subscribe paths updated and unsubscribe forgot to decrement; now it is derived from the live registry at snapshot time and cannot disagree with reality. A new concurrent stress test guards against regression.
- `README.md` documentation links no longer pin commit SHA `da3d757…`; they now use repo-relative paths so docs.rs and the GitHub repo render correctly as the tree evolves.
- `README.md` License heading no longer contains an emoji.

## [0.1.0-beta] — 2025-07-05

Initial public preview of the event dispatcher.

### Added
- `EventDispatcher` with synchronous `dispatch`, fire-and-forget `emit`, and (behind the `async` feature) `dispatch_async`.
- `Event` trait with `as_any` / `type_id` / `event_name`, providing type-safe downcasting from a `&dyn Event` to the concrete type.
- Priority levels (`Lowest`, `Low`, `Normal`, `High`, `Highest`, `Critical`) with listeners executed highest-first.
- Closure subscription helpers: `on` for infallible handlers, `subscribe` for fallible handlers returning `Result`, and `subscribe_with_priority` to set the execution order at registration time.
- Async listener support gated by the `async` feature: `subscribe_async`, `subscribe_async_with_priority`, and the `AsyncEventListener` trait.
- `unsubscribe(ListenerId)` for both sync and async listeners, with `clear()` to drop everything at once.
- `DispatchResult` reporting per-listener success/error counts, the `is_blocked()` state when middleware halted dispatch, and a borrowed view of the underlying errors via `errors()`.
- Middleware pipeline (`add_middleware`, `MiddlewareManager`) that can short-circuit dispatch by returning `false`.
- `EventMetadata` with dispatch counts, last-dispatch timestamp, and listener counts, plus a `metrics()` snapshot keyed by `TypeId`.
- `prelude` module re-exporting the common surface (`Event`, `EventDispatcher`, `Priority`, and `AsyncEventListener` when async is enabled).
- Integration tests covering sync and async dispatch, priority ordering, error propagation, middleware blocking, unsubscribe, listener counts, metrics, and clear.
- `criterion` benchmark scaffold under `benches/` (`single_listener`, `multiple_listeners`).
- Example binaries under `examples/`: `basic_usage` and `async_usage`.
- Documentation set under `docs/`: quick-start, API reference, performance notes, examples, best practices, and migration guide.

### Notes
- Default features: `async`. Disable with `default-features = false` if you need a sync-only build.
- MSRV is unspecified for this preview; see the roadmap for the planned pin.

[Unreleased]: https://github.com/jamesgober/mod-events/compare/0.2.0...HEAD
[0.2.0]: https://github.com/jamesgober/mod-events/compare/0.1.0-beta...0.2.0
[0.1.0-beta]: https://github.com/jamesgober/mod-events/releases/tag/0.1.0-beta
