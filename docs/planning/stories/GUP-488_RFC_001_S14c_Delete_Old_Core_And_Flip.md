---
implementer: sonnet
implementer-reason:
  deletion of a fixed file set plus a one-line facade; every CI, mask and
  manifest edit is listed in Decisions; broad but mechanical
---

# GUP-488: RFC-001 Step S14c: Delete the Old Render Core and Flip `gup`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-24)
**Created**: 2026-10-19

## Context

This is the flip RFC-001 has been building towards: `gup` becomes `gup-core`'s
public crate, and the old render path is deleted rather than left frozen beside
it (orchestrator review point 1: "systems running side by side never get
deleted").

After GUP-486 (S14b), the old path that `mask old-path-loc` measures is the
render core: `selection.rs`, `mark/`, `shader_function/`, `shader_pipeline.rs`,
`context.rs` and `render.rs` (18,504 lines at `d133343`). Almost everything else
in `src/` depends on them: of the 57 top-level modules, all but a handful of
leaves (`math`, `zoom`, `spatial_index`, `tick_generator`, `color_descriptor`,
`depth`, `lighting`, `gpu_timer`, the `performance*` reports) import the old
core directly or through another module. The leaves have no caller outside the
old path and no place in `gup-core`'s API, and `gup-core` already has its own
scales, ticks, picking and timing. So the flip deletes `src/` down to a facade
`lib.rs`, together with the old `gup-macros` crate and the examples, tests and
benches that use any of it. Git history keeps all of it.

By this point every user of the old path has moved: dogfood (GUP-465, GUP-480,
GUP-485), the in-repo examples that matter (GUP-466), `gup-egui` and `gup-bevy`
(GUP-483, GUP-484), and the macros resolve `gup` (GUP-487). One workspace member
still uses the old core: the quarantined `crates/gup-culling-lod` (`GupContext`,
`RenderContext`, `mark::Rectangle`, `streaming`). Decisions parks it.

## Read First

- RFC-001 §11 (lines 744–792) and the orchestrator review point 1 (lines
  917–929).
- [GUP-486](GUP-486_RFC_001_S14b_Delete_Old_Chart_Builder.md) "Decisions" (the
  deletion rule) and its Implementation Summary.
- Root `Cargo.toml` (whole: workspace, dependencies, features, targets).
- `src/lib.rs` (to be replaced) and `crates/gup-core/src/lib.rs` lines 80–165
  and 295–302 (`pub use wgpu`, the prelude).
- `maskfile.md` task list (`grep -n '^## ' maskfile.md`) and
  `.github/workflows/{wasm,tests,performance,visual-regression,gallery}.yml`.
- `scripts/clippy_wasm32.sh` (`excluded`), `scripts/old_path_loc.pl`,
  `scripts/test_debug_feature.sh`.
- `dogfood/Cargo.toml` and README "Running it".
- The strategic review's Decisions 1 and 3 (culling/LOD crate, parked crates).

## User Story

> "As a user, I want `gup` to be the new core, so that
> `gup::scatter(rows).x(..).y(..)` is the library, with no second API beside
> it."

## Acceptance Criteria

### AC1: The facade

- [x] `src/` holds only `lib.rs`: the copyright header, crate docs with the
      north-star example as a doctest (Decisions), and `pub use gup_core::*;`.
- [x] Root `Cargo.toml`: `[lib] crate-type = ["rlib"]`; `[dependencies]` is
      `gup-core` only; `[dev-dependencies]` only what `tests/examples_smoke.rs`
      and the remaining examples use; `[features]` is empty or gone; the
      `[[example]]`, `[[test]]` and `[[bench]]` lists match what is left.
- [x] `gup-macros/` is deleted and out of `members`.
- [x] A second doctest in `lib.rs` defines a colour function with
      `gup::wgsl_function` and renders a layer with it, proving GUP-487's `gup`
      row from a crate that depends only on `gup` (doctests are external
      crates).

### AC2: Users renamed

- [x] The five gup-core examples (`hello_world`, `basic/01_hello_chart`,
      `basic/03_line_chart`, `basic/04_bar_chart`, `composite_mixed_data`) use
      `gup::` and run under `examples_smoke`; read their screenshots.
- [x] `dogfood/Cargo.toml` depends on `gup` (path) and not on `gup-core`; every
      `gup_core::` in `dogfood/` is `gup::`;
      `xvfb-run -a     ./dogfood/run_all.sh` passes with no FAIL, XPASS or
      XFAIL.
- [x] `gup-egui` and `gup-bevy` keep depending on `gup-core` (Decisions) and
      still build and test.

### AC3: Deleted, with gates in step

- [x] Every root `examples/`, `tests/` and `benches/` file that used the old
      path is deleted; `examples/INDEX.md`, `examples/README.md`,
      `scripts/gallery_config.toml` and `tests/visual_regression/*.toml` list
      only what is left; `scripts/check_gallery_sync.sh` passes. List counts in
      the Implementation Summary.
- [x] CI and mask edits exactly as in Decisions; `actionlint` passes;
      `mask     all-check` passes; `mask ci` passes locally where it can run.
- [x] `crates/gup-culling-lod` is parked as in Decisions.

### AC4: The metric is zero

- [x] `mask old-path-loc` prints `TOTAL 0`; paste it. Then
      `scripts/old_path_loc.pl` and the `old-path-loc` task are deleted, and
      INDEX.md's "RFC-001 Migration" header says "Old-path LOC: 0 (GUP-488,
      <date>); the metric is retired."
