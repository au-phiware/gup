---
implementer: sonnet
implementer-reason:
  deletion of a fixed file set plus a one-line facade; every CI, mask and
  manifest edit is listed in Decisions; broad but mechanical
---

# GUP-488: RFC-001 Step S14c: Delete the Old Render Core and Flip `gup`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-19

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

- [ ] `src/` holds only `lib.rs`: the copyright header, crate docs with the
      north-star example as a doctest (Decisions), and `pub use gup_core::*;`.
- [ ] Root `Cargo.toml`: `[lib] crate-type = ["rlib"]`; `[dependencies]` is
      `gup-core` only; `[dev-dependencies]` only what `tests/examples_smoke.rs`
      and the remaining examples use; `[features]` is empty or gone; the
      `[[example]]`, `[[test]]` and `[[bench]]` lists match what is left.
- [ ] `gup-macros/` is deleted and out of `members`.
- [ ] A second doctest in `lib.rs` defines a colour function with
      `gup::wgsl_function` and renders a layer with it, proving GUP-487's `gup`
      row from a crate that depends only on `gup` (doctests are external
      crates).

### AC2: Users renamed

- [ ] The five gup-core examples (`hello_world`, `basic/01_hello_chart`,
      `basic/03_line_chart`, `basic/04_bar_chart`, `composite_mixed_data`) use
      `gup::` and run under `examples_smoke`; read their screenshots.
- [ ] `dogfood/Cargo.toml` depends on `gup` (path) and not on `gup-core`; every
      `gup_core::` in `dogfood/` is `gup::`;
      `xvfb-run -a     ./dogfood/run_all.sh` passes with no FAIL, XPASS or
      XFAIL.
- [ ] `gup-egui` and `gup-bevy` keep depending on `gup-core` (Decisions) and
      still build and test.

### AC3: Deleted, with gates in step

- [ ] Every root `examples/`, `tests/` and `benches/` file that used the old
      path is deleted; `examples/INDEX.md`, `examples/README.md`,
      `scripts/gallery_config.toml` and `tests/visual_regression/*.toml` list
      only what is left; `scripts/check_gallery_sync.sh` passes. List counts in
      the Implementation Summary.
- [ ] CI and mask edits exactly as in Decisions; `actionlint` passes;
      `mask     all-check` passes; `mask ci` passes locally where it can run.
- [ ] `crates/gup-culling-lod` is parked as in Decisions.

### AC4: The metric is zero

- [ ] `mask old-path-loc` prints `TOTAL 0`; paste it. Then
      `scripts/old_path_loc.pl` and the `old-path-loc` task are deleted, and
      INDEX.md's "RFC-001 Migration" header says "Old-path LOC: 0 (GUP-488,
      <date>); the metric is retired."
- [ ] RFC-001 gains "S14 findings (GUP-485–488)": what was deleted (files and
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

- [ ] Record the last pre-flip commit for the culling-lod README.
- [ ] Delete `src/*` except `lib.rs`, `gup-macros/`, old examples, tests,
      benches, `src/bin`; write the facade and its doctests.
- [ ] Root `Cargo.toml`: deps, dev-deps, features, targets, members.
- [ ] Rename the five examples and dogfood to `gup::`; run both.
- [ ] Park `gup-culling-lod`; CI, mask and script edits; `actionlint`.
- [ ] `old-path-loc` to 0, then retire it; INDEX header; RFC findings.

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

- [ ] Old-path LOC 0; `src/` is one file.
- [ ] An external crate depending only on `gup` builds every dogfood task.

## Risk Assessment

- **Medium**: something still links an old module through a path not caught by
  grep (a workflow, a script, a doc include). _Mitigation_: `mask ci`,
  `actionlint` and the full test run before committing the CI edits.
- **Medium**: the parked culling-lod crate rots. _Mitigation_: it is reference
  code by owner decision; its README names the commit to build it at.
- **Low**: trybuild `.stderr` churn: none expected (gup-core sources are not
  edited); the `gup` doctests are new, not snapshots.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test --workspace -- --test-threads=1`
- [ ] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS, no XFAIL)
- [ ] Lint and format clean: `mask all-fix`, then `mask all-check`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-browser` passes (this story changes the wasm build and the
      workflows)
- [ ] `mask wasm-size` run and the figures reported in the Implementation
      Summary (expected unchanged; the orchestrator re-records)
- [ ] Rendered output verified by eye: the five examples' screenshots
- [ ] `mask old-path-loc` TOTAL 0 recorded before the task is retired
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
