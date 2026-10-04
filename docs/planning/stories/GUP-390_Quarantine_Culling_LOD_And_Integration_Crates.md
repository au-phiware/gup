# GUP-390: Quarantine GPU Culling/LOD and Park Integration Crates

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 📋 Planned **Created**:
2026-10-04

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

- [ ] A new workspace member (e.g. `gup-experimental` or `gup-culling`) is
      created, **excluded from the default build** (not listed in the top-level
      `[workspace] members` that `cargo build`/`cargo test` without
      `-p`/`--workspace` flags would include by default — confirm the exact
      exclusion mechanism Cargo supports, e.g. a separate top-level
      `[workspace]` vs. `default-members`).
- [ ] `src/mark/compute_instance_filter.rs`, `occlusion_culler.rs`,
      `radix_sort.rs`, `unified_culling_pipeline.rs`, `src/renderer/`, and
      `src/lod/` move into the new crate with their module structure preserved,
      importing `gup` as a normal path dependency for the types they still need
      (`Mark`, buffer pool types, etc.).
- [ ] `src/mark.rs` and `src/lib.rs` no longer reference the moved modules; any
      remaining glue code needed for the move compiles.
- [ ] The new crate builds independently: `cargo check -p gup-experimental` (or
      chosen name) succeeds.
- [ ] A top-level `README.md` note (or a README in the new crate) explains why
      it exists, that it is unwired from the main render path, and that T3/T7
      are expected to draw from it.

### AC2: Debug and performance code is feature-gated

- [ ] `src/debug.rs`/`src/debug/`, `src/performance.rs`,
      `src/performance_export.rs`, and `src/performance_targets.rs` are gated
      behind a Cargo feature (e.g. `debug`), off by default.
- [ ] Any example or test that currently uses debug/performance APIs either
      enables the feature explicitly or is updated/moved accordingly — no
      example silently stops compiling with default features.
- [ ] `cargo build` (default features) does not compile the gated modules;
      `cargo build --features debug` does.

### AC3: Integration crates are excluded from the default workspace and CI

- [ ] `gup-egui`, `gup-bevy`, `gup-ios`, `gup-android`, and `gup-tauri` (the
      latter is an example app under `examples/gup-tauri/`, not a workspace
      member — confirm its actual build mechanism before deciding how to exclude
      it) are removed from the default workspace build surface used by CI
      (`cargo build --workspace`, `cargo test --workspace`).
- [ ] CI workflow files are updated so these crates are not built/tested by the
      default pipeline.
- [ ] A top-level README note lists the parked integration crates, why
      (`gup-egui`/`gup-bevy` currently fail to compile against `main`; all five
      are scheduled for re-wiring after T2), and that `cargo check -p gup-egui`
      etc. remain valid manual commands for anyone actively working on re-wiring
      them.
- [ ] This does **not** delete any of the five crates' source — only removes
      them from the default build/CI surface, consistent with "park," not
      "delete."

## Technical Tasks

- [ ] Create the new experimental crate's `Cargo.toml` and module skeleton.
- [ ] `git mv` the culling/LOD files into the new crate, fixing `use` paths.
- [ ] Update `src/lib.rs` and `src/mark.rs` to remove the moved module
      declarations (coordinate with GUP-389's concurrent edit to the same file
      if both are in flight).
- [ ] Add `#[cfg(feature = "debug")]` gates (or move into a `gup-debug` crate —
      pick one approach and justify it in the retrospective) around
      `src/debug.rs`, `src/debug/`, `src/performance*.rs`.
- [ ] Update `Cargo.toml`'s `[workspace] members` to exclude the five parked
      crates; verify `cargo build --workspace` and `cargo test --workspace` no
      longer touch them.
- [ ] Update `.github/workflows/*.yml` (or equivalent CI config) to match.
- [ ] Add the README notes from AC1 and AC3.
- [ ] Run `cargo build`, `cargo build --features debug`,
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

- [ ] `cargo build --workspace` (post-change default members) excludes the five
      parked crates and the experimental culling/LOD crate, and succeeds.
- [ ] `cargo build --features debug` succeeds and exercises the gated
      debug/performance code.
- [ ] CI run time/footprint decreases (fewer crates built by default) — report
      the before/after in the retrospective if measurable.

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

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document, including the feature-flag vs.
      separate-crate decision for debug/performance code and the before/after CI
      footprint if measured.
