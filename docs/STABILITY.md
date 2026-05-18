# mod-events — Stability and SemVer Policy

> This document is the formal stability contract for the `1.x` line.
> It is binding: any release that violates it is a bug.

## Headline guarantee

**Every public symbol in `1.0.0` is locked for the entire `1.x`
line.** Signatures, observable behaviour, and panic-vs-error
semantics cannot change without a major version bump to `2.0.0`.

This applies to:

- Every symbol re-exported by `mod_events::*` and the `prelude`
  module: `EventDispatcher`, `Event`, `EventListener`,
  `AsyncEventListener`, `Priority`, `ListenerId`, `ListenerError`,
  `DispatchResult`, `EventMetadata`, `AsyncEventResult`.
- The trait implementations on those types (`Debug`, `Default`,
  `Clone`, etc.).
- The behavioural contracts documented in each method's rustdoc
  (panic safety, FIFO ordering within equal priority, sequential
  async dispatch, `errors()` returning `&[ListenerError]`, etc.).
- The `async` cargo feature semantics. The default feature set is
  `["async"]`; removing async from the defaults is a major bump.

## Version policy

The `1.x` line follows [Semantic Versioning 2.0.0](https://semver.org/).
The specific rules for this crate:

### Patch releases (`1.x.Y`)

May contain:

- Bug fixes that do not change observable behaviour for any
  documented input.
- Documentation improvements.
- Internal performance work that does not move any public surface.
- Test additions.
- Internal dependency updates (within their compatible range).

May NOT contain:

- New public items.
- Behavioural changes (e.g. changing the panic message format,
  changing which lock primitive is used in a way callers can
  observe through profiling tools).
- MSRV bumps.

### Minor releases (`1.Y.0`)

May contain everything a patch release may contain, plus:

- New public items (functions, methods, types, modules) so long as
  they are pure additions — no signature change to any existing
  symbol.
- New optional cargo features that default to off, or default-on
  features that activate purely additive code paths.
- MSRV bumps. Each MSRV bump is a minor-version bump minimum and
  is called out in the CHANGELOG.
- New variants on `#[non_exhaustive]` types (`Priority`,
  `EventMetadata`).
- New fields on `#[non_exhaustive]` structs (`EventMetadata`).

### Major releases (`2.0.0`)

Required for any change that violates the above. Specifically, any
of the following requires `2.0.0`:

- Removal or rename of any public symbol.
- A signature change to any existing public symbol (including
  making panicking conditions stricter or looser in a
  user-observable way).
- A change to the `async` default-feature status (e.g. moving
  async to a non-default feature).
- A change that breaks 0.x → 1.x → 2.x migration documented in
  `docs/migration.md`.
- Adding a runtime dependency to the `[dependencies]` table beyond
  the two already there (`parking_lot`, optional `futures-util`).
- Removing the `parking_lot` dep in favour of `std::sync::RwLock`
  (which would re-introduce lock poisoning — observable).

## Determinism

`mod-events` is **not deterministic** by design — listener
execution order is fully deterministic (priority then FIFO), but
the dispatcher does not record reproducible internal state. There
is no seed, no replay capability, and no commitment to identical
output across runs.

Determinism guarantees that hold:

- Listener ordering: within a single dispatch, listeners run in
  descending-priority order. Within equal priority, they run in
  FIFO registration order. Both are part of the contract.
- Sequential async dispatch: `dispatch_async` awaits listeners one
  at a time in priority order. Concurrent execution would lose
  ordering; if you need it, spawn from inside each listener.

Non-determinism that is acceptable:

- The `EventMetadata::last_dispatch: Instant` field. Time is not
  reproducible.
- The exact contents of a panic-payload-derived `ListenerError`
  message ("listener panicked: ..."). The prefix is part of the
  contract; the suffix is whatever the panic carried.

## Panic safety

Both `dispatch` and `dispatch_async` wrap each listener in
`catch_unwind`. A panicking listener becomes a
`ListenerError` with the message prefix `"listener panicked: "`
and subsequent listeners still run. This is part of the public
contract and cannot weaken in the `1.x` line.

`emit` shares the same `catch_unwind` wrapping (panics from
fire-and-forget listeners are caught and dropped, never propagated
to the caller). This is also locked.

The dispatcher itself never panics through normal use. It does not
panic on:

- Unknown `ListenerId` passed to `unsubscribe` (returns `false`).
- Empty event-type registry (returns immediately).
- Middleware returning `false` (returns `DispatchResult::is_blocked()`).
- Lock acquisition (the `parking_lot` primitive does not poison).

## Deprecation policy

A symbol marked `#[deprecated]` in a `1.x.Y` release:

- Remains callable for the entire `1.x` line.
- Continues to behave per its documented contract.
- May only be removed in a `2.0.0` release.

Deprecation is for guidance, not removal. Code that compiles
against `1.0.0` must continue to compile against `1.x.LATEST`
(modulo the deprecation warning), without source changes.

## What is NOT covered

The stability contract does NOT cover:

- **Internal performance characteristics.** A `1.x.Y` may make any
  operation faster or slower than `1.0.0`, so long as it stays
  within the spirit of the documented performance properties
  ("sub-microsecond dispatch", "zero-allocation success path",
  etc.). Documented benchmark numbers in the README and
  `docs/performance.md` are illustrative, not contractual.
- **Internal types and modules.** Any item not re-exported through
  `lib.rs` (`type_id_map::*`, `middleware::*`, the `pub(crate)`
  guts of `metrics.rs` and `listener.rs`) is internal and may
  move or change between minor releases.
- **Exact wording of error messages and panic-payload messages.**
  The error kind, the `Display` chain via `Error::source`, and
  the `"listener panicked: "` prefix are part of the contract;
  the human-readable text following them is not.
- **The `TypeId` hashing implementation.** `mod-events` uses an
  internal no-op hasher specialised for `TypeId`. This is an
  implementation detail and may change.

## Dependencies

Two runtime dependencies are sealed for the `1.x` line:

- `parking_lot` (exact-pinned to `=0.12.4`) — hot-path lock
  primitive. The exact pin per REPS §Dependency Management for
  critical crates.
- `futures-util` (optional, gated on the `async` feature) — used
  only for `FutureExt::catch_unwind` on the async dispatch path.

Adding any third runtime dependency requires a `2.0.0` bump.
Removing either requires a `2.0.0` bump.

## MSRV

`1.0.0` ships with MSRV `1.81.0`. Any change to the MSRV is a
minor-version bump minimum and is called out in the CHANGELOG.

The `1.81` floor is deliberate: Cargo lockfile format v4 (default
since Rust 1.78) cannot be parsed by older toolchains, and the
`0.2.x` MSRV CI job on `1.75` failed at lockfile parse before any
code compiled. `1.81` is the conservative choice — stable since
2024-09-05, well over a year of release window.

## Feature flags

The default feature set is `["async"]`. The `async` feature pulls
in `futures-util` and enables the async dispatch surface
(`AsyncEventListener`, `dispatch_async`, `subscribe_async`,
`subscribe_async_with_priority`, `AsyncEventResult`).

Removing `async` from defaults, or adding a new default feature,
requires a major version bump.

Adding a non-default feature is a minor bump. Renaming a feature
is a major bump.

## Reporting a stability break

If you encounter what looks like a `1.x` stability break:

1. Run the failing case against the latest patch release of `1.x`
   to confirm reproducibility.
2. Capture the exact API call sequence and observed-vs-expected
   behaviour.
3. Open an issue at
   <https://github.com/jamesgober/mod-events/issues> with the
   repro.

Stability breaks are bugs and are fixed in the next patch
release.
