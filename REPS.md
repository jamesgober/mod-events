<h1 align="center">
    <img width="90px" alt="Rust" src="https://raw.githubusercontent.com/jamesgober/rust-collection/refs/heads/main/assets/rust-logo.svg">
    <br>RUST EFFICIENCY<br>
    <sup><sub>AND PERFORMANCE STANDARDS</sub></sup>
</h1>

**REPS** (**Rust Efficiency** &amp; **Performance Standards**) define the REQUIRED guidelines, standards, and engineering principles for high-performance Rust projects. These standards exist to enforce maximum efficiency, structural security, and long-term reliability. Code that compiles and passes tests but does not meet these standards is not considered complete.

<hr>

The key words "**MUST**", "**MUST NOT**", "**REQUIRED**", "**SHALL**", "**SHALL NOT**", "**SHOULD**", "**SHOULD NOT**", "**RECOMMENDED**",  "**MAY**", and "**OPTIONAL**" in this document are to be interpreted as described in [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

&rarr; view [**definitions**](definitions.md) for details.

<hr>


## Performance

Performance is a hard constraint, not a post-delivery optimization pass. All code MUST be written with the lowest possible overhead from the first implementation. Latency, throughput, CPU cycles, and memory usage are measured against concrete targets on real hardware. Code that satisfies functional requirements but fails performance requirements is not done. Every design decision MUST be evaluated against its runtime cost.

- Zero-copy wherever possible. MUST NOT clone data that can be borrowed.
- MUST minimize heap allocations on hot paths. Prefer stack allocation.
- MUST pool buffers and reuse memory rather than allocating per-request.
- Allocation-free hot paths are the goal, not the exception.
- MUST design for cache locality. Struct layout matters—arrange fields to minimize false sharing and cache misses.
- Use `#[inline]` on small, frequently-called functions. Profile before inlining larger functions.
- MUST benchmark before and after any performance-sensitive change using `criterion`.
- MUST NOT guess at performance. Measure it. Use profilers (`perf`, `flamegraph`, `cachegrind`) to identify bottlenecks before optimizing.
- SHOULD avoid unnecessary abstractions that add overhead. Custom code is often faster than generic libraries.
- SHOULD prefer `no_std` where possible to avoid the overhead of the Rust standard library.
- Use `Cow<'a, T>` in APIs that sometimes require owned data and sometimes do not, to avoid unconditional allocation.
- Evaluate computation-heavy hot paths for SIMD acceleration. Use `std::arch` intrinsics with `#[cfg(target_feature = "...")]` guards for platform-specific paths.

<br>

## Concurrency & Async

Where the project supports concurrency, it MUST be engineered for high-contention workloads from the ground up. Thread safety is REQUIRED for all shared types—it is not optional and cannot be retrofitted. Lock-free approaches SHOULD be used wherever correctness can be proven. Single-writer bottlenecks are an architectural defect.

Where the project supports async I/O, async MUST be a first-class architectural decision, not an incremental addition. The async executor is a shared, finite resource. Blocking it—for any reason—degrades the entire system under load. Blocking I/O within an async context is PROHIBITED.

- MUST use `tokio` for async runtime.
- MUST NOT block async contexts under any circumstance. Use `tokio::task::spawn_blocking` to offload blocking operations to a dedicated thread pool.
- MUST prefer message-passing (`tokio::sync::mpsc`, `broadcast`) over shared state.
- When shared state is unavoidable, MUST use `Arc<RwLock<T>>` and keep lock scope as narrow as possible.
- MUST prefer `RwLock` over `Mutex` when reads dominate. Profile lock contention to confirm the choice.
- MUST use `tokio::sync::Semaphore` for bounded concurrency. Unbounded async task spawning is PROHIBITED.
- All async functions MUST respect cancellation. Accept and propagate cancellation signals via `CancellationToken` or `tokio::select!`.
- MUST NOT use `std::sync::Mutex` in async code. It will block the executor thread.
- Thread-safe by default for any type intended to be shared across tasks. MUST use `Send + Sync` bounds on shared types.
- MUST NOT introduce single-writer bottlenecks. Use sharding, partitioning, or lock-free structures (`DashMap`, `crossbeam`) for concurrent access.
- MUST NOT use `Arc` where `Rc` suffices. Atomic reference counting has overhead. If a type is strictly single-threaded, use `Rc<T>`.

<br>

## Security

Security is a structural requirement, not a release-phase concern. All external input MUST be treated as hostile until explicitly validated. Least-privilege access MUST govern every component boundary—no component SHALL have access beyond what its function strictly requires. Sensitive data MUST be encrypted at rest and in transit without exception. A system that functions correctly but leaks data, permits privilege escalation, or fails a security audit does not meet these standards.

- MUST validate and sanitize every input at the system boundary before it touches any internal structure. Reject before accepting—never accept and correct after the fact.
- MUST NOT allow secrets, tokens, credentials, or PII to appear in logs, error messages, panics, or stack traces. Implement redaction filters at the logging boundary.
- MUST use constant-time comparison for sensitive values (passwords, tokens, keys). Use `subtle::ConstantTimeComparison` or equivalent. Direct `==` on secrets is PROHIBITED.
- MUST encrypt all sensitive data at rest and in transit. Plaintext sensitive data anywhere in persistent storage or on the wire is PROHIBITED.
- When encryption is required, MUST use AEAD constructions (ChaCha20-Poly1305 or AES-GCM). Raw symmetric encryption without an authentication tag is PROHIBITED—it provides confidentiality but not integrity.
- Secrets and cryptographic key material MUST be zeroed from memory after use. Use the `zeroize` crate (`Zeroize` trait, `ZeroizeOnDrop` derive) for all types holding sensitive data. Dropping a secret without zeroing leaves it recoverable from memory.
- MUST use checked, saturating, or wrapping arithmetic explicitly for any security-sensitive or safety-critical integer operations. In release mode, Rust integer overflow wraps silently. For untrusted input, index calculations, size arithmetic, or crypto, this is a vulnerability. Use `checked_add`, `checked_mul`, `saturating_*`, or `wrapping_*` as appropriate.
- MUST maintain audit trails for all privileged operations. Log who did what and when—without including sensitive content.
- MUST keep dependencies up to date. Run `cargo audit` (RustSec advisory database) and `cargo deny check` in CI. Both tools serve distinct purposes and both are REQUIRED.
- `unsafe` code MUST have explicit safety justification. Use `#[deny(unsafe_op_in_unsafe_fn)]` to enforce that every `unsafe` operation inside an `unsafe fn` requires its own `unsafe {}` block with a `// SAFETY:` annotation.
- MUST conduct regular security reviews. All findings MUST be addressed before the next release.

<br>

## Architecture

Architecture MUST be designed for long-term maintainability and minimal coupling. Every module, type, and function MUST have a single, clear responsibility. Inter-module dependencies MUST flow through explicit abstractions—concrete implementations are internal details, never dependencies. Code that cannot be unit tested without external services or global state has a structural problem. Observability is not a feature to be added later; it is a REQUIRED property of the system from the first component.

- MUST follow SOLID principles.
- MUST follow KISS—the simplest correct solution wins.
- MUST follow DRY—but MUST NOT abstract until the pattern has been proven at least twice.
- MUST follow YAGNI—build what is needed now. Design for extension, not speculation.
- Modules MUST have a single, focused responsibility. One responsibility per module, per type, per function.
- MUST use trait-driven design. Depend on abstractions, not concrete implementations.
- MUST NOT create god structs or god modules.
- MUST use dependency injection through constructors and trait bounds. Global state is PROHIBITED unless genuinely unavoidable, in which case it MUST be explicitly documented and thread-safe.
- Internal implementation details MUST stay internal. Use `pub(crate)` and module visibility aggressively.
- MUST design for testability. No code path SHOULD require an external service or global state to execute a unit test.
- MUST design for observability. Include hooks for logging, metrics, and tracing from the first component. No black boxes.
- MUST use `#[cfg(test)]` to include test-only code. Test scaffolding MUST NOT pollute the production binary.

<br>

## Error Handling

In Rust, errors are values—not exceptions, not panics, and not silently discarded return codes. Every fallible operation MUST return `Result<T, E>`. Library code MUST NOT panic under any recoverable condition. Errors MUST carry sufficient context to identify the failure, locate its origin, and determine the correct recovery path. An error message that only says "failed" is a debug obstruction, not an error type.

- MUST use `Result<T, E>` for all fallible operations. Panics in library code are PROHIBITED. Panics are only acceptable in `main()` or binary entry points during unrecoverable boot failures.
- MUST define domain-specific error types per module. Using `Box<dyn Error>` in internal APIs is PROHIBITED.
- MUST define custom error types using established crates (`thiserror` for libraries, `anyhow` at binary boundaries). `anyhow` MUST NOT be used in library crates.
- MUST use `#[must_use]` on any return type whose value being silently dropped is almost certainly a programmer error. Annotate own types and fallible handles accordingly.
- Errors MUST carry enough context to be actionable: what was attempted, why it failed, and what the caller can do about it. No bare "failed" messages.
- MUST NOT silently swallow an error. Every error MUST be logged, propagated, or handled deliberately. `let _ = fallible_op()` is PROHIBITED without a documented justification.
- Edge cases MUST be considered and handled during the initial implementation pass, not deferred.
- MUST use `#[deny(unused_must_use)]` and `#[deny(unused_results)]` to enforce handling of all `Result`s.
- All error types and their variants MUST be documented. Documentation MUST explain what each variant means and what the caller SHOULD do when they encounter it.
- All error paths MUST be tested, including edge cases and failure modes.
- Logging of errors MUST include sufficient context for diagnosis without exposing sensitive information.

<br>

## Code Quality

Code quality is enforced mechanically, not aspirationally. All code MUST compile without warnings. All code MUST pass `clippy` at the configured strict profile. Formatting MUST be enforced by `rustfmt` with zero tolerance for deviations. Every commit to the repository is treated as production-ready. There is no "clean up later"—if it is not ready, it does not get committed.

**Compiler & Lint Configuration:**

All crates MUST include the following at the crate root:

```rust
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::unreachable)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::undocumented_unsafe_functions)]
```

Use `#[forbid(...)]` instead of `#[deny(...)]` for lints that downstream crates MUST NOT override. `#[forbid]` is the stronger form—it prevents any downstream or inner scope from suppressing the lint. Reserve `#[deny]` for lints where a documented, scoped `#[allow]` is occasionally justified.

Test modules MAY use `#[allow(clippy::unwrap_used)]` at the module level only when unwrapping is genuinely unavoidable (e.g., test setup code where an error cannot be meaningfully handled).

**Code Standards:**

- Every commit is production-ready. There is no "clean up later."
- MUST run `clippy` with a strict profile. Fix every lint, or provide a scoped `#[allow]` with a documented justification.
- MUST format with `rustfmt`. No exceptions.
- Names MUST be self-documenting. If a name requires a comment to explain it, rename it.
- Functions MUST be short and focused. If a function does not fit on a screen, it is doing too much.
- MUST NOT use `unwrap()` or `expect()` in library code. In binary crates, these are only acceptable at startup where a failure is genuinely unrecoverable, and MUST be documented.
- MUST NOT commit commented-out code. Delete it or track it in a ticket.
- MUST NOT commit `todo!()` or `unimplemented!()` without a linked issue and documented rationale.
- MUST annotate return types with `#[must_use]` when silently dropping the returned value is almost certainly a bug.

<br>

## Unsafe Code

`unsafe` is not forbidden, but it carries absolute responsibility. Every use of `unsafe` is a claim that the author has verified all invariants that the compiler cannot. That claim MUST be documented, justified, and tested. Undocumented `unsafe` code will not be accepted.

- Every `unsafe` block MUST be preceded by a `// SAFETY:` comment that explains, specifically, why the operation is sound: what invariants hold, what the code assumes, and what would cause undefined behavior if violated.
- `unsafe fn` MUST document in the function's doc comment which invariants the caller MUST uphold. The `# Safety` doc section is REQUIRED on all `unsafe fn`.
- The scope of every `unsafe` block MUST be as narrow as possible. Do not wrap large sections of code in a single `unsafe` block to avoid thinking about each operation individually.
- MUST use `unsafe {}` blocks inside `unsafe fn` rather than relying on the implicit unsafety of the function. `#![deny(unsafe_op_in_unsafe_fn)]` enforces this.
- MUST NOT use `unsafe` to bypass the borrow checker when a correct safe design exists. If you are reaching for `unsafe` to silence a lifetime error, reconsider the ownership model first.
- MUST NOT dereference raw pointers without proving non-null and valid alignment. Document the proof in the `// SAFETY:` comment.
- All `unsafe` code MUST have targeted tests that exercise the invariants being relied upon, including out-of-bounds, null, and aliasing scenarios where applicable.
- `unsafe` code that relies on a specific platform, architecture, or CPU feature MUST use appropriate `#[cfg(...)]` guards.

<br>

## Testing

Tests are the contract between implementation and requirements. All logic MUST have unit test coverage. All I/O boundaries and cross-module interactions MUST have integration tests. A failing test on any commit MUST block the build without exception. Test coverage targets exist to prevent regressions—not to satisfy a metric. Tests MUST cover failure paths, boundary conditions, and concurrent access scenarios, not just the expected happy path.

- Unit tests MUST cover all logic. Integration tests MUST cover all I/O boundaries and cross-module interactions.
- Unit tests MUST live in a `#[cfg(test)]` module at the bottom of each source file. Integration tests MUST live in `tests/`.
- Test naming convention: `test_<subject>_<condition>_<expected>`. Example: `test_buffer_push_at_capacity_returns_err`.
- Edge cases MUST be tested, not assumed. At minimum: empty input, maximum input, concurrent access, failure mid-operation, partial writes, and corrupted data.
- Benchmarks MUST live in `benches/` using `criterion`. Every performance-sensitive code path MUST have a benchmark. Baselines MUST be tracked over time.
- A failing test on any commit MUST block the build.
- Test coverage MUST prioritize behavioral coverage and edge cases over line-count metrics.
- Concurrent code MUST be tested with `loom` for deterministic race condition detection where feasible.
- Algorithms that SHOULD hold for all inputs MUST be covered by property-based tests using `proptest`.
- Security-relevant behavior MUST be explicitly tested: input rejection at boundaries, authentication failures, authorization denials, and sanitization correctness.

<br>

## Documentation

Documentation is a deliverable, not a finishing step. Every public type, trait, function, and constant MUST have a doc comment before the implementation is considered complete. Doc comments MUST explain behavior and intent—not restate what the type signature already conveys. Documentation MUST be written by a human for a human. AI-generated filler, marketing language, and structural boilerplate have no place in technical documentation.

- Every public type, trait, function, and constant MUST have a doc comment.
- Doc comments MUST explain the *why* and the *behavior*—not just the *what*. The signature describes the shape. The doc comment explains the contract: what it does, when to use it, and when not to.
- MUST include `# Examples` with runnable `\`\`\`rust` code blocks for any non-trivial public API.
- MUST use `#[deny(missing_docs)]` to enforce documentation on all public items.
- Documentation MUST read as written by a human. Do not use AI-style patterns:
  - No "Phase X" or "Step Y" section headers
  - No "Placeholder", "Simplified", or "TODO: implement" comments
  - No emoji in documentation
  - No "comprehensive", "robust", "seamless", or "leverage"
  - No verification reports or certification blocks
- Commit messages MUST be imperative, lowercase, concise, with no trailing period. Example: `add error context to parser` — not `Added Exception Context Handling`.
- Public-facing docs in `docs/` MUST be written for developers integrating with or extending the system. They explain concepts, architecture, and design decisions—not just the API surface.
- Internal source-level docs MUST explain rationale and constraints, not just code flow.
- All `unsafe` code MUST have a `# Safety` section in its doc comment, and a `// SAFETY:` comment on every `unsafe {}` block explaining the invariants being relied upon.

<br>

## Dependency Management

Every dependency is a liability. Adding a crate imports its entire maintenance history, security posture, transitive dependency graph, and license obligations into the project. No dependency SHALL be added without explicit justification against all of those factors. When functionality can be implemented locally at reasonable cost, it SHOULD be. The default answer to adding a new dependency is no—the requester bears the burden of justification.

- Every dependency MUST justify itself. If it can be implemented locally in a reasonable scope, it SHOULD be.
- Before adding any third-party crate, the following MUST be evaluated:
  - Active maintenance status and security track record
  - Alignment with project values (no unsound code; minimal allocations for performance-critical use)
  - MSRV (Minimum Supported Rust Version) compatibility
  - Quality and scope of transitive dependencies
  - License compatibility
- Transitive dependencies MUST be audited. MUST use `cargo deny` to enforce policies on licenses, banned crates, and duplicate versions.
- MUST run `cargo audit` in CI to check against the RustSec advisory database for known vulnerabilities in the dependency tree.
- Critical crates MUST have exact version pins. Less critical, stable crates MAY use compatible version ranges (`^`, `~`) with justification.
- Feature flags MUST be additive only. A feature flag MUST NOT remove or disable required functionality.
- The reason for every significant dependency MUST be documented in a comment in `Cargo.toml`.
- Optional dependencies MUST NOT be added to `Cargo.toml` without a corresponding feature flag and gated code paths.

<br>

## Workspace & Project Structure

Workspace structure MUST enforce domain boundaries—not merely organize files. Each crate is a unit of ownership, responsibility, and visibility. Coupling between crates MUST flow through declared traits and interfaces. Direct access to another crate's concrete internals is PROHIBITED. A crate's public API is a contract; anything not in that contract is an internal detail and SHALL NOT be depended on externally.

```
project-root/
├── crates/
│   ├── core/           # Shared essentials — cross-cutting types, no business logic
│   ├── <domain-a>/     # Library crate with focused, bounded responsibility
│   ├── <domain-b>/     # Another bounded library crate
│   └── ...
├── bins/               # Binary/executable crates — depends on crates/, not internals
│   ├── <app-a>/
│   └── <app-b>/
├── docs/               # Public documentation
├── CHANGELOG.md        # Keep a Changelog format
└── Cargo.toml          # Workspace root
```

**Visibility Rules:**

- Each crate MUST have a single, clearly documented responsibility. Document it in `Cargo.toml` under `description`.
- MUST use `pub(crate)` aggressively. Minimize the surface of what is publicly exported.
- Coupling between crates MUST flow through trait definitions. Direct calls to concrete internals across crate boundaries are PROHIBITED.
- Binary crates MUST depend only on the stable public API of library crates. They MUST NOT reach into internal modules.
- Circular dependencies between crates are PROHIBITED. Restructure if they arise.
- Each crate SHOULD have a clear owner. Document ownership in a `CODEOWNERS` file.

<br>

## Versioning & Semver

All public crates MUST follow semantic versioning strictly. A breaking change—regardless of how minor it appears—MUST result in a MAJOR version increment. Downstream consumers rely on version signals to make safe upgrade decisions. Violating semver is a trust failure with real consequences for anyone depending on the crate.

- MUST use semantic versioning: `MAJOR.MINOR.PATCH`.
- MAJOR: Breaking API changes, behavioral changes, protocol or schema incompatibilities.
- MINOR: New features, performance improvements, non-breaking additions.
- PATCH: Bug fixes, documentation updates, internal refactors with no behavioral change.
- Pre-release versions MUST use `-alpha`, `-beta`, or `-rc` suffixes as appropriate.
- A breaking change in any library crate MUST result in a MAJOR version increment, regardless of scope. No compatibility shims.
- Deprecations MUST be marked with `#[deprecated(since = "X.Y.Z", note = "use ... instead")]`.
- All deprecated items MUST have a migration path documented. Deprecated items MUST NOT be removed until at least one MAJOR version after being deprecated.

<br>

## Platform-Specific Code

All supported platforms MUST be treated as first-class targets. A codebase that compiles and runs correctly only on the developer's platform is not cross-platform—it is untested everywhere else. Every platform-specific branch MUST have a corresponding test, and no platform MUST be silently degraded without an explicit, documented fallback path.

- MUST use `#[cfg(target_os = "...")]` for OS-specific code. Every platform-specific branch MUST have a corresponding test.
- MUST NOT write bare `#[cfg(unix)]` without a `#[cfg(windows)]` equivalent, and vice versa. Every branch MUST be covered.
- MUST NOT silently fall back to a degraded implementation on an unsupported platform. The fallback MUST be explicit, documented, and log a warning at runtime.
- SHOULD prefer platform abstraction crates (`libc`, `nix` for Unix; `windows-sys` for Windows) over raw FFI where feasible.
- All supported platforms (Linux x86_64/ARM64, macOS x86_64/ARM64, Windows x86_64) MUST compile and pass the full test suite.

<br>

## Observability & Monitoring

A system that cannot be observed cannot be reliably operated or debugged in production. Metrics, structured logs, and trace instrumentation MUST be considered structural requirements—not additions made after deployment reveals problems. Every component MUST expose sufficient telemetry to diagnose failures and measure performance without requiring code changes, restarts, or access to internal state.

- Every module MUST expose metrics. At minimum: operation counts, error rates, latency distributions, and resource utilization.
- MUST use structured logging throughout. Use `tracing` for both logging and distributed trace instrumentation.
- MUST use log levels consistently: `ERROR` for failures requiring immediate attention, `WARN` for degraded or unexpected conditions, `INFO` for significant lifecycle events, `DEBUG` for diagnostic detail, `TRACE` for granular execution flow.
- MUST NOT log secrets, credentials, or sensitive data under any log level. Redaction MUST be enforced at the logging boundary.
- Network-facing components MUST expose health check endpoints returning structured status.
- MUST include request IDs or correlation IDs on all operations that cross service or module boundaries.
- Every metric MUST be documented: what it measures, what normal values look like, and what an anomalous value indicates.
- The benchmark framework MUST track performance baselines over time. Regressions exceeding the defined threshold (see CI/CD) MUST block merges.

<br>

## Failure Modes & Degradation

Failures are inevitable. The system MUST be designed to contain them. A failure in one subsystem MUST NOT propagate unchecked into unrelated subsystems. Critical components MUST remain operational when non-critical ones fail. Graceful degradation is the REQUIRED response to unexpected conditions—a total system shutdown from a partial failure is a design defect.

- MUST identify and document which components are critical vs. non-critical. This classification MUST drive isolation and recovery design.
- Non-critical component failures MUST NOT crash or halt critical components.
- Subsystem isolation MUST be implemented using the bulkhead pattern. Each failure domain MUST be bounded.
- MUST implement circuit breakers for all external dependencies. Fail fast; recover automatically when the dependency recovers.
- MUST implement retry logic with exponential backoff and jitter for transient failures. Retry budgets and timeout limits MUST be documented and configurable.
- Graceful shutdown MUST handle: signal receipt, in-flight request completion, resource cleanup, and lock/file release. No orphaned state after shutdown.
- MUST detect and handle memory pressure. Cache eviction MUST be graceful. OOM panics are PROHIBITED.
- MUST detect when disk capacity is near-exhausted and stop writes before running out of space entirely. Errors MUST surface clearly to the operator.

<br>

## Memory Debugging & Leak Detection

Memory safety guarantees in Rust eliminate a large class of bugs, but they do not prevent leaks, excessive allocation, or fragmentation. Memory profiling SHOULD be performed routinely during development, not reactively after a production incident. Issues that appear in production but were not caught during development represent a gap in the validation process.

- Linux/macOS: MUST use `valgrind` or `heaptrack` regularly during development to detect memory leaks.
- Windows: MUST use `DrMemory` or WinDbg heap analysis tooling for equivalent coverage on Windows targets.
- MUST enable `AddressSanitizer` in CI for all tests on Linux and macOS. Configure strictness via `ASAN_OPTIONS`.
- SHOULD enable `MemorySanitizer` for crates with significant `unsafe` code. Configure via `MSAN_OPTIONS`.
- MUST profile memory usage with allocation flamegraphs (`cargo flamegraph` with `--allocs`). Identify and justify all allocations on hot paths.
- MUST test with reduced memory limits to validate OOM scenarios. Use Linux `cgroups` to impose limits in CI.
- SHOULD use `jemallocator` instead of the system allocator if fragmentation is observed in profiling. Profile and confirm improvement before committing the change.

<br>

## Reproducibility & Build Determinism

Builds MUST be reproducible. Given identical source, toolchain, and build parameters, the output MUST be bitwise identical across environments. Non-deterministic builds obscure supply chain integrity, complicate debugging, and undermine confidence in deployed artifacts. Any build behavior that depends on environment state, system timestamps, or runtime randomness is a defect.

- MUST pin the Rust toolchain version in `rust-toolchain.toml` at the workspace root. Documenting the version in a README is insufficient — it MUST be enforced by tooling.
- MUST NOT embed timestamps in compiled artifacts. Use `SOURCE_DATE_EPOCH` in build scripts to enforce a fixed reference time.
- MUST NOT generate random seeds, UUIDs, or nonces at compile time.
- MUST NOT have environment-dependent behavior in build scripts. All paths MUST be relative or explicitly passed as input.
- `Cargo.lock` MUST be committed for all workspace roots. Dependency versions MUST NOT drift between environments.
- The complete build environment MUST be documented: Rust version, LLVM version, OS, target architecture, relevant environment variables.
- Dockerized or scripted build environments SHOULD be provided for any project where reproducibility is a hard requirement.

<br>

## Git Conventions

Version control history is documentation. Commit messages MUST communicate intent clearly and consistently. A well-maintained history serves as an audit trail, a diagnostic tool, and an onboarding resource for future maintainers. Vague messages, mixed-concern commits, and auto-generated summaries degrade its value for everyone who depends on it.

- Commit messages MUST be written in the imperative mood, lowercase, concise, with no trailing period:
  - `add exponential backoff to retry logic`
  - `fix off-by-one in buffer allocation`
  - `remove unused metrics collection`
- Branch names MUST use the prefixes: `feature/`, `fix/`, `docs/`, `perf/`, `refactor/`
- MUST NOT commit generated files: `target/`, build artifacts, `*.o`, `*.a`, etc. Exception: workspace `Cargo.lock`.
- MUST NOT commit AI artifacts: directive files, phase documents, planning notes, and verification reports. These MUST remain outside version control.
- Every meaningful change MUST have a corresponding CHANGELOG entry under `[Unreleased]`.
- CHANGELOG MUST use proper version linking with dates: `[1.2.0] — 2026-04-24`, with links to diffs.
- Each commit MUST represent a single logical change. Mixing refactoring with feature work in one commit is PROHIBITED.
- MUST NOT bypass commit hooks with `--no-verify`. If a hook fails, fix the underlying issue.

<br>

## Crate Packaging & Publishing

A published crate is a public contract. All metadata MUST be complete and accurate. All public behavior MUST be documented. All breaking changes MUST be accompanied by a migration path and a corresponding MAJOR version increment. Publishing an incomplete, undocumented, or poorly packaged crate is worse than not publishing—it creates a dependency that consumers cannot reliably use or upgrade.

- All shippable crates MUST target minimum Rust stable. MSRV MUST be declared in `Cargo.toml` under `rust-version`.
- MUST include complete crate metadata: `description`, `keywords`, `categories`, `repository`, `license`, `authors`, `homepage`.
- MUST include a `README.md` in each published crate explaining purpose, usage, and examples.
- Feature flags MUST be additive only. A feature flag MUST NOT remove or disable required functionality.
- MUST NOT use `panic!` or `unwrap()` in build scripts. Errors MUST be propagated gracefully via `Result`.
- MUST NOT publish to crates.io without passing the full CI/CD checklist. CHANGELOG release notes MUST match the git tag exactly.

<br>

## CI/CD & Pre-Merge Checklist

No commit SHALL reach the main branch without satisfying every check listed below. These are not suggestions—each item is REQUIRED. Any check that fails MUST block the merge without exception. Bypassing these checks for any reason, including urgency or low perceived risk, is not permitted.

### Required Checks

- [ ] **Compilation**: All crates compile on all supported platforms with zero warnings.
- [ ] **Formatting**: `cargo fmt --all -- --check` passes. Zero deviations.
- [ ] **Linting**: `cargo clippy --all-targets --all-features -- -D warnings` passes. Zero violations.
- [ ] **Unit Tests**: `cargo test --lib` passes on all supported platforms.
- [ ] **Integration Tests**: `cargo test --test '*'` passes on all supported platforms.
- [ ] **Doc Tests**: `cargo test --doc` passes. All documentation examples compile and run correctly.
- [ ] **Security Audit**: `cargo audit` passes. No known vulnerabilities in the dependency tree.
- [ ] **Dependency Policy**: `cargo deny check` passes. No license violations, banned crates, or policy violations.
- [ ] **MSRV**: `cargo +<msrv> build --all` succeeds for all shippable crates.
- [ ] **Documentation**: `cargo doc --no-deps --all-features` generates without warnings. All public items have doc comments.

### Performance Gates

- [ ] **Benchmarks**: Any change touching a performance-sensitive code path MUST include a `cargo bench` run. Results MUST be recorded. A regression exceeding **5% on any tracked metric** MUST block the merge.
- [ ] **Benchmark Baseline**: Baseline values MUST be stored and compared. Regressions are relative to the last passing baseline on `main`, not the developer's local environment.

### Coverage & Quality Gates

- [ ] **Code Coverage**: New and modified code MUST have meaningful behavioral test coverage. Target: >80% on new code. Track trends — declining coverage is a warning signal.
- [ ] **No Panics**: Zero new `panic!`, `unwrap()`, or `expect()` in library code. Documented exceptions in binary startup code only.
- [ ] **No TODOs**: Zero new `todo!()` or `unimplemented!()` without a linked issue.
- [ ] **No Commented Code**: Zero blocks of commented-out code. Delete or track in a ticket.
- [ ] **Error Paths Tested**: All `Result`-returning functions MUST have tests for both the `Ok` and `Err` paths.

### Before Marking Ready

- [ ] **Changelog Updated**: Entry added under `[Unreleased]` documenting the change.
- [ ] **Backward Compatibility**: Breaking changes are intentional, documented, and accompanied by a migration guide.
- [ ] **Commit Quality**: Imperative mood, lowercase, clear description, linked to relevant issues or PRs.
- [ ] **Code Review**: At least one approval from a maintainer with direct knowledge of the affected area.
- [ ] **Merge Strategy**: Feature branches are squashed. Merge commits on main are PROHIBITED.

<br>

## Anti-Patterns: What NOT To Do

The following patterns are explicitly PROHIBITED in REPS-compliant code. Each item describes a practice that either degrades performance, introduces security risk, reduces reliability, or undermines long-term maintainability. These are not style preferences—they are disqualifying defects.

### Performance Anti-Patterns

- **`clone()` in hot paths**: Cloning strings, vectors, or structs inside loops creates allocations on every iteration. Use references and borrows instead.
- **Large stack copies**: Stack frames exceeding ~8–16 KB are risky and may overflow. Use `Box<T>` or pass by reference for large types. Profile with AddressSanitizer.
- **Unbounded async task spawning**: `tokio::spawn()` without bounds results in unbounded memory growth under load. Use semaphores, bounded queues, or task pools.
- **Shared `Arc<Mutex<T>>` under high contention**: This is slower than message-passing at scale. Prefer `tokio::sync::mpsc` or sharded structures (`DashMap`).
- **Per-request allocation**: Allocating fresh buffers for every request, query, or batch is unnecessary overhead. Use object pools or thread-local buffer reuse.
- **`Box<dyn Trait>` in tight loops**: Dynamic dispatch prevents inlining and adds indirection. Use generics or enum dispatch in performance-critical paths.
- **Uninstrumented inlining**: Applying `#[inline]` to large, complex functions without profiling first can bloat binary size and harm cache locality.
- **Synchronous I/O in async code**: A single `std::io::Read` or `std::fs` call blocks the entire async executor thread. Use `tokio::io` or `tokio::task::spawn_blocking`.
- **Claiming "optimized" without measurement**: Performance assertions MUST be backed by `criterion` benchmarks or profiler output. Assertion without measurement is speculation.
- **Unnecessary `.iter().collect::<Vec<_>>()`**: Materializing intermediate collections when the data could be processed lazily wastes allocation. Use iterator chains.
- **`Arc<T>` instead of `Rc<T>` in single-threaded contexts**: Atomic reference counting has measurable overhead. If the type is strictly not shared across threads, use `Rc<T>`.
- **Calling `.to_string()` or `.to_owned()` without necessity**: Allocating an owned `String` from a `&str` for no reason is wasteful. Use `&str` or `Cow<str>` in APIs where ownership is not always needed.

### Security Anti-Patterns

- **Validate after parse**: Input MUST be validated before or during parsing, not after. Accepting first and correcting second is a vulnerability.
- **Raw secrets in logs**: Logging error messages that include passwords, tokens, or keys. Redact at the logging boundary—never pass secrets into log macros.
- **`==` for secret comparison**: Direct equality on secret values is vulnerable to timing side-channels. Use `subtle::ConstantTimeEq` or equivalent.
- **`panic!` on invalid input**: Panicking in response to bad external input exposes internal details and can be used for denial-of-service. Use `Result` with an appropriate error variant.
- **Raw symmetric encryption (no AEAD)**: Encrypting without an authentication tag provides confidentiality but not integrity. Encrypted data can be tampered with silently. Use ChaCha20-Poly1305 or AES-GCM.
- **Secrets left in memory after use**: Dropping a value holding a secret does not zero the memory. An attacker with memory access can recover it. Use `zeroize` on all types that hold secrets.
- **Unchecked arithmetic on untrusted input**: In release mode, Rust integer arithmetic wraps on overflow. Index calculations, size arithmetic, and crypto operations on untrusted values MUST use checked or saturating arithmetic.
- **Hardcoded credentials**: Secrets, API keys, tokens, and encryption keys committed to source control are permanently exposed. Use environment variables or a secrets manager.
- **Unaudited supply chain**: Transitive dependencies are part of the attack surface. Run `cargo audit` and `cargo deny` in CI. Failing to audit is not a neutral choice.

### Correctness & Reliability Anti-Patterns

- **`unwrap()` in library code**: `unwrap()` panics on `Err` or `None`, converting a recoverable error into a crash. Use `Result` and propagate.
- **Silently ignoring errors**: `let _ = fallible_op()` throws the error away. Log it, propagate it, or handle it deliberately. Silent discard is PROHIBITED.
- **Incomplete match arms on error types**: Handling `Ok` but not `Err`, or some `Err` variants but not all, leaves failure modes untested and unhandled.
- **Race conditions through unsynchronized shared state**: Shared mutable state accessed from multiple threads without synchronization is undefined behavior in other languages and a logic bug in Rust if `unsafe` is involved. Use channels, `Arc<Mutex<T>>`, or lock-free structures with documented proof.
- **Off-by-one errors**: Classic boundary bug. MUST test: 0, 1, capacity-1, capacity, capacity+1.
- **Unreachable or dead code paths**: `match` arms that can never execute, or code branches that are logically impossible. Remove them—they create confusion and hide intent.
- **Hardcoded configuration values**: Timeouts, pool sizes, buffer sizes, and thresholds embedded as literals. These MUST be configurable and MUST be tested with non-default values.
- **Assuming format or schema stability**: Any serialized, persisted, or transmitted format will eventually need to evolve. Use versioned formats from day one.
- **No atomicity for multi-step operations**: Any sequence of operations that must succeed or fail together—and that can fail partway through—MUST use transactional, WAL-based, or atomic file-swap semantics. Partial completion is a corruption scenario.
- **Panicking inside a `Mutex` lock guard**: A panic while holding a `Mutex` poisons it permanently. All subsequent attempts to acquire the lock will return `Err`. If the locked critical section can panic, the code MUST handle poisoning at every call site, or restructure to avoid panicking under the lock.

### Architectural Anti-Patterns

- **God modules**: A module with five or more unrelated responsibilities is doing too much. Split along domain boundaries; delegate clearly.
- **God structs**: A struct with unrelated fields from multiple concerns should be decomposed into smaller, focused types. Use composition.
- **Sideways coupling**: Layer A directly calls into Layer C, bypassing Layer B. This breaks contracts, complicates testing, and makes tracing failures difficult. All coupling MUST flow through declared interfaces.
- **Global mutable state**: `static mut`, `lazy_static!`, or `once_cell` with interior mutability create implicit shared state that is invisible at call sites. Dependency-inject shared resources explicitly.
- **Excessive trait bounds**: A function with many unrelated trait bounds is a signal of over-abstraction. Simplify the interface or use a concrete type.
- **Speculative generality**: Building configurability, extension points, or abstractions for hypothetical future requirements. Follow YAGNI. Every abstraction must pay for itself with current, not imagined, usage.
- **Context-free error propagation**: Errors bubbled from lower layers without adding context make debugging across layer boundaries nearly impossible. Add context at every propagation boundary.
- **Exposed internals**: Making internal types `pub` because "they might be useful externally" pollutes the public API and creates semver obligations for types that should be free to change. Start private; promote only when justified.
- **Circular crate dependencies**: They break modularity, create compile-order problems, and are a sign of unclear domain boundaries. Restructure.
- **`RefCell<T>` to bypass the borrow checker**: `RefCell` moves borrow checking to runtime, turning compile-time errors into panics. It has legitimate uses (e.g., `Rc<RefCell<T>>` in single-threaded graph structures), but using it to avoid reasoning about lifetimes is a design problem.

### Testing Anti-Patterns

- **Test-only behavior in production code**: `#[cfg(test)]` blocks that change logic or behavior in non-trivial ways are a test smell. Extract the logic so it can be tested without altering production behavior.
- **Testing implementation, not behavior**: Tests that assert which internal functions were called, rather than what the observable result is, break on refactoring and provide false confidence. Test what the code produces, not how.
- **Happy-path-only tests**: Tests that only cover expected inputs are incomplete. Edge cases—empty input, max input, concurrent access, partial failure—are where real bugs live.
- **Mocking everything**: Unit tests that mock the database, filesystem, and network pass in isolation but fail to catch integration-level bugs. Use real I/O in integration tests.
- **Slow test suites**: A test suite that takes more than a few minutes to run discourages frequent execution and delays feedback. Parallelize, isolate, and use fixtures strategically.
- **Flaky tests**: Tests that pass or fail non-deterministically indicate a race condition or external dependency. A flaky test is a real bug—it MUST be fixed, not skipped.
- **No concurrency tests**: Single-threaded tests cannot detect race conditions. MUST use `loom` or similar tools for concurrent code paths.
- **Brittle tests**: Tests that fail because of unrelated changes are tightly coupled to implementation. Write tests against public behavior and contracts, not internal structure.

### Documentation Anti-Patterns

- **No doc comments**: "The code is self-explanatory" is almost never true six months later. Every public item MUST explain why it exists and how it should be used.
- **Paraphrasing the signature**: `/// Returns the value` when the return type already says `-> T`. Doc comments MUST explain behavior, contract, and intent—not restate the syntax.
- **Generic or marketing-style docs**: Descriptions with no concrete information. Documentation is for engineers. Be precise and specific.
- **No examples on public APIs**: Developers should not have to guess how to use a type or function. Runnable `# Examples` blocks are REQUIRED for non-trivial APIs.
- **Comments as a band-aid**: `// TODO: this is a hack` or `// fix later` in committed code. Fix it or create a tracked issue. Comments on broken code that remain in the repository are PROHIBITED.
- **Undocumented design decisions**: Future maintainers cannot know the constraints, tradeoffs, and rejected alternatives unless they are written down. Document the why behind non-obvious decisions.
- **Stale documentation**: Docs that describe behavior the code no longer has. Stale documentation is misleading and produces real bugs. Keep docs in sync or delete them.

### Dependency Anti-Patterns

- **Unjustified dependency accumulation**: Adding crates for convenience without evaluating their cost. Every dependency added enlarges the attack surface, the compile time, and the maintenance burden.
- **Unmaintained crates**: Dependencies with no meaningful updates in several years carry compounding security and compatibility risk. Regularly audit and migrate.
- **Under-scoped dependency evaluation**: Adding a crate without checking its transitive dependency tree. The transitive graph is your problem too.
- **Enabling unused features**: Activating all features of a crate to get one specific feature pulls in code, dependencies, and compile time you do not need. Enable only what is used.
- **Overly loose version constraints**: Using `*` or very wide version ranges will eventually cause unexpected breakage. Use exact pins or well-justified compatible ranges.
- **Ignoring audit output**: `cargo audit` or `cargo deny` reporting vulnerabilities that are dismissed without documentation. Every finding MUST be resolved or explicitly justified in `deny.toml` or `audit.toml` with a rationale.



<!-- /// END ///
############################### -->
<h2></h2>
<blockquote>REPS &mdash; Version 0.2.0</blockquote>
<span>Copyright &copy; 2026 James Gober.</span>