# Security Policy

## Supported Versions

Security fixes are applied to the most recent minor release line. Older
minor releases are not patched.

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| 0.1.x   | :x:                |

## Reporting a Vulnerability

**Do not file a public GitHub issue for security vulnerabilities.**
Public issues become known to attackers before a fix is available.

Report vulnerabilities one of two ways:

1. **GitHub private vulnerability report** (preferred). Open the
   "Security" tab on this repository and click *"Report a
   vulnerability"*. This files a private advisory only the maintainer
   can see, and tracks coordinated disclosure through to a CVE if
   warranted.
2. **Email** [me@jamesgober.com](mailto:me@jamesgober.com) with the
   subject line `mod-events security`. Include a clear description of
   the issue, a minimal reproducer, and any relevant version numbers.
   You will receive an acknowledgement within 7 days.

Please include in your report:

- The crate version (`Cargo.toml` `mod-events` line and `Cargo.lock`
  resolved version).
- Rust toolchain version (`rustc --version`).
- A minimal proof-of-concept demonstrating the issue.
- The impact (memory safety, denial of service, information leak, etc.).
- Whether the issue is already publicly known.

## Disclosure Timeline

- **Day 0**: Report received and acknowledged.
- **Day 1–14**: Triage, severity assessment, fix design.
- **Day 14–60**: Fix implemented, tested, and prepared for release.
- **Day 60+**: Patched release published. Public advisory and (if
  warranted) RustSec / GHSA filing follow within 7 days of the
  release.

If you need a longer or shorter window for coordination (for example
because the issue is already in the wild), say so in your report and
we will adjust.

## Scope

In-scope:
- Memory safety issues in `mod-events` itself.
- Logic errors that allow unauthorized listener invocation, event
  spoofing, or `DispatchResult` tampering.
- Denial-of-service issues (panics, deadlocks, unbounded resource
  growth) reachable from the public API with reasonable inputs.

Out of scope:
- Vulnerabilities in transitive dependencies (`tokio`, `parking_lot`,
  etc.). These are reported through the upstream crates' own security
  policies. We track them via `cargo audit` in CI and patch the
  lockfile as soon as a fixed version is published.
- Issues that require a malicious or compromised process inside the
  same address space — `mod-events` is not a sandbox.
- Misuse documented in the public docs (e.g. listeners panicking with
  `panic = "abort"` in the consumer's profile).
