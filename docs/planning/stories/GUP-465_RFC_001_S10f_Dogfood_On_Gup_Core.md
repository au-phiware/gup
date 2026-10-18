---
implementer: sonnet
implementer-reason:
  rewrites user-level task binaries on the S10 builders; the manifest rules
  (what may move, what may not) and each task's port are fixed in Decisions
---

# GUP-465: RFC-001 Step S10f: Dogfood Tasks on `gup-core`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-18

## Context

RFC-001 S10's exit criterion is "dogfood tasks 1–3 pass". The dogfood crate
(`dogfood/`, GUP-394) is a detached external crate. It runs realistic tasks
against the public API in CI and checks their PNGs against a manifest
(`dogfood/src/suite.rs`) with tracked gaps: XFAIL is expected, XPASS fails the
build until the gap is removed. Its README says: when the new core replaces the
API, rewrite the task binaries, and the manifest's bar stays put.

Seven task binaries use the old `scatter()`, `line()` or `bar()` builders:
`t0_smoke`, `t1_timeseries`, `t2_bars`, `t3_scatter_png`, `t3_scatter_window`,
`t4_linked` and `t5_live`. GUP-466 deletes those builders, so all seven move to
`gup-core` here: the class, not only tasks 1–3. `ref_export_png`, `t5_stream`
and `t6_*` use the raw old `Selection`, `ComposedChart` or the old macro. They
stay until RFC-001 S12 and S14.

What the new core should turn from XFAIL into PASS in tasks 0–3:

- `NO_TEXT` (S2/S3), `DOUBLE_GAMMA` (S3 colour policy) and `NO_LEGEND` (S7c);
- `NO_GRID` (GUP-460), `NO_GROUPING` and `NO_STACKING` (GUP-464), and
  `CATEGORICAL_GREY` (S5b).

`SVG_NO_MARKS` stays: `SvgTarget` writes guides only, and no story schedules
vector marks.

## Read First

- `dogfood/README.md` (whole) and `dogfood/src/suite.rs` lines 1–340 (types,
  gaps, palette) and the task functions you change.
- The seven task binaries in `dogfood/src/bin/` and `dogfood/src/lib.rs`
  (`ChartTexture`, `plot_frac`, `rasterise_svg`).
- [GUP-462](GUP-462_RFC_001_S10c_Chart_Builders_And_Scatter.md),
  [GUP-463](GUP-463_RFC_001_S10d_Line_Builder.md) and
  [GUP-464](GUP-464_RFC_001_S10e_Bar_Builder.md) Decisions (the APIs), and
  [GUP-460](GUP-460_RFC_001_S10a_Grid_Lines.md) (`grid`).
- `crates/gup-core/src/app.rs` lines 25–90 (`GUP_SCREENSHOT_PATH`,
  `GUP_SCREENSHOT_POINTER`, `GUP_SCREENSHOT_WIDTH`/`HEIGHT`).
- `.github/workflows/dogfood.yml`.

## User Story

> "As the project, I want the dogfood tasks to run on the new core with no
> workarounds, so that S10's exit criterion is checked on every push, by code a
> user would write."

## Acceptance Criteria

### AC1: Tasks 0–3 pass

- [ ] `dogfood/Cargo.toml` adds `gup-core = { path = "../crates/gup-core" }`.
      `t0_smoke`, `t1_timeseries`, `t2_bars`, `t3_scatter_png` and
      `t3_scatter_window` import only `gup_core` (and non-Gup crates).
- [ ] `xvfb-run -a ./dogfood/run_all.sh` reports PASS for every check of tasks
      0–3, except `t3_scatter_png`'s "SVG contains the data points", which is
      XFAIL with its reworded gap (Decisions). There is no FAIL and no XPASS.
      Paste the summary into the Implementation Summary.
