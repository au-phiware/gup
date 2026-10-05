# GUP-390: Quarantine GPU Culling/LOD and Park Integration Crates

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: ✅ Complete (2026-10-05)
**Created**: 2026-10-04

## Context

This story implements the "quarantine/move" half of **T1** from the
[October 2026 strategic review](../STRATEGIC_REVIEW_2026-10.md#t1--prune-and-public-surface-sm-low-risk),
split out from GUP-389 (which deletes dead code outright) because this work
_moves_ or _feature-gates_ code that is expected to be reused later, rather than
deleting it — a materially different and lower-risk kind of change that can
proceed in parallel with GUP-389 with minimal file overlap (GUP-389 edits
`src/lib.rs`'s module list for deleted modules; this story edits it for moved
modules and `Cargo.toml`'s workspace members — coordinate the `src/lib.rs` diff
if both land close together).

Per [Decision 1](../STRATEGIC_REVIEW_2026-10.md#decisions): "GPU culling/LOD...
moves to an experimental crate for T3/T7 to draw from" rather than being
deleted, because — unlike the transpiler or Mixable — this is real, tested GPU
engineering (the review's own words: "the GPU engineering underneath is
genuinely good") that nothing currently wires into chart builders, but that T3
(GPU columns, billion-point goal) and T7 (wiring culling/LOD into core) are
expected to need.

Per [Decision 3](../STRATEGIC_REVIEW_2026-10.md#decisions): park `gup-egui`,
`gup-bevy`, `gup-ios`, `gup-android`, and `gup-tauri` (exclude from
workspace/CI) until T2 (one Context/Scene/RenderTarget) lands, then re-wire them
onto the single render path. Verified on 2026-10-04: `cargo check -p gup-egui`
and `cargo check -p gup-bevy` **both fail** with the same root cause —
`ComposedChart::render()` (`src/chart_builder.rs:1161-1163`) requires
`M: crate::selection::MarkInstanceBuilder`, which the generic chart types used
by both integration crates do not satisfy
(`error[E0277]: the trait 'MarkInstanceBuilder' is not implemented for 'M'`).
This matches the review's "`gup-egui` doesn't compile against HEAD" finding
exactly. `gup-ios` and `gup-android` currently do compile, but depend on
`gup-macros`' transpiler module being present (their build emits dead-code
warnings from `gup-macros/src/transpile/validation.rs`), so they are affected by
GUP-389's deletions and should be re-verified after that story lands.

**Revised 2026-10-05 (after GUP-389 merged).** The compile failure above no
longer holds: GUP-389 commit `353e85c` fixed the `DynChart` bound in both
crates, and `gup-egui` and `gup-bevy` compile against `main`. The owner's
decision to park all five integrations still stands, for a different reason:
RFC-001 replaces `ComposedChart`, `DynChart` and the context types these crates
are built on. Keeping them compiling through RFC-001 S1–S13 is churn the owner
declined. They are parked to avoid that churn, not because they are broken, and
must be re-wired at RFC-001 S13 (`gup-egui`, `gup-bevy`) and in strategic review
T7 (`gup-ios`, `gup-android`, `gup-tauri`). GUP-389 also left
`src/mark/batch_renderer.rs` in place because its `InstanceAttributes` and
`Viewport2D` are used only by the culling/LOD code; it moves with that code
here.

Verified module locations for the culling/LOD move:
`src/mark/compute_instance_filter.rs`, `src/mark/occlusion_culler.rs`,
`src/mark/radix_sort.rs` (+ `src/shaders/radix_sort.compute.wgsl`),
`src/mark/unified_culling_pipeline.rs`, `src/renderer/` (`mod.rs`, `blend.rs`,
`viewport.rs`, `viewport_cull.rs`, `adaptive.rs`, `debug_overlay.rs`), and
`src/lod/` (`mod.rs`, `selection.rs`, `streaming.rs`) — combined, roughly 11,100
lines (`wc -l` across these files). A caller check (`grep -rln` for
`compute_instance_filter::`, `occlusion_culler::`, `radix_sort::`, and
`unified_culling_pipeline::` across `src/*.rs`, `src/chart_builder/**/*.rs`, and
`src/mark.rs`) found only `src/mark.rs` and `src/lib.rs` as callers outside the
modules themselves, consistent with "GPU culling and LOD... [is] unwired."

Debug and benchmark code: `src/debug.rs` + `src/debug/` total ~10,185 lines;
`src/performance.rs`, `src/performance_export.rs`, `src/performance_targets.rs`
total ~2,600 lines. These are not necessarily _unused_ (performance tests and
some examples reference them), so they are feature-gated rather than deleted or
moved to a separate crate, per the review's "Move debug, benchmark and
performance-report code behind a `debug` feature or into `gup-debug`."

## User Story

> "As a maintainer, I want GPU culling/LOD code preserved but out of the default
> build, so T3/T7 can draw from it later without it adding to today's compile
> time, binary size, or public-API surface."
>
> "As a CI maintainer, I want integration crates that don't currently compile
> excluded from the default workspace build, so a CI failure always means a real
> regression in a crate that is supposed to work, not a known-broken parked
> crate."

## Acceptance Criteria

### AC1: GPU culling/LOD moves to an experimental crate

- [x] A new workspace member (e.g. `gup-experimental` or `gup-culling`) is
      created, **excluded from the default build** (not listed in the top-level
      `[workspace] members` that `cargo build`/`cargo test` without
      `-p`/`--workspace` flags would include by default — confirm the exact
      exclusion mechanism Cargo supports, e.g. a separate top-level
      `[workspace]` vs. `default-members`). _Done as `crates/gup-culling-lod`: a
      member, with `default-members = ["."]`, so plain `cargo build`/`test` skip
      it but `cargo check -p gup-culling-lod` works. It is deliberately inside
      `--workspace` (see Implementation Summary)._
- [x] `src/mark/compute_instance_filter.rs`, `occlusion_culler.rs`,
      `radix_sort.rs`, `unified_culling_pipeline.rs`, `src/renderer/`, and
      `src/lod/` move into the new crate with their module structure preserved,
      importing `gup` as a normal path dependency for the types they still need
      (`Mark`, buffer pool types, etc.). _Plus `batch_renderer.rs`, five compute
      shaders, two integration tests, six benches and three examples._
- [x] `src/mark.rs` and `src/lib.rs` no longer reference the moved modules; any
      remaining glue code needed for the move compiles.
- [x] The new crate builds independently: `cargo check -p gup-experimental` (or
      chosen name) succeeds.
      _`cargo check -p gup-culling-lod --all-targets     --all-features` passes,
      and CI runs it plus the crate's tests._
- [x] A top-level `README.md` note (or a README in the new crate) explains why
      it exists, that it is unwired from the main render path, and that T3/T7
      are expected to draw from it. _Both: `crates/gup-culling-lod/README.md`
      and the root README's "Workspace Crates" section._

### AC2: Debug and performance code is feature-gated

- [x] `src/debug.rs`/`src/debug/`, `src/performance.rs`,
      `src/performance_export.rs`, and `src/performance_targets.rs` are gated
      behind a Cargo feature (e.g. `debug`), off by default.
- [x] Any example or test that currently uses debug/performance APIs either
      enables the feature explicitly or is updated/moved accordingly — no
      example silently stops compiling with default features. _7 examples and 8
      integration tests declare `required-features = ["debug"]`._
- [x] `cargo build` (default features) does not compile the gated modules;
      `cargo build --features debug` does.

### AC3: Integration crates are excluded from the default workspace and CI

- [x] `gup-egui`, `gup-bevy`, `gup-ios`, `gup-android`, and `gup-tauri` (the
      latter is an example app under `examples/gup-tauri/`, not a workspace
      member — confirm its actual build mechanism before deciding how to exclude
      it) are removed from the default workspace build surface used by CI
      (`cargo build --workspace`, `cargo test --workspace`). _All five are in
      `[workspace] exclude`. `gup-tauri` is built by `cargo tauri` against the
      WASM package and does not depend on the `gup` crate; before this story it
      could not be built at all inside the repository ("believes it's in a
      workspace when it's not"). Excluding it fixes that._
- [x] CI workflow files are updated so these crates are not built/tested by the
      default pipeline. _No default workflow built them; the iOS and Android
      workflows are now manual-dispatch only._
- [x] A top-level README note lists the parked integration crates, why
      (`gup-egui`/`gup-bevy` currently fail to compile against `main`; all five
      are scheduled for re-wiring after T2), and that `cargo check -p gup-egui`
      etc. remain valid manual commands for anyone actively working on re-wiring
      them. _Reason revised (see Context): they compile, and are parked to avoid
      churn. Outside the workspace `-p` cannot work, so the README gives
      `cargo check --manifest-path gup-egui/Cargo.toml` instead._
- [x] This does **not** delete any of the five crates' source — only removes
      them from the default build/CI surface, consistent with "park," not
      "delete."

## Technical Tasks

- [x] Create the new experimental crate's `Cargo.toml` and module skeleton.
- [x] `git mv` the culling/LOD files into the new crate, fixing `use` paths.
- [x] Update `src/lib.rs` and `src/mark.rs` to remove the moved module
      declarations (coordinate with GUP-389's concurrent edit to the same file
      if both are in flight).
- [x] Add `#[cfg(feature = "debug")]` gates (or move into a `gup-debug` crate —
      pick one approach and justify it in the retrospective) around
      `src/debug.rs`, `src/debug/`, `src/performance*.rs`.
- [x] Update `Cargo.toml`'s `[workspace] members` to exclude the five parked
      crates; verify `cargo build --workspace` and `cargo test --workspace` no
      longer touch them.
- [x] Update `.github/workflows/*.yml` (or equivalent CI config) to match.
- [x] Add the README notes from AC1 and AC3.
- [x] Run `cargo build`, `cargo build --features debug`,
      `cargo check     --examples`, `cargo test -- --test-threads=1` and fix
      fallout.

## Dependencies

### Prerequisite Stories

- None — independent of GUP-389, though both touch `src/lib.rs` and should be
  sequenced or rebased carefully if run concurrently in separate worktrees.

### Enables Stories

- **Prerequisite for [RFC-001](../rfcs/RFC-001_Core_Architecture.md) step S0**
  (accepted 2026-10-04), alongside GUP-389: the quarantine/park moves here
  reduce what `crates/gup-core`'s build-out needs to ignore or work around.
- GUP-391 (Curated prelude and public surface purge) is now ⏸ Parked
  (superseded by RFC-001 S7/S14) — no longer a dependent of this story.
- Future T3/T7 work that wires GPU culling/LOD back into the core render path
  can depend on the quarantine crate created here, independent of the RFC-001
  timeline.

## Testing Strategy

- **Build verification**: `cargo build` (default features),
  `cargo build --features debug`, `cargo check -p <experimental-crate>`,
  `cargo check -p gup-ios`, `cargo check -p gup-android` (confirm these two
  still build once GUP-389's transpiler deletion lands).
- **CI verification**: confirm the updated CI workflow actually skips the five
  parked crates (run the workflow or a local equivalent, don't just read the
  YAML).
- **Regression**: no chart-builder behaviour should change — this is a pure
  code-location/feature-gate change with zero runtime effect on the default
  build's rendered output.

## Success Metrics

- [x] `cargo build --workspace` (post-change default members) excludes the five
      parked crates and the experimental culling/LOD crate, and succeeds.
      _Partly by design: `--workspace` excludes the five parked crates and
      succeeds, but includes `gup-culling-lod`, which is a non-default member so
      that `cargo check -p` and CI can cover it. Plain `cargo build` (the
      default members) excludes it._
- [x] `cargo build --features debug` succeeds and exercises the gated
      debug/performance code.
- [x] CI run time/footprint decreases (fewer crates built by default) — report
      the before/after in the retrospective if measurable. _See Retrospective._

## Risk Assessment

- **Medium**: Moving ~11k lines of culling/LOD code risks subtle breakage in
  code that has its own non-trivial tests (occlusion culling, radix sort).
  Mitigation: move file-by-file, running `cargo check -p <experimental-crate>`
  after each, and re-run the moved tests in their new location before
  considering the move complete.
- **Low**: Excluding integration crates from the default workspace could mask a
  _new_ regression in, say, `gup-ios` if nobody runs `cargo check -p gup-ios`
  manually going forward. Mitigation: keep a lightweight, separate "parked
  crates" CI job that runs on a schedule (not on every PR) so drift is caught
  without blocking every commit — note this as a follow-up if not done in this
  story.
- **Low**: Choosing a Cargo feature flag vs. a separate `gup-debug` crate for
  debug/performance code is a judgment call the review leaves open ("behind a
  `debug` feature **or** into `gup-debug`"); document the choice and reasoning
  in the retrospective so it isn't re-litigated later.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] Lint and format clean: `mask all-fix`.
- [x] All examples compile: `cargo check --examples`.
- [x] Story status updated to ✅ Complete in story file and INDEX.md.
- [x] Retrospective added to story document, including the feature-flag vs.
      separate-crate decision for debug/performance code and the before/after CI
      footprint if measured.

## Implementation Summary

Three commits on `main`: `e47c367` (culling/LOD move), `a4f29b0` (park
integrations), `e8376a9` (`debug` feature).

### What moved where

| From (`gup`)                                                                                                           | To (`crates/gup-culling-lod`)                             |
| ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| `src/mark/{batch_renderer,compute_instance_filter,occlusion_culler,radix_sort,unified_culling_pipeline}.rs`            | `src/mark/` (`gup::mark::X` → `gup_culling_lod::mark::X`) |
| `src/renderer/`, `src/lod/`                                                                                            | `src/renderer/`, `src/lod/`                               |
| 5 compute shaders (`instance_filter`, `occlusion_culling`, `radix_sort`, `lod_aggregate`, `viewport_cull`)             | `src/shaders/`                                            |
| `tests/{adaptive_renderer_integration,lod_pyramid}.rs`                                                                 | `tests/`                                                  |
| 6 benches (`compute_filter`, `unified_culling`, `occlusion_culling`, `mark_batch`, `lod_pyramid`, `adaptive_renderer`) | `benches/` (with the `gpu-bench` gate)                    |
| 3 examples (`lod_pyramid_debug`, `adaptive_lod_debug`, `streaming_lod_scatter`)                                        | `examples/`                                               |

About 13.0k lines of Rust library code, 1.5k lines of WGSL and 2.5k lines of
tests, benches and examples. The root re-exports (`gup::ComputeInstanceFilter`,
`gup::Viewport2D`, ...) are now `gup_culling_lod::*`. Only `crate::` paths to
`gup` types (`error`, `context`, `buffer`, `streaming`, `test_utils`, `render`,
`mark::{Mark, Circle, Rectangle, performance_opt}`) had to change. The caller
check (module paths and all re-exported type names across `src/`, `tests/`,
`benches/`, `examples/`) found no `gup` caller outside `lib.rs`/`mark.rs`
re-exports.

### Workspace and CI

- `Cargo.toml`:
  `members = ["gup-macros", "crates/gup-visual-regression", "crates/gup-culling-lod"]`,
  `default-members = ["."]`, and
  `exclude = [gup-egui, gup-bevy, gup-ios, gup-android, examples/gup-tauri/src-tauri]`.
  `"."` had to leave `members` because Cargo treats it as a prefix of every
  subdirectory, which overrides `exclude`.
- `Cargo.lock` lost 190 packages (bevy, egui, eframe, naga_oil, ...). No
  remaining package changed version.
- `visual-regression.yml` runs
  `cargo check -p gup-culling-lod --all-targets --all-features` and
  `cargo test -p gup-culling-lod`, and builds the crate's examples for the smoke
  test. `tests/examples_smoke.rs` now covers examples from both `gup` and
  `gup-culling-lod`.
- `ios-ci.yml` and `android-ci.yml` run on manual dispatch only and build their
  crate via `--manifest-path` / `working-directory` with `CARGO_TARGET_DIR` set
  to the repository's `target/`.
- `performance.yml` passes `--features debug` to `performance_ci_tests`.

### `debug` feature

- `debug`, `performance`, `performance_export`, `performance_targets` (about
  12.8k lines) and the root `pub use debug::*` are `#[cfg(feature = "debug")]`.
- `GupContext`'s `performance_profiler` field and its five profiling methods are
  gated; `web-dashboard` implies `debug`.
- 7 examples and 8 integration tests declare `required-features = ["debug"]`.

### Docs

Root README "Workspace Crates" section (experimental crate, `debug` feature,
parked crates with `--manifest-path` commands),
`crates/gup-culling-lod/README.md`, parked notes in the `gup-egui`, `gup-bevy`
and `gup-tauri` READMEs, `docs/LOD_SYSTEM.md`, `docs/mark-system/*`,
`docs/PERFORMANCE_GUIDE.md`, the examples index and the gallery.

### Verification (2026-10-05)

- `cargo test -- --test-threads=1` (default features): 3762 passed, 0 failed,
  152 ignored across 107 test binaries (lib: 2502 passed).
- `cargo test --features debug --lib` plus the 8 gated tests: lib 2637 passed;
  gated tests 46 passed, 1 ignored.
- `cargo test -p gup-culling-lod -- --test-threads=1`: 178 unit, 16 integration
  and 6 doc tests passed.
- `mask visual-regression`: 16 passed.
- `mask smoke-examples`: 56 passed, 3 expected failures (all pre-existing
  tracked entries: `density_scatter_overlay`, `gpu_debug_demo`,
  `pattern_pipeline_demo`), 45 skipped. The three moved examples run and pass.
- `cargo check --all-targets` with default features and with `--features debug`;
  `cargo check --target wasm32-unknown-unknown --lib`; `cargo check --workspace`
  (checks only `gup`, `gup-macros`, `gup-visual-regression`, `gup-culling-lod`).
- `cargo check --manifest-path <crate>/Cargo.toml --all-targets` passes for
  `gup-ios`, `gup-android`, `gup-egui` and `gup-bevy`.
- `mask old-path-loc`: 32051 → 28882.
- No rendered output changed: no golden image was re-blessed.

## Retrospective

**Completed**: 2026-10-05

### Key Technical Learnings

#### `"."` in `[workspace] members` silently defeats `exclude`

- **Challenge**: After adding the five parked crates to `exclude`, every one of
  them still failed with "current package believes it's in a workspace when it's
  not". A minimal workspace in `/tmp` with the same `exclude` worked.
- **Solution**: Cargo decides "explicitly a member" by a path-prefix test
  against each `members` entry, and `root/.` is a prefix of every subdirectory,
  so the explicit-member rule overrides `exclude`. Removing `"."` fixed it. The
  root package is a member implicitly whenever the root manifest has a
  `[package]`.
- **Pattern**: Never list `"."` in `members`. Test exclusion with
  `cargo metadata --no-deps` run from inside the excluded crate; it fails in
  milliseconds if the crate is still claimed.

#### Moving a module tree is cheapest when its paths are kept

- **Challenge**: About 13k lines across 16 files refer to each other through
  `super::`, `crate::mark::...` and `crate::lod::...`.
- **Solution**: Keeping the `mark/`, `lod/` and `renderer/` layout in the new
  crate meant only `crate::` paths to `gup` types needed rewriting. One perl
  pass over a closed list of prefixes (`error`, `context`, `buffer`,
  `streaming`, `test_utils`, `render`, `mark::{Mark, Circle, ...}`) plus
  `gup::lod` → `gup_culling_lod::lod` in docs, tests and benches. The crate
  compiled on the first `cargo check`, and its 200 tests passed unchanged.
- **Pattern**: When quarantining code, preserve module paths and rewrite only
  the boundary. Flattening or renaming can come later, when someone ports it.

#### `required-features` beats `#[cfg]` in test files

- **Challenge**: 7 examples and 8 integration tests use the gated modules.
- **Solution**: `required-features = ["debug"]` on each target. Default builds
  skip them silently, `--all-features` builds (clippy, the smoke test,
  visual-regression CI) still cover them, and `cargo test --test X` without the
  feature fails loudly instead of running zero tests.
- **Pattern**: Gate whole targets in the manifest; reserve `#[cfg]` for code
  inside the library.

#### The clippy gate is weaker than everyone thinks

- **Challenge**: `cargo clippy --no-deps -p gup-culling-lod -- -D warnings`
  failed, and the plain `cargo clippy -- -D warnings` on `gup` failed with 106
  errors, yet the pre-commit hook passed.
- **Solution**: The hook's `clippy --fix ... -- -D warnings` downgrades
  non-fixable lints to warnings and exits 0 (99 warnings on `main`). Fixed the
  two lints in the moved code; wrote GUP-398 for the rest.
- **Pattern**: When a gate "passes", check that it can fail. Use
  `cargo clippy --no-deps -p <crate>` to lint one crate whose dependencies have
  lint debt.

### Architectural Decisions

#### Feature flag, not a `gup-debug` crate, for debug/performance code

- **Decision**: A `debug` Cargo feature (off by default) gates `debug`,
  `performance`, `performance_export` and `performance_targets`.
- **Reasoning**: `GupContext` owns an optional `PerformanceProfiler`, and the
  debug modules use `gup` types throughout. A `gup-debug` crate would depend on
  `gup` while `gup` depends on it for the profiler hook: a cycle, or a redesign
  of the hook. RFC-001's new `Context` replaces `GupContext`, so that redesign
  would be wasted. The feature delivers the review's goal (out of the default
  build and API) in about 40 lines.
- **Trade-off**: The code stays in `src/`, so it still counts towards the
  crate's size on disk and can still be reached with `--all-features`.
- **Future**: A `gup-debug` crate (strategic review T7) is natural once
  `gup-core` exposes a profiling hook that does not need `gup` internals.

#### The experimental crate is a non-default member, not excluded

- **Decision**: `crates/gup-culling-lod` is a workspace member that is not in
  `default-members`. It is inside `--workspace`, which the story's Success
  Metric said to avoid.
- **Reasoning**: The orchestrator asked for it to stay buildable on its own with
  a `cargo check -p` CI step. `-p` requires membership, and an excluded crate
  gets its own lockfile, so it would drift from `gup`'s dependency versions. No
  CI job used `--workspace`, so including it there costs nothing by default.
  Plain `cargo build`/`cargo test` still skip it.
- **Trade-off**: `cargo test --workspace` runs its 200 GPU tests too, and it is
  public within the workspace, so someone could depend on it. The README says
  not to.
- **Future**: RFC-001 T3/T7 work ports pieces into `gup-core` and deletes them
  here. When the crate is empty, delete it.

#### Parked crates are outside the workspace, not just non-default

- **Decision**: The five integrations are in `exclude`, not merely left out of
  `default-members`.
- **Reasoning**: Decision 3 says "exclude from workspace/CI", RFC-001 S13 says
  "back into the workspace", and the story names `--workspace` as the surface to
  clear. Exclusion also drops 190 packages from `Cargo.lock`, so
  `cargo metadata`, fetch and `--workspace` stop resolving bevy and egui.
- **Trade-off**: `cargo check -p gup-egui` no longer works. The replacement is
  `CARGO_TARGET_DIR=target cargo check --manifest-path gup-egui/Cargo.toml`,
  which resolves its own (gitignored) lockfile. `naga_oil` also left the lock:
  GUP-395 must add it to `gup-core` directly (RFC-001 dependency note 5 expected
  this).
- **Future**: S13 moves `gup-egui` and `gup-bevy` back into `members`.

#### Smoke test covers the quarantined examples

- **Decision**: `tests/examples_smoke.rs` reads examples from both `gup` and
  `gup-culling-lod`, and `mask smoke-examples` and CI build both with `-p`.
- **Reasoning**: All three moved examples were headless, so they were smoke
  tested before the move. Dropping that coverage would let the quarantined code
  rot in exactly the way the orchestrator wanted to avoid.

### Development Workflow Insights

- **Disk: put heavy builds on a different pool.** The main pool went from 7 GB
  free to 0 during the story. Two causes: `clippy -p gup-culling-lod` linted
  `gup` as a dependency with a new fingerprint, and the editor's rust-analyzer
  re-checked the workspace after every `Cargo.toml` edit. The 15-minute ZFS
  snapshots hold every superseded incremental session, so cache churn consumes
  pool space even though cargo deletes the files. `/tmp` is a separate pool
  (`turing`, 21 GB free), and all heavy builds after that point (tests, smoke,
  clippy, and the hook itself via an exported `CARGO_TARGET_DIR`) ran there with
  no further pressure on the main pool. Space returned at the 12:00 snapshot
  rotation. Check `zfs list -o avail` and `df /tmp` before choosing a target
  directory.
- **`git stash` drops staged renames from the index.** `git stash pop` restored
  the working tree but left the `git mv` renames as unstaged deletes plus staged
  adds; `git add -A <paths>` put them back. Avoid stash while a large move is
  staged; compare against `HEAD` with `git show HEAD:<file>` instead.
- **zsh array subscripts**: `"$tests[[test]]..."` in zsh is an array subscript,
  not a string concatenation. Use `${tests}`.
- **`cargo check` time** (the `gup` lib on this machine, `touch src/lib.rs` then
  re-check, median of 2–3 runs, main target): incremental 2.8 s → 2.0 s;
  non-incremental (`CARGO_INCREMENTAL=0`) 8.2 s → 5.5 s, about 30% faster. About
  25.8k lines left the default lib (13.0k culling/LOD, 12.8k debug/performance).
- **CI footprint**: no default workflow built the parked crates before, so the
  per-PR saving is that iOS and Android CI no longer trigger on
  `src/platform/**` or their crates' paths (macOS runner plus simulator, and NDK
  plus emulator). Visual regression gains `cargo check` and `cargo test` for
  `gup-culling-lod` (about 10 s of checking and 10 s of tests locally, plus
  compile time). `Cargo.lock` is 2,458 lines shorter.
- **Not done: a scheduled "parked crates still compile" job** (Risk Assessment).
  The owner parked these crates precisely so they do not have to keep compiling
  while RFC-001 replaces `ComposedChart`/`DynChart`. A job that turns red as
  soon as S7 lands would be noise. S13 rewrites them anyway.
- **Unverified**: the `android-ci.yml` change runs `cargo ndk ... build` from
  `working-directory: gup-android` with `CARGO_TARGET_DIR` set. There is no NDK
  here, and the workflow is manual-dispatch only, so the first manual run is the
  test.

### Follow-up Stories

1. **GUP-398: Honest Clippy Gate**: make `mask all-check` fail on lints (strict
   clippy without `--fix`, all workspace members including `gup-culling-lod` and
   `gup-core`) and fix or narrowly allow the existing debt.

Notes, no new story:

- **GUP-395** should add `naga_oil` (0.20, naga 27) to `gup-core` as a direct
  dependency; it is no longer in `Cargo.lock`.
- The root README's "Project Structure" tree (`src/core/`, `src/gpu/`, ...) is
  stale and predates this story. RFC-001 S14 rewrites the docs.
