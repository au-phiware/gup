---
implementer: sonnet
implementer-reason:
  a new workflow and two mask tasks around fixed commands, plus a features table
  for the facade fixed in Decisions
touches:
  [
    Cargo.toml,
    crates/gup-core/Cargo.toml,
    crates/gup-core-macros/Cargo.toml,
    crates/gup-text/Cargo.toml,
    crates/gup-wgsl/Cargo.toml,
    gup-egui/Cargo.toml,
    gup-bevy/Cargo.toml,
    .github/workflows/,
    maskfile.md,
  ]
---

# GUP-589: Publish Dry Run and docs.rs Build in CI

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 📋 Planned
**Created**: 2026-11-02

## Context

After [GUP-588](GUP-588_Publish_Metadata_And_Licence_Headers.md) the seven
crates package. Nothing proves they would publish. `cargo publish --dry-run`
also builds each packaged crate against the others as packaged. Nothing proves
docs.rs could build their documentation either. docs.rs builds offline, with
`--cfg docsrs`, for the targets and features in `[package.metadata.docs.rs]`. A
crate whose docs fail there has no documentation page. `mask doc` (GUP-533)
builds the docs on the host target with default features only.

Cargo 1.93 (the pinned toolchain) has `cargo publish --workspace`. With
`--dry-run`, it packages every publishable member in dependency order, and it
verifies each one against a local overlay of the members not yet on crates.io.
One command checks the whole release.

Two facts shape the docs.rs build. `gup_core::web` (`CanvasApp`, `CanvasTarget`)
compiles only for `wasm32` with feature `web`, so a host-only docs.rs build
leaves the browser API out. And the `gup` facade has no features of its own: it
depends on `gup-core` with default features (`window`, `svg`). A browser user
cannot turn on `web` through `gup`, or turn off `window`. They must depend on
`gup-core` directly, which the README does not tell them.

## Read First

- `Cargo.toml` (root) `[dependencies]` and `[package.metadata.docs.rs]`, and
  `crates/gup-core/Cargo.toml` `[features]` (lines 60–75).
- `crates/gup-core/src/lib.rs` lines 85–100 (the features table) and 160–225
  (which modules each feature gates).
- `maskfile.md` "## doc" (lines 611–618) and "## ci" (lines 407–430, the
  per-workflow subtasks).
- `.github/workflows/lint.yml`: the toolchain and cache steps to copy.

## User Story

> "As the maintainer, I want CI to prove on every push that the workspace would
> publish and that docs.rs would document it, so that release day is one
> command."
>
> "As a web developer, I want
> `gup = { default-features = false, features = ["web"] }`, so that I can use
> the facade in the browser."

## Acceptance Criteria

### AC1: The facade's features

- [ ] The root `Cargo.toml` has the `[features]` table of Decisions, and its
      `gup-core` dependency sets `default-features = false`.
- [ ] `cargo check -p gup --target wasm32-unknown-unknown --no-default-features --features web`
      passes. So does `cargo check -p gup` (default features, host).
- [ ] A doctest, or the crate docs' features table, shows the browser dependency
      line:
      `gup = { version = "0.1", default-features = false, features = ["web", "svg"] }`.
      `src/lib.rs` gains a "## Features" section with the table of Decisions,
      and `mask doc` passes.

### AC2: docs.rs metadata

- [ ] Each of the seven published crates has the `[package.metadata.docs.rs]`
      table of Decisions.
- [ ] `scripts/docs_rs.sh` reads each published crate's docs.rs table with
      `cargo metadata` and `jq`. For each listed target, it runs
      `cargo doc --no-deps -p <crate> --target <t>`, with `--all-features` when
      `all-features = true`. It sets `RUSTDOCFLAGS` to the table's
      `rustdoc-args` plus `-D warnings`, and `DOCS_RS=1`. It exits non-zero on
      the first failure and names the crate and target. `mask docs-rs` runs it,
      and it passes locally.
- [ ] After `mask docs-rs`, the wasm32 docs of `gup-core` contain
      `web/struct.CanvasApp.html`:
      `test -f "$CARGO_TARGET_DIR/wasm32-unknown-unknown/doc/gup_core/web/struct.CanvasApp.html"`.

### AC3: Publish dry run

- [ ] `mask publish-check` runs `cargo publish --workspace --dry-run --locked`.
      Run it locally: it packages and verifies all seven crates. Paste the last
      ten lines into the Implementation Summary.

### AC4: The workflow