- [x] RFC-001 gains "S14 findings (GUP-485–488)": what was deleted (files and
      lines per step, from git), what was parked, and that the remaining
      builders are T5.

## Decisions

- **Owner decisions (2026-10-19)**: park `crates/gup-culling-lod` (out of the
  workspace and CI, code kept frozen in the tree; GUP-457 replaces its culling),
  and approve the full flip scope: delete nearly all of the old `src/` tree,
  `performance.yml` and the old `gup-macros`, ending at `mask old-path-loc` 0.
  Every deletion step keeps CI and dogfood green.

- **Facade**: `pub use gup_core::*;`. The whole public surface of `gup-core`
  (its root items, `prelude`, `wgpu`, the macros, the doc-hidden `__private` the
  macros need) becomes `gup`'s. Reason: one curated surface, maintained in
  `gup-core`; the RFC's "re-exports the gup-core prelude" is a subset of this.
- **North-star doctest**:

  ```rust
  use gup::prelude::*;
  struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }
  let data = vec![/* three rows */];
  let mut chart = gup::scatter(data)
      .x(|d| d.gdp).y(|d| d.life_exp)
      .color(|d| d.continent.as_str())
      .size(|d| d.population);
  let image = chart.render(&Context::shared()?, 800, 500)?;
  assert_eq!(image.dimensions(), (800, 500));
  ```

- **`crate-type = ["rlib"]`**: `cdylib` existed for `wasm-pack` of the old
  `wasm_api`; the browser path is `gup-core`'s harness.
- **Integration crates keep `gup-core`**: same types, fewer crates to build;
  their docs say either works.
- **`gup-culling-lod`**: removed from `members`, added to `exclude`, its CI
  steps (`visual-regression.yml` lines 65–71) and `smoke-examples`'s
  `-p gup-culling-lod` removed, its `clippy_wasm32.sh` entry removed, and its
  README says "Parked (GUP-488): built against the pre-flip `gup` at
  `<last commit before this story>`; T7 ports what it needs onto gup-core".
  Reason: strategic review Decision 1 keeps it as reference code for T7, not as
  a build; porting it is T7 work. **The orchestrator must confirm this before
  the story starts** (see the escalation trigger).
- **CI and mask edits** (authorised for this story only):
  - `wasm.yml`: keep "Check WASM library compilation" (the facade builds for
    wasm32); delete the `wasm-pack build`, "Verify package output", the wasm32
    test-compilation step and both `wasm-pack test` steps.
  - `tests.yml`: delete the `test_debug_feature.sh` step (and the script).
  - `performance.yml`: delete the workflow. Every job times or benchmarks
    deleted code; `gup-core`'s budgets are `mask perf-budget` and the
    deterministic CI proxies already in place (GUP-417).
  - `visual-regression.yml`: the culling-lod steps above;
    `cargo build --examples --all-features -p gup -p gup-culling-lod` becomes
    `-p gup`.
  - `maskfile.md`: delete `validate-marks` (and its use in `all-check`),
    `bench`, `bench-interaction`, `bench-wasm-native`, `bench-wasm-build`,
    `bench-wasm-serve`, `perf-check`, `perf-alert`, `perf-trend-record`,
    `perf-trend-report`, `test-tutorials` and `old-path-loc` (after AC4);
    `smoke-examples` loses `-p gup-culling-lod`. Leave `tauri-example` and
    `ios-build` (parked crates, T7).
- **Tutorials**: no longer compiled (the `tutorial_doctests` module goes with
  the old `lib.rs`); they keep GUP-466's "out of date until T6" note.
- **Parked mobile and Tauri crates** are not touched; they stay excluded.

## Escalation Triggers