- [ ] Each of these gaps is used by no task 0–3 check, and is deleted from
      `suite.rs` if no other task uses it: `NO_TEXT`, `DOUBLE_GAMMA`,
      `NO_LEGEND`, `NO_GRID`, `NO_GROUPING`, `NO_STACKING`, `CATEGORICAL_GREY`.
- [ ] No task 0–3 binary computes a domain, a plot rect, a legend, an SVG
      element or a colour mapping by hand.
      `grep -n "SvgElement\|plot_frac\|rasterise_svg\|fold((f32::MAX"` over
      those five binaries is empty.

### AC2: The bar did not move

- [ ] No check's threshold (`min_px`, `min_fraction`, `min`, `max`, `tol`,
      `max_px`) is lowered in any task.
- [ ] Every region that moved (because the new layout puts, for example, the
      legend at the right) is listed in the Implementation Summary as old → new.
      Each new region was taken from a fresh render's `Layout` (plot rect,
      legend bounds), not tuned until the check passed.
- [ ] Workaround outputs (`t1_svg.png`, `t2_grouped_wa.png`,
      `t2_stacked_wa.png`) are no longer written. Each of their checks is either
      covered by an equal or stricter plain check, or moved onto the plain
      output with the same measure. For example, "Q1 stack shows red layer"
      moves to `t2_stacked.png`. List the mapping.

### AC3: Tasks 4 and 5 (live) on gup-core

- [ ] `t4_linked` and `t5_live` import only `gup_core` (and egui, eframe and
      non-Gup crates). `ChartTexture` renders any `gup_core::Chart` through
      `ChartExt::render` into an egui texture.
- [ ] Both pass their existing checks under `xvfb-run` (regions re-derived as in
      AC2).

### AC4: Docs