- [ ] `.github/workflows/release-check.yml` (name "Release check") runs on push
      to `main`, on pull requests and on `workflow_dispatch`. It has two jobs,
      `publish-dry-run` (`mask publish-check`'s command) and `docs-rs`
      (`./scripts/docs_rs.sh`). Each job installs the toolchain from
      `rust-toolchain.toml` with `rustup toolchain install` and caches the cargo
      registry as `lint.yml` does. Each has a 60-minute timeout.
- [ ] `actionlint .github/workflows/*.y*ml` passes (it is in `mask all-check`).
      `maskfile.md` gains `### ci release-check`, running both commands, beside
      the other `mask ci` subtasks.
- [ ] The workflow passes on GitHub for the story's branch. Link the run in the
      Implementation Summary. If the branch cannot be pushed from the worker,
      say so; the orchestrator links it after the merge.

## Decisions

- **The facade's features** (root `Cargo.toml`). They mirror `gup-core`'s, so
  `gup` stays a facade with nothing of its own:

  ```toml
  [dependencies]
  # Not `workspace = true`: a member cannot turn off default features that the
  # workspace entry leaves on, and gup-egui and gup-bevy want them on.
  gup-core = { path = "crates/gup-core", version = "=0.1.0", default-features = false }

  [features]
  default = ["window", "svg"]
  # A winit window target, `show` and `GupApp`.
  window = ["gup-core/window"]
  # Data marks in `SvgTarget`.
  svg = ["gup-core/svg"]
  # A `<canvas>` target and host (`web::CanvasApp`); wasm32 only.
  web = ["gup-core/web"]
  ```

  GUP-598's version check counts this line as a fourth place that holds the
  version, beside `[workspace.package]` and `[workspace.dependencies]`. The
  crate docs' table in `src/lib.rs` has three columns, as `gup-core`'s does:
  feature, default, what it adds.

- **docs.rs tables**. docs.rs builds only the listed targets:
  - `gup`, `gup-core`: `all-features = true`,
    `targets = ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"]`,
    `rustdoc-args = ["--cfg", "docsrs"]`.
  - `gup-core-macros`, `gup-wgsl`, `gup-text`, `gup-egui`, `gup-bevy`:
    `targets = ["x86_64-unknown-linux-gnu"]`,
    `rustdoc-args = ["--cfg", "docsrs"]`.

  `all-features` on wasm32 is safe: `scripts/clippy_wasm32.sh` already lints
  `gup-core --all-features` on wasm32. `all-features` on the host is safe
  because `web`'s dependencies are wasm32-only target dependencies.

- **Stable rustdoc, not nightly.** docs.rs uses a nightly rustdoc. This project
  pins one toolchain (GUP-403), and no code uses `doc_cfg` or other nightly-only
  attributes. The pinned stable toolchain with `--cfg docsrs` is the check. A
  nightly-only failure on docs.rs is a risk accepted here (Risk Assessment).

- **No network in the docs build.** `DOCS_RS=1` is set, as docs.rs sets it. The
  build scripts (`gup-core/build.rs`, `gup-wgsl/build.rs`) read only their own
  package's files. The script does not pass `--offline`, because CI's registry
  cache may be cold.

- **Not in `mask all-check`.** Both checks need the network, and the dry run
  builds Bevy (minutes). They run in CI and by hand (`mask publish-check`,
  `mask docs-rs`), and GUP-598's release check runs both.

- **System packages.** If the dry run's verification of `gup-bevy` or `gup-egui`
  needs a system library on the runner, install it with `apt-get` in the
  workflow. Name each package and the error that needed it in a comment.

## Escalation Triggers

- `cargo publish --workspace --dry-run` fails for a reason a manifest field
  cannot fix. For example, a packaged build reads a file outside its package, or
  the overlay resolves two versions of an internal crate.
- `cargo check -p gup --no-default-features --features web` on wasm32 fails in
  `gup-core` code (a `cfg` gap). Report the error. Do not change `gup-core`'s
  gating in this story.
- docs.rs-style `cargo doc` with `-D warnings` warns only under `--all-features`
  or only on wasm32, in code this story does not touch. Report the warnings.
  Fixing a doc link is in scope; changing an API is not.

## Out of Scope

- Publishing, the crates.io token, and the version tag: owner steps after
  GUP-598.
- A semver check against a published baseline (`cargo-semver-checks`): there is
  no baseline before 0.1. It is a follow-up after the first release.
- The README's install lines: GUP-597 (features) and GUP-599 (crates.io).

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] Facade features, the wasm32 and host checks, and the features section
      (AC1). Commit.
- [ ] docs.rs tables, `scripts/docs_rs.sh`, `mask docs-rs` (AC2). Commit.
- [ ] `mask publish-check` and its local run (AC3). Commit.
- [ ] The workflow and `mask ci release-check` (AC4); actionlint; DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-588: Publish metadata and licence headers 📋 — packageable manifests.

### Enables Stories

- GUP-597: README for release 0.1 — documents the facade's features.
- GUP-598: Release checklist — runs both checks.

## Testing Strategy

- **Commands**: `cargo check` with the feature sets of AC1, `mask docs-rs`,
  `mask publish-check`, `actionlint`.
- **CI**: the new workflow green on the branch.
- **Regression**: `mask all-check`; root `cargo test -- --test-threads=1`.

## Success Metrics

- [ ] Every push proves that the seven crates publish and document.
- [ ] `gup` works in the browser without a direct `gup-core` dependency.

## Risk Assessment

- **Medium**: docs.rs's nightly rustdoc rejects something stable accepts, and
  the first published version has no docs page. _Mitigation_: no code uses
  nightly doc attributes. If docs.rs fails after publishing, a patch release
  fixes it; docs.rs can also rebuild on request.
- **Low**: the dry run takes long in CI (seven verifications, Bevy among them).
  _Mitigation_: a separate workflow, so the Tests and Lint workflows are not
  slowed; 60-minute timeout.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable, nothing renders
      differently
- [ ] `mask wasm-browser`: not needed; no layout, input or `gup-core` wasm code
      changes (the facade's features only forward)
- [ ] WASM size: unchanged, no library code change (the harness uses `gup-core`
      directly)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