- **Before starting**: the orchestrator has not confirmed the `gup-culling-lod`
  decision.
- Anything outside the deletion set needs a code change other than `gup_core` →
  `gup` renames (for example, a gup-core test that reaches into the old path).
- `cargo build --target wasm32-unknown-unknown --lib` fails for the facade.
- `pub use gup_core::*` re-exports something that collides with a name the
  facade adds (it should add none).
- Dogfood or an integration crate fails after the rename.

## Out of Scope

- Revising TECHNICAL_APPROACH, IMPLEMENTATION_STRATEGY and the root README:
  GUP-489.
- Re-adding area, boxplot, violin, density, heatmap and choropleth: T5.
- Mobile, Tauri and culling/LOD on gup-core: T7.

## Technical Tasks

- [x] Record the last pre-flip commit for the culling-lod README.
- [x] Delete `src/*` except `lib.rs`, `gup-macros/`, old examples, tests,
      benches, `src/bin`; write the facade and its doctests.
- [x] Root `Cargo.toml`: deps, dev-deps, features, targets, members.
- [x] Rename the five examples and dogfood to `gup::`; run both.
- [x] Park `gup-culling-lod`; CI, mask and script edits; `actionlint`.
- [x] `old-path-loc` to 0, then retire it; INDEX header; RFC findings.

## Dependencies

### Prerequisite Stories

- GUP-486: RFC-001 S14b Delete Old Chart Builder 📋.
- GUP-487: Macros Resolve Crate Path 📋.
- GUP-483, GUP-484: gup-egui and gup-bevy on gup-core 📋 — no workspace crate
  but `gup-culling-lod` uses the old core.
- GUP-485: dogfood residue 📋 (through GUP-486).

### Enables Stories

- GUP-489: S14d architecture docs.
- T5 builder stories (area, boxplot, …) on `gup`.

## Testing Strategy

- **Build and test**: `cargo test --workspace -- --test-threads=1`,
  `cargo test --doc`, `cargo check --examples --tests`, `examples_smoke`
  (ignored), the wasm32 lib build, `mask all-check`.
- **End to end**: the dogfood suite on `gup` alone.
- **Visual**: the five examples' screenshots.

## Success Metrics

- [x] Old-path LOC 0; `src/` is one file.
- [x] An external crate depending only on `gup` builds every dogfood task.

## Risk Assessment

- **Medium**: something still links an old module through a path not caught by
  grep (a workflow, a script, a doc include). _Mitigation_: `mask ci`,
  `actionlint` and the full test run before committing the CI edits.
- **Medium**: the parked culling-lod crate rots. _Mitigation_: it is reference
  code by owner decision; its README names the commit to build it at.
- **Low**: trybuild `.stderr` churn: none expected (gup-core sources are not
  edited); the `gup` doctests are new, not snapshots.

## Definition of Done

- [x] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test --workspace -- --test-threads=1`
- [x] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS, no XFAIL)
- [x] Lint and format clean: `mask all-fix`, then `mask all-check`
- [x] All examples compile: `cargo check --examples`
- [x] `mask wasm-browser` passes (this story changes the wasm build and the
      workflows)
- [x] `mask wasm-size` run and the figures reported in the Implementation
      Summary (expected unchanged; the orchestrator re-records)
- [x] Rendered output verified by eye: the five examples' screenshots
- [x] `mask old-path-loc` TOTAL 0 recorded before the task is retired
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Six commits from `06a5cef8` (the last pre-flip commit): start, park, delete old
examples/tests/benches/bins/perf workflow, the flip, retire the metric, and this
summary.

- **Facade**: `src/lib.rs` is the header, two doctests (the north-star scatter;
  a `#[gup::wgsl_function]` colour ramp, with `path = "ramp_demo"` because a
  doctest in package `gup` would otherwise land in the reserved `gup::`
  namespace) and `pub use gup_core::*;`. Root `Cargo.toml`: `rlib`, dependency
  on `gup-core` only, dev-dependencies for `examples_smoke`, no `[features]`.
- **Deleted** (counts from git): 200 `src/` files (132,465 lines), 114 `tests/`
  files (35,230), 21 `benches/` files (4,765), 74 `examples/` files (27,307),
  `gup-macros`, `src/bin`, `assets/`, `performance.yml`, nine perf, benchmark
  and debug-feature scripts, `perf-thresholds.toml` and stray generated files
  (`rust_out`, CSV/JSON/DOT dumps). 477 files, 208,932 lines in all.
- **Kept**: five examples, `tests/examples_smoke.rs`, `tests/golden/`,
  `tests/visual_regression/*.toml` (the skip list is now empty; the smoke test
  accepts a list with no `[[skip]]` tables), and `gallery_config.toml` /
  `examples/INDEX.md` / `examples/README.md` cut to match.
