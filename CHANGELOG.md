# Changelog

All notable changes to `mod-events` are recorded in this file.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **CI: musl target no longer fails at clippy with `error[E0463]: can't find crate for 'core'`.** The `dtolnay/rust-toolchain@stable` action installs the stable channel, but `rust-toolchain.toml` redirects cargo to `1.95.0`; targets installed via the action's `targets:` parameter were attached to stable, not to the channel that cargo actually used. Targets are now added via an explicit `rustup target add` step that runs in the workspace, where rustup honours `rust-toolchain.toml`. Affects the `check / ubuntu-x86_64-musl` job in CI; the local development experience is unchanged.

### Changed
- Dependabot's `github-actions` ecosystem now groups minor + patch updates from `actions/*` (the GitHub-published actions) and from third-party publishers separately. Major-version bumps still get their own PR per action so breaking changes are not hidden in a group. Cuts the per-cycle PR count from one-per-action to roughly two.
- Dependabot now ignores `dtolnay/rust-toolchain` updates. The action uses Rust version numbers as tags (e.g. `@1.81.0` in the `msrv` CI job), and Dependabot mistakes those for action versions, opening PRs to bump them to non-existent Rust versions (it tried `@1.100.0`). The MSRV pin must track `Cargo.toml`'s `rust-version` field, never auto-bump.
- `Cargo.toml` `exclude` list narrowed: examples, benches, and docs now ship in the published tarball. They are useful for vendoring users and are rendered by crates.io / docs.rs. The previous broader exclusion triggered `cargo publish` warnings (`ignoring example/benchmark, not in published package`) because `[[bench]]` was declared but the benches directory was excluded. Tarball grew from ~50 KiB to ~92 KiB compressed — irrelevant for download but the warnings are gone.

## [0.9.0] — 2026-05-03

Release-candidate-style minor for `1.0`. Re-engineered the dispatcher around three goals: panic safety on every path, zero-allocation success path, and runtime-agnostic async. Verified with a property-test suite, an expanded cross-platform CI matrix (Linux x86_64 + ARM64 + musl, macOS ARM64, Windows x86_64), nightly AddressSanitizer, and `loom` model checking of the only non-trivial concurrency pattern in the crate. See the full release notes in [`docs/release/v0.9.0.md`](docs/release/v0.9.0.md).

