# Releasing mod-events

This document codifies the release procedure. Following it keeps the
git tag, `Cargo.toml` version, and `CHANGELOG.md` heading in lockstep
— a misalignment between the three is the most common release
incident.

## Pre-flight

Before any release work, the repository must satisfy:

- `main` is checked out and clean (`git status` shows no uncommitted
  changes).
- The CI workflow on the latest `main` commit is green (use
  `gh run list --limit 1` to confirm).
- All items in the [`CONTRIBUTING.md`](CONTRIBUTING.md) "Local CI
  gate" section pass locally.

## Picking the version

The crate uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
For a `0.x` crate:

- **MINOR bump** (`0.2.x` → `0.3.0`): any breaking change to the
  public API, behavior, or feature set. Examples: removing a method,
  changing a return type, raising MSRV.
- **PATCH bump** (`0.2.0` → `0.2.1`): bug fixes, security patches,
  performance improvements with no API change, doc-only updates.
- **Pre-release suffix**: `-alpha`, `-beta`, or `-rc` per
  REPS §Versioning. The first stable release of a new MINOR is
  typically tagged `x.y.0` directly without a pre-release.

## Steps

1. **Decide the version.** Settle on `x.y.z` (or
   `x.y.z-beta`/`-rc`). Refer to the
   [`Unreleased`](CHANGELOG.md#unreleased) section to see what
   accumulated changes warrant.

2. **Update `Cargo.toml`.**

   ```toml
   [package]
   version = "x.y.z"
   ```

   Update the comment above `version` to record the release date and
   one-line rationale.

3. **Graduate `[Unreleased]` in `CHANGELOG.md`.**

   - Move all entries under `## [Unreleased]` into a new
     `## [x.y.z] — YYYY-MM-DD` section. Use the ISO date.
   - Reset `## [Unreleased]` to an empty heading (no `TBD.`
     placeholders).
   - Add a one-paragraph release summary at the top of the new
     section.
   - Add a link to the release notes file
     (`docs/release/vx.y.z.md`).
   - Update the link references at the bottom of the file:

     ```markdown
     [Unreleased]: https://github.com/jamesgober/mod-events/compare/x.y.z...HEAD
     [x.y.z]: https://github.com/jamesgober/mod-events/compare/PREV...x.y.z
     ```

4. **Sweep version-referencing documentation.**

   Every install snippet must show the literal new version, not a
   caret range:

   - `README.md` Quick Start
   - `docs/quick-start.md`
   - `docs/api-reference.md` Feature Flags section
   - `docs/migration.md`

   Use `grep` to find stragglers:

   ```bash
   grep -rn "mod-events = \"" --include="*.md" .
   ```

5. **Write release notes.**

   Create both:

   - `docs/release/vx.y.z.md` — public, git-tracked, what shows up on
     the GitHub release page.
   - `.dev/release/vx.y.z.md` — private working draft (git-ignored).

   Both should describe what changed, who is affected, breaking
   changes if any, and the upgrade path.

6. **Run the full local CI gate.** All commands in
   [`CONTRIBUTING.md`](CONTRIBUTING.md) "Local CI gate" must pass.

7. **Commit.**

   ```bash
   git add .
   git commit -m "release x.y.z: <one-line summary>"
   ```

8. **Push and confirm CI.**

   ```bash
   git push
   gh run watch
   ```

   Do not proceed to tagging if the workflow fails.

9. **Tag and push the tag.**

   ```bash
   git tag -a x.y.z -m "x.y.z"
   git push --tags
   ```

   The tag, `Cargo.toml` `version`, and CHANGELOG heading must all
   read `x.y.z` exactly.

10. **Cut the GitHub release.**

    ```bash
    gh release create x.y.z \
      --title "vx.y.z" \
      --notes-file docs/release/vx.y.z.md
    ```

    For pre-releases (`-alpha`, `-beta`, `-rc`), add `--prerelease`.

11. **Publish to crates.io.**

    ```bash
    cargo publish
    ```

    `cargo publish` re-runs the full build against a freshly extracted
    tarball. If anything fails here that did not fail in the local
    gate, it is almost always because the `Cargo.toml` `exclude` field
    is stripping a file the build needs. Add it back, repeat from
    step 7 with a `-rc.N` pre-release, and try again.

## After publishing

- Verify the crate appears on `crates.io/crates/mod-events` and
  `docs.rs/mod-events` (docs.rs build can take 5–30 minutes).
- Check the [README badges](README.md) at the top of the rendered
  README — version + docs + CI badges should reflect the new release
  within the hour.
- Update the project board / roadmap with the published version.

## If something goes wrong

- **Wrong version in the tag.** Delete the tag locally and remotely
  (`git tag -d x.y.z; git push --delete origin x.y.z`), fix
  `Cargo.toml` + CHANGELOG, retag, and republish. If the bad tag
  triggered a `cargo publish` already, **yank** it
  (`cargo yank --vers x.y.z`) — never delete a published version,
  always yank.
- **CHANGELOG missed an entry.** Add a follow-up patch release
  (`x.y.z+1`) with a CHANGELOG entry under `### Fixed`:
  `changelog entry for x.y.z corrected — <what was missed>`.
- **CI fails post-publish.** Land the fix on `main`, cut a patch
  release, and yank the bad version with a note pointing users to the
  patch.

## REPS reference

This procedure implements REPS §Versioning, REPS §Crate Packaging,
and the in-repo [`.dev/DIRECTIVES.md`](.dev/DIRECTIVES.md) §D-1
discipline rules. Where any of those documents disagree with this
file, the upstream document wins and this file MUST be amended.
