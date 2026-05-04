# Contributing to mod-events

Thanks for considering a contribution. This document covers the practical
expectations for changes that land in this crate.

## Quick start

```bash
git clone https://github.com/jamesgober/mod-events.git
cd mod-events
cargo test --all-features
```

The repository pins its toolchain in `rust-toolchain.toml`, so `cargo`
will automatically download the right `rustc` (currently 1.95.0) the
first time you build. The crate's MSRV is **Rust 1.81**.

## Local CI gate

Every change must pass these checks before being merged. They are the
same commands the GitHub Actions workflow runs.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-features
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo audit
cargo deny check all
RUSTUP_TOOLCHAIN=1.81.0 cargo build --all-features    # MSRV check
RUSTFLAGS="--cfg loom" cargo test --release --test loom_concurrent
```

If you do not have `cargo-audit` or `cargo-deny` installed:

```bash
cargo install cargo-audit cargo-deny
```

The Windows-elevation requirement on the criterion bench binary means
`cargo bench` may not run locally on Windows; it does run in CI.

## Engineering standards

This crate follows the engineering standards in
[`REPS.md`](REPS.md) — *Rust Efficiency and Performance Standards*. The
short version:

- No `unwrap()` / `expect()` / `panic!` in library code.
- No `Box<dyn Error>` in public APIs; use the typed `ListenerError`.
- All public items have doc comments. Doc comments explain the *why*,
  not just the *what*.
- No emoji or marketing words (`comprehensive`, `robust`, `seamless`,
  `leverage`) in prose.
- Performance assertions must be backed by `criterion` benchmarks.
- Concurrency invariants are validated by `loom` model checks.

The crate-root `#![deny(...)]` block in [`src/lib.rs`](src/lib.rs)
enforces a large subset of these rules at compile time. Read it before
writing code; you will see lints fail otherwise.

## Pull request expectations

- Branch from `main`. Branch names use the prefixes `feature/`,
  `fix/`, `docs/`, `perf/`, or `refactor/`.
- Commit messages are imperative, lowercase, concise, with no trailing
  period. Example: `add panic safety to sync dispatch path`. Not
  `Added panic safety...`.
- Each commit is a single logical change. Mixing a refactor with a
  feature in one commit is not accepted.
- Every user-visible change adds an entry under `## [Unreleased]` in
  [`CHANGELOG.md`](CHANGELOG.md) in the same PR. The entry describes
  the *user-visible effect*, not the implementation. Keep a Changelog
  1.1.0 sections in this order: `Added`, `Changed`, `Deprecated`,
  `Removed`, `Fixed`, `Security`. Skip empty sections.
- Breaking changes are called out with a `**Breaking:**` prefix in the
  CHANGELOG entry.
- New public APIs need doc comments and at least one runnable
  `# Examples` block.
- New behavior needs a test. Edge cases (empty input, max input,
  concurrent access, partial failure) are part of "the test".
- Test names follow `test_<subject>_<condition>_<expected>`, e.g.
  `test_dispatch_with_panicking_listener_collects_error_and_continues`.

## What gets rejected

- Code that compiles but fails any item in the local CI gate above.
- Public API changes without a CHANGELOG entry.
- Doc comments that only paraphrase the function signature.
- New dependencies without a `# why:` comment in `Cargo.toml`
  justifying them.
- Performance claims unsupported by a benchmark.
- Bypassing commit hooks (`--no-verify`) or signing checks.

## Reporting bugs

Open a GitHub issue with:
- The crate version (from `Cargo.toml` and `Cargo.lock`).
- The Rust toolchain version (`rustc --version`).
- A minimal reproducer.
- The expected vs. actual behavior.

For **security** issues, do **not** open a public issue. Read
[`SECURITY.md`](SECURITY.md) for the private disclosure process.

## License

By contributing, you agree that your contributions will be licensed
under the [Apache-2.0](LICENSE) license that covers the rest of the
project.