### Added
- **Async path panic safety.** `dispatch_async` now wraps each listener future in `FutureExt::catch_unwind`. A panicking listener future becomes a `ListenerError` in `DispatchResult::errors()` (prefix `"listener panicked: "`), mirroring the sync dispatch contract. Subsequent listeners still run; the dispatcher remains usable. New test `test_dispatch_async_with_panicking_listener_collects_error_and_continues`.
- **Sync path panic safety.** `dispatch` and `emit` wrap each listener call in `std::panic::catch_unwind`. Same contract as the async path. New test `test_dispatch_with_panicking_listener_collects_error_and_continues`.
- **Property test suite** ([tests/property_tests.rs](tests/property_tests.rs)) using `proptest`. Six properties cover dispatch invokes-every-listener-exactly-once, priority-then-FIFO ordering, subscribe/unsubscribe count invariants, dispatch-count metric consistency, middleware blocks-iff-any-false, and clear drops every listener. Each property runs ~256 randomised cases per CI run.
- **`EventDispatcher::clear_middleware()`** — drops every registered middleware. Counterpart to `clear()` (listeners only).
- **`futures-util` dependency** (optional, gated on the `async` feature) for `FutureExt::catch_unwind`. Pulled in instead of writing a hand-rolled `CatchUnwind` future to keep the crate's `unsafe` surface at zero.
- **Cross-platform CI matrix expanded** to five host/target combinations: `ubuntu-latest` x86_64-gnu, `ubuntu-24.04-arm` aarch64-gnu, `ubuntu-latest` x86_64-musl, `macos-latest` aarch64, `windows-latest` x86_64-msvc. Every combination runs fmt + clippy + build + test + doc.
- **AddressSanitizer CI job** on Linux nightly per REPS §Memory Debugging. Catches use-after-free, double-free, and buffer-overflow that the regular tests miss.
- **Criterion bench job** in CI uploads the report as a build artifact (30-day retention). Persistent baseline tracking + 5%-regression-gate is the next iteration.
- **[`docs/architecture.md`](docs/architecture.md)** — design rationale. Why `parking_lot`, why `Arc<EventMetricsCounters>`, why `partition_point`, why sequential async, why `catch_unwind` on both paths, why no `DashMap` yet, why no per-listener timeout, why runtime-agnostic. Documents the lazy errors Vec, the metrics fast path, the test architecture, the memory profile, and the `0.x → 1.0` API stability promise.
- **[`docs/comparison.md`](docs/comparison.md)** — when to choose `mod-events` vs `tokio::sync::broadcast`, `event-listener`, `bus`, `crossbeam-channel`, plain function calls, or hand-rolled `Vec<Box<dyn Fn>>`. Replaces unverified "Nx faster than Redis/Kafka" claims with honest qualitative comparisons.
- **Dependabot** ([.github/dependabot.yml](.github/dependabot.yml)) for the `cargo` and `github-actions` ecosystems. Weekly grouped updates, immediate ungrouped security PRs.
- **Governance files**: [SECURITY.md](SECURITY.md) (vulnerability disclosure policy), [CONTRIBUTING.md](CONTRIBUTING.md) (contributor onboarding + local CI gate), [RELEASING.md](RELEASING.md) (codified release procedure with recovery), [.github/CODEOWNERS](.github/CODEOWNERS).
- **`Cargo.toml` `exclude`** — published tarball is now library source + README + CHANGELOG + LICENSE + Cargo metadata + tests. Examples, benches, docs, CI config, REPS.md, working notes are stripped.
- Refreshed benchmark numbers in [docs/performance.md](docs/performance.md) against actually-measured 0.9.0 timings (~133 ns single-listener, ~244 ns 10-listener).

### Changed
- **Breaking — runtime-agnostic async, no tokio dep.** The `async` cargo feature no longer pulls in `tokio`. The library never imported tokio anyway; only the cargo feature wiring did. Consumers using the `async` feature with their own runtime (any executor that polls `std::future::Future` — tokio, async-std, smol, embassy, …) see no behavioral change. Consumers who were relying on tokio being a transitive dep of `mod-events` will need to add tokio to their own `[dependencies]`.
- **Breaking — `DispatchResult::errors()` returns `&[ListenerError]`** instead of `Vec<&ListenerError>`. Iteration with `for err in result.errors()` and `for err in result.errors().iter()` is unchanged. Callers that did `.into_iter()` or assigned the return into a `Vec<&_>` will need to adapt.
- **Performance: zero-allocation success path.** `DispatchResult` internal storage is now a `Vec<ListenerError>` (errors only) plus a `usize` listener count, instead of `Vec<Result<(), ListenerError>>` (one entry per listener). On the success path the Vec stays empty and `Vec::new()` does not allocate. A fully successful dispatch of N listeners now performs zero heap allocations for the result; failing dispatches allocate proportional to the failure count, not the listener count.
- **Performance: `emit` skips result building entirely.** The fire-and-forget path no longer constructs a `DispatchResult` it then drops. Same panic-safety contract as `dispatch`; saves one `Vec` allocation per call after the first dispatch of a given event type.
- `EventDispatcher::dispatch_async` documentation updated: confirms `catch_unwind` is now applied to every async listener (was previously documented as caller-responsibility).
- `EventDispatcher::clear` documentation cross-references the new `clear_middleware`.

### Removed
- **`tokio` runtime dependency.** Was always a phantom dep — the library never imported it.
- The "v0.3.0 backlog" stub in `.dev/ROADMAP.md` is superseded by the actual 0.9.0 work landed here.

### Fixed
- The internal `DispatchResult::new(Vec<Result<...>>)` constructor previously allocated a results vector for every dispatch, even on the success path. Now only allocates if at least one listener errors or panics. This is internal but observable as fewer allocations under profiling.

## [0.2.1] — 2026-05-03

Patch release. CI hardening, security advisories cleared, MSRV bumped to keep the committed lockfile parseable. No public API changes from `0.2.0`. See the full release notes in [`docs/release/v0.2.1.md`](docs/release/v0.2.1.md).

### Changed
- **Breaking (build-time only):** MSRV bumped from `1.75` to `1.81`. Cargo lockfile format v4 (default since Rust 1.78) cannot be parsed by older toolchains, and the MSRV CI job for `1.75` failed at lockfile parse before any code compiled. `1.81` is conservative — stable since 2024-09-05.
- `rust-toolchain.toml` left at `1.95.0`. Day-to-day work and the `check` matrix continue to use `1.95.0`; the dedicated `msrv` job verifies `1.81.0`.

### Fixed
- **Security: RUSTSEC-2026-0007** (`bytes`). Pulled patched `bytes 1.11.1` into the committed lockfile (was `1.10.1`). The advisory describes an integer overflow in `BytesMut::reserve` that can corrupt the capacity field and cause out-of-bounds slices. `bytes` is a transitive dependency via `tokio`. Fix lands by `cargo update -p bytes`.
- **Security: RUSTSEC-2025-0047** (`slab`). Pulled patched `slab 0.4.12` into the committed lockfile (was `0.4.10`, also yanked). The advisory describes an out-of-bounds access in `Slab::get_disjoint_mut` due to an incorrect bounds check. Transitive via `tokio`. Fix lands by `cargo update -p slab`.
- CI workflow now declares `permissions: { contents: read, checks: write }`. The `rustsec/audit-check` action posts findings as GitHub Check Runs, which require `checks: write`; without it the action failed with `Resource not accessible by integration`.

### Documentation
- `README.md` Key Features and Error Handling sections updated to mention `ListenerError`, the lock-free metrics path, FIFO equal-priority ordering, and the `loom`-verified concurrency invariants. Install snippet documents the `1.81` MSRV.
- `docs/quick-start.md` install snippet bumped to `0.2.1`; documents the `1.81` MSRV; subscribe example notes the `ListenerError` conversion path.
- `docs/api-reference.md` rewritten to match the 0.2.x signatures: every handler and trait signature uses `Result<(), ListenerError>` instead of `Result<(), Box<dyn Error + Send + Sync>>`; `EventMetadata::dispatch_count` documented as `u64`; `Priority` shown with `#[derive(Default)]`; `AsyncEventResult<'a>` documented as the canonical async-listener return type; new `ListenerError` section under Error Handling; Performance Characteristics rewritten to reflect the lock-free metrics path and binary-insertion subscribe; obsolete `AsyncResult` / `AsyncHandler` aliases removed.
- `docs/best-practices.md` reusable-listener example updated to return `Result<(), ListenerError>` and convert foreign errors via `ListenerError::new`.
- `docs/examples.md` helper signatures updated to return `Result<(), ListenerError>`; the file-write helper converts `io::Error` via `ListenerError::new`.
- `docs/migration.md` adds an "Upgrading from mod-events 0.1.0-beta to 0.2.x" section at the top: dependency bump, MSRV note, listener-error refactor with before/after, async-listener `AsyncEventResult<'a>` example, `EventMetadata::dispatch_count: u64` cast, and a summary of internal performance changes.

## [0.2.0] — 2026-05-03

First stable release. Brings the dispatcher in line with the project's REPS engineering standards: typed listener errors, lock-free metrics on the dispatch hot path, no `unwrap()` in library code, full lint coverage, MSRV pinned to 1.75, cross-OS CI, and `loom` model checks for the only double-checked-locking pattern in the crate. See the full release notes in [`docs/release/v0.2.0.md`](docs/release/v0.2.0.md).

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

[Unreleased]: https://github.com/jamesgober/mod-events/compare/0.9.0...HEAD
[0.9.0]: https://github.com/jamesgober/mod-events/compare/0.2.1...0.9.0
[0.2.1]: https://github.com/jamesgober/mod-events/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/jamesgober/mod-events/compare/0.1.0-beta...0.2.0
[0.1.0-beta]: https://github.com/jamesgober/mod-events/releases/tag/0.1.0-beta