- **Renamed**: the five examples and `dogfood/` use `gup::`; `dogfood` depends
  on `gup` (path) alone. `gup-egui` and `gup-bevy` keep `gup-core`.
- **Parked**: `crates/gup-culling-lod` (workspace `exclude`, CI steps and
  `smoke-examples` flag gone, README names `06a5cef8`).
- **CI and mask**: as in Decisions; also `mask ci performance`, the `ci wasm`
  wasm-pack lines, `scripts/pre_commit.sh`'s validate-marks hook and the
  `test_pre_commit.sh` cases, and `scripts/clippy_wasm32.sh` entries for the
  deleted crates. `.github/workflows/README.md` was rewritten without the
  performance sections.
- **Metric**: `mask old-path-loc` printed `TOTAL 0` after the flip; the task and
  `scripts/old_path_loc.pl` are deleted; INDEX header and RFC-001 "S14 findings
  (GUP-485-488)" written.

### Evidence

- `cargo test --workspace --exclude gup-bevy -- --test-threads=1` under Xvfb:
  592 passed, 0 failed. `gup-bevy` tests pass. `cargo test --doc -p gup`: 2
  passed. `mask smoke-examples`: 3 passed (all five examples run headless).
- Dogfood (lavapipe, Xvfb): 10 pass, 0 XFAIL, 0 FAIL, 0 XPASS.
- `mask all-check`: all seven checks exit 0; `mask ci wasm` and
  `cargo check --examples` pass; `actionlint` passes.
- `mask wasm-browser`: `GUP PASS` (Chrome/155, SwiftShader).
- `mask wasm-size`: scatter 334,149 B gz (recorded 331,601; +0.8%), over wgpu
  301,789 B (budget 400,000), per row type 1,539 B; every check passed. The
  numbers come from `gup-core`, which this story did not edit.
- Screenshots of `hello_world`, `01_hello_chart`, `03_line_chart`,
  `04_bar_chart` and `composite_mixed_data` read: titles, axes, legends, series
  colours and geometry all present and inside the plot area.
- `mask old-path-loc`: `TOTAL 0` (18,504 before).
- Not run locally: `mask ci` end to end (the Tests and Visual regression
  workflows' parts were run piecewise above); `gup-ios`, `gup-android` and the
  Tauri app (parked; they name the deleted `ios-shim` and `android-shim`
  features).

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Deleting in step with the gates

- **Challenge**: `mask all-check` and the pre-commit hook test the hook's own
  scoping (`test_pre_commit.sh`), which names workspace members, so parking a
  crate or deleting `gup-macros` fails the gate until those cases change.
- **Solution**: update the expectations in the same commit as the manifest
  change; run the script by hand first.
- **Pattern**: a workspace-shape change touches `maskfile.md`, `pre_commit.sh`,
  `test_pre_commit.sh`, `clippy_wasm32.sh`, the workflows and the smoke test
  together.

#### The smoke test classified examples by source text

- **Challenge**: after renaming `gup_core::` to `gup::`, every example timed out
  at 60 s: `examples_smoke` decided "headless-capable" by searching for the text
  `gup_core::show(`.
- **Solution**: search for `gup::show(`. A text-matching classifier is a hidden
  coupling to a rename.
- **Pattern**: rename, then run the smoke test before anything else.

#### Doctests in package `gup`

- **Challenge**: `#[gup::wgsl_function]` in a `gup` doctest was refused: the
  macro derives its path from the package name and `gup::` is reserved.
- **Solution**: `path = "ramp_demo"` in the doctest.
- **Pattern**: macros that key off `CARGO_PKG_NAME` misfire in doctests of a
  package named like the library.

### Architectural Decisions

#### An empty skip list is valid

- **Decision**: `examples_skip_list.toml` has no `[[skip]]` tables; the test
  treats a missing key as empty.
- **Reasoning**: all five surviving examples have a headless path.
- **Trade-off**: a typo in the key name now reads as "no skips".
- **Future**: windowed examples added later add entries again.

### Development Workflow Insights

- Deleting first (examples, tests, benches, workflow, scripts), then flipping,
  kept each commit green and made the big one reviewable: the flip commit holds
  `src/`, `gup-macros` and the renames only.
- Cold clippy and test builds were the slow steps; everything else was
  mechanical.

### Follow-up Stories

None. The root README and TECHNICAL_APPROACH (including the stale `debug`
feature section) are GUP-489; porting the parked crates is T7; the remaining
builders are T5.