- [ ] `dogfood/README.md` describes each ported task as it now is: what works,
      what is still a gap (task 2's value labels, task 5's missing append), and
      what was dropped (task 3's `$Nk` tick format).

## Decisions

- **Dependency**: until the S14 flip, the tasks import `gup_core::…`
  (`gup_core::scatter`, `gup_core::show`, `gup_core::prelude::*`). At S14 that
  becomes `gup::…` with no other change.
- **Colours**: the tasks configured the 10-colour palette their manifest checks
  (`BLUE`, `ORANGE`, …), so they pass it explicitly:
  `.color_scale(Categorical::from_palette([..]))`. The manifest's colours do not
  change to Okabe-Ito.
- **Per task**:
  - `t0_smoke`: `gup_core::scatter(points).x(..).y(..).title(..)`, then
    `save_png` at the same size as before.
  - `t1_timeseries`:
    - `gup_core::line(rows)` with `.x(|r| r.t)`, Unix seconds as `f64`, and
      `.x_scale(Time::new())`;
    - `.y(|r| r.price)`, with `price: Option<f64>`;
    - `.color(|r| r.ticker.as_str())` with the palette;
    - `.width(1.5)`, `.grid(Axes::Y)`, `.title(..)`.

    The SVG workaround is deleted.

  - `t2_bars`: Part A becomes `gup_core::bar` with `.group(..)` and then
    `.stack(..)` on the region, the palette and titles. Part B (the workaround)
    is deleted. Value labels are not available and are recorded in the README,
    unchecked as before.
  - `t3_scatter_png`:
    - `gup_core::scatter` with `.x_scale(Log::new())`,
      `.color(|p| p.segment.as_str())` with the palette, `.size(|p| p.weight)`,
      `.opacity(0.35)`, `.grid(Axes::Both)` and the title, then `save_png`;
    - the SVG is `SvgTarget` over `Scene::guides` of the same resolve, because a
      scene with marks is an error.
  - `t3_scatter_window`: the same chart with `.tooltip(..)`, shown by
    `gup_core::show`. With `DOGFOOD_AUTO=1`, the binary resolves the chart
    headless first, takes a row's `pick_anchor`, and sets `GUP_SCREENSHOT_PATH`,
    `GUP_SCREENSHOT_POINTER` (that anchor), `GUP_SCREENSHOT_WIDTH=1000` and
    `GUP_SCREENSHOT_HEIGHT=700`. It does this with
    `unsafe { std::env::set_var }` before any thread starts, with a `// SAFETY:`
    comment. egui is no longer used in this task.
  - `t4_linked`: two `ScatterChart`s in eframe through `ChartTexture`. egui
    pointer events go to the left chart's `Chart::handle` as `Input`, in
    texture-local logical pixels. The left chart uses `Drag::Brush`. Its
    `on_brush` handler writes the brushed customer ids to the shared state. The
    right chart is rebuilt from the rows, as `Clone` user data, whenever the
    selection changes: grey for all customers and a coloured layer for the
    selected ones. `DOGFOOD_AUTO` scripts the brush drag as today.
  - `t5_live`: the line and the 100k scatter are rebuilt from their rows every
    tick. This matches today's full re-upload; RFC-001 S12's `appender` will
    replace it. The README records it.

- **Reworded gap**: `SVG_NO_MARKS` becomes "SvgTarget writes guides only; data
  marks in vector output are not scheduled (RFC-001 §7)".
- **Unchanged**: `ref_export_png`, `t5_stream` and `t6_*` binaries and their
  manifest entries.

## Escalation Triggers

- A task 0–3 check fails, and passing it would need a gup-core change beyond
  GUP-460–464, or a lowered threshold.
- A check XPASSes in `ref_export_png`, `t5_stream` or `t6_*` (they should not
  change).
- `t3_scatter_window`'s screenshot shows no tooltip at the chosen anchor.
- The 200k-row task 3 takes longer than its `DOGFOOD_TIMEOUT_SECS` (300 s).

## Out of Scope

- Deleting the old builders and porting in-repo examples and tests: GUP-466.
- Vector marks in SVG, value labels, `appender`: not scheduled, not scheduled,
  and RFC-001 S12.
- `gup-egui` (RFC-001 S13). `ChartTexture` is dogfood user code.

## Technical Tasks

- [ ] `Cargo.toml`; `ChartTexture` over `gup_core::Chart`.
- [ ] Port t0, t1, t2, t3a, t3b; run; re-derive regions; update gaps.
- [ ] Port t4 and t5_live; run under `xvfb-run`.
- [ ] README; the AC1–AC2 evidence in the Implementation Summary.

## Dependencies

### Prerequisite Stories

- GUP-460: RFC-001 S10a Grid Lines 📋.
- GUP-462: RFC-001 S10c Chart Builders and Scatter 📋.
- GUP-463: RFC-001 S10d Line Builder 📋.
- GUP-464: RFC-001 S10e Bar Builder 📋.

### Enables Stories

- GUP-466: nothing in `dogfood/` uses the old scatter, line or bar builders.

## Testing Strategy

- **End to end**: `xvfb-run -a ./dogfood/run_all.sh` locally; the Dogfood
  workflow in CI.
- **Visual**: the implementer reads `t1.png`, `t2_grouped.png`,
  `t2_stacked.png`, `t3_scatter.png` and `t3_window.png`.

## Success Metrics

- [ ] Tasks 1–3 PASS with no tracked gap except vector marks.
- [ ] The five task 0–3 binaries shrink: record total lines before and after.

## Risk Assessment

- **Medium**: re-deriving regions could be used to dodge a real failure.
  _Mitigation_: AC2 requires regions from `Layout` and lists every move.
- **Low**: egui/eframe event mapping in t4. _Mitigation_: the same pattern
  `t3_scatter_window` used before; `DOGFOOD_AUTO` is scripted.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS)
- [ ] `cargo test --manifest-path dogfood/Cargo.toml` passes (manifest unit
      tests)
- [ ] Lint and format clean: `mask all-fix`
- [ ] Rendered output verified by eye: the five PNGs above
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
