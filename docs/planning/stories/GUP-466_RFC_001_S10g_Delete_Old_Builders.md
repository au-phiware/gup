---
implementer: sonnet
implementer-reason:
  mechanical once GUP-462–465 land; what to delete, port and keep, the migration
  table and the LOC target are fixed in Decisions
---

# GUP-466: RFC-001 Step S10g: Delete the Old Scatter, Line, Bar and Composite Builders

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-19)
**Created**: 2026-10-18

## Context

RFC-001's orchestrator review made "LOC remaining in the old path" the metric
that must trend to zero, so that the old and new paths do not ossify side by
side. `mask old-path-loc` measures 28,910 at `b3f87b5` (2026-10-18). With
GUP-462–464, `gup-core` has `scatter`, `line` and `bar` builders. With GUP-465,
no dogfood task uses the old ones. This story deletes them and migrates the
remaining in-repo users.

The old composite builder (`src/chart_builder/builders/composite.rs`, 782 LOC)
exists to combine exactly these three builders
(`ChartLayer::{Scatter, Line, Bar}`), so it goes too. Composing layers is
already `Plot`'s job: mixed row types, shared scales and z order are there since
S7a. RFC-001 S11 keeps secondary y and the deletion of `ComposedChart` and
`DynChart`, which the remaining old builders (area, boxplot, violin, density,
heatmap, choropleth) still return. Those wait for the S14 flip.

## Read First

- RFC-001 §11 "Story sequence" rows S10, S11 and S14 (lines 768–792) and the
  orchestrator review point 1 (lines 917–929).
- `scripts/old_path_loc.pl` (the metric) and INDEX.md's "RFC-001 Migration"
  header paragraph.
- The users, from the grep below (44 files at `b3f87b5`: 7 in `dogfood/`, which
  GUP-465 handles; 6 in the parked `gup-egui`/`gup-bevy`; and a false positive,
  `span.line()` in `crates/gup-core-macros/src/lib.rs`):

  ```bash
  git grep -lE '\b(scatter|line|bar)\(\)|ScatterPlotBuilder|LineChartBuilder|BarChartBuilder|CompositeChartBuilder|composite\(\)' -- '*.rs'
  ```

- `tests/examples_smoke.rs` lines 1–60, `tests/visual_regression/*.toml`,
  `scripts/check_gallery_sync.sh` lines 1–30, `scripts/gallery_config.toml`,
  `examples/INDEX.md`.
- `src/chart_builder/visual_regression/builder_cases.rs` (the old builders'
  goldens).
- The merged `crates/gup-core/src/builder/` (GUP-462–464).

## User Story

> "As a maintainer, I want the replaced builders deleted rather than frozen, so
> that there is one way to draw a scatter, line or bar chart and the old path
> shrinks toward the S14 flip."

## Acceptance Criteria

### AC1: Deleted

- [x] `src/chart_builder/builders/{scatter,line,bar,composite}.rs` are deleted,
      with every re-export (`builders.rs`, `chart_builder.rs`, `lib.rs`,
      `prelude.rs`).
- [x] The old goldens' cases `golden_scatter`, `golden_bar`, `golden_line`,
      `golden_line_explicit_scales` and `golden_composite`, and their PNGs under
      `tests/golden/chart_builders/`, are deleted. The remaining cases (area,
      boxplot, violin, density, GPU density, heatmap, choropleth) pass
      unchanged.
- [x] `tests/composite_chart_integration.rs` and `benches/bar_chart_100k.rs`
      (and its `[[bench]]`) are deleted. In `tests/axis_integration_tests.rs`,
      `color_scale_integration.rs`, `linear_scale_integration.rs` and
      `log_scale_integration.rs`, every test that builds one of the deleted
      builders is deleted. A file left with no tests is deleted. List the
      deleted tests with counts.
- [x] Doc examples naming a deleted builder (`src/chart_builder.rs`,
      `src/chart_builder/builders.rs`, `src/grid.rs`) use `area()` instead when
      they demonstrate a trait `AreaChartBuilder` implements, else they are
      deleted. `cargo test --doc` passes.
- [x] Outside `gup-egui/` and `gup-bevy/` (parked, excluded), the Read First
      grep finds nothing but the `span.line()` false positive.

### AC2: Examples

- [x] Ported to the `gup-core` builders (Decisions), each at most 40 lines, with
      a headless path (`GUP_SCREENSHOT_PATH` through `gup_core::show`, or a PNG
      written):
  - `basic/01_hello_chart` (scatter);
  - `basic/03_line_chart` (line, with series);
  - `basic/04_bar_chart` (grouped and stacked);
  - `composite_mixed_data` (a `Plot` with circles and a line over two row
    types).
- [x] Deleted: `bar_chart`, `line_chart_demo`, `grid_visual_demo`,
      `intermediate/categorical_bar`, `intermediate/multi_series_line`,
      `intermediate/styled_scatter`, `showcase/business_dashboard`,
      `composite_bar_trend`, `composite_layer_order` and
      `composite_scatter_regression`.
- [x] `Cargo.toml` `[[example]]` entries, `examples/INDEX.md`,
      `examples/README.md`, `scripts/gallery_config.toml` and
      `tests/visual_regression/{examples_skip_list,expected_failures}.toml` are
      in step. `scripts/check_gallery_sync.sh` passes, and so does
      `cargo test --all-features --test examples_smoke -- --ignored --test-threads=1`.
- [x] The implementer reads the four ported examples' screenshots. Each shows
      its title, tick labels and data inside the plot rect, plus a legend where
      there is a colour key.

### AC3: Migration table

- [x] `crates/gup-core/src/builder/mod.rs`'s module docs gain "Migrating from
      the old builders": the table in Decisions.

### AC4: The metric went down

- [x] `mask old-path-loc` TOTAL is **at most 26,900** (from 28,910: at least
      2,010 fewer lines). Record the figure.
- [x] INDEX.md's "RFC-001 Migration" header paragraph states the new figure, the
      date and this story.
- [x] RFC-001 gains "S10 findings (GUP-460–466)": the builder API in one
      paragraph, what was deleted with the LOC delta, and that the composite
      builder's deletion moved from S11 to S10, with the reason. Also add a
      one-line note on the S11 row of §11's table.

## Decisions

- **Deleted now**: the old scatter, line, bar and composite builders; their
  goldens, tests, bench and examples; doc examples that name them. All users
  either move to `gup-core` or go.
- **Deleted later (S11)**: `ComposedChart`, `DynChart` and secondary y. They are
  the remaining builders' output types.
- **Deleted later (S14)**: everything else in the old path, including the area,
  boxplot, violin, density, heatmap and choropleth builders, `AccessorFunction`
  and `ConfigurableBuilder`. Those builders come back as T5 stories after S14
  (RFC-001 §11).
- **Parked crates**: `gup-egui` and `gup-bevy` still name the deleted builders.
  They are excluded from the workspace and CI, and S13 rewires them onto
  `gup-core`. Leave them; add one line to each crate's README saying so.
- **Examples kept vs deleted**: the four kept cover each builder once plus
  `Plot` composition. The deleted ones duplicate them, or need features no story
  schedules (dashboards of several charts, secondary y). T6's flagship gallery
  replaces the debug-demo examples.
- **Migration table**, old → new ("—" means dropped). It goes into the module
  docs as a table; written here as a list because `mdl` misreads closures' pipes
  inside table cells:
  - `scatter()` / `line()` / `bar()` → `gup_core::scatter(rows)` / `line(rows)`
    / `bar(rows)`
  - `.x(AccessorFunction::new(|d| AccessorValue::Float(d.a)))` → `.x(|d| d.a)`
  - `.build_with_data(rows, ctx)` → rows go to `scatter(rows)`; the context is
    `Context::shared()` or passed to `save_png`
  - `.x_scale(LinearScale::new(d0, d1, -1.0, 1.0))` →
    `.x_scale(Linear::new().domain(d0, d1))`; omit for an automatic domain
  - `LogScale::base10(..)` → `.x_scale(Log::new())`
  - `.color(Categorical/String)` → `.color(|d| d.key.as_str())`
  - `.color(Float)` + `.color_scale(..)` → `.color_value(|d| d.v)` +
    `.color_value_scale(Sequential::..)`
  - `.color(AccessorValue::Color(..))` per row → — (map keys through
    `.color_scale(Categorical::from_palette(..))`)
  - `.fill_color(c)` / `.stroke_color(c)` → `.fill(c)` / `.stroke(c)`
  - `.size(..)` ("percent of plot width") → `.size(|d| d.w)` (square-root area,
    2–12 px) or `.radius(px)`
  - `.point_size(px)` → `.radius(px)`
  - `.opacity(a)` / `.fill_opacity(a)` → `.opacity(a)`
  - `.stroke_width(..)` / `.stroke_width_px(px)` (line) → `.width(px)`
  - `.connect_nulls(false)` → default: nulls (`None`, NaN) break the line
  - `.connect_nulls(true)`, `.sort_x()` → — (filter or sort the rows)
  - `.interpolate`, `.smooth`, `.monotone`, `.step` → — (straight segments only)
  - `.group_by(..)` / `.stack_by(..)` / `.stack(..)` →
    `.group(|d| d.k.as_str())` / `.stack(|d| d.k.as_str())`
  - `.gap(g)` / `.bar_width(..)` → `.padding(p)` / — (the band decides)
  - `.orient(..)` / `.horizontal()` → — (vertical bars only)
  - `.anti_alias(..)`, `.point_budget(..)` → — (analytic antialiasing always;
    LOD is T7)
  - `.title(..)` → `.title(..)`
  - `.width(w)` / `.height(h)` (canvas) → the size argument of `save_png`,
    `render` or `GupApp::size`
  - `.show_axes(true)` → always on
  - `.grid()` / `.horizontal_grid()` → `.grid(Axes::Both)` / `.grid(Axes::Y)`
  - `.x_tick_format(..)` → — (`Time` gives calendar labels)
  - `.hover_reveal(..)`, `.tooltip_config(..)` → `.tooltip(|d| format!(..))`
  - `export_png(path, w, h)` → `chart.save_png(&cx, path, w, h)`
  - `render_to_rgba(w, h)` → `chart.render(&cx, w, h)`
  - `export_svg(..)` → `SvgTarget` over `Scene::guides` (marks not yet)
  - `composite().layer(a).layer(b)` → one `Plot` with `circles(..)`, `line(..)`
    and `rects(..)` layers
  - `.layer_with_y2(..)` → — until RFC-001 S11

## Escalation Triggers

- A test or doc example outside the files named in AC1 depends on a deleted
  builder in a way that is not a straight deletion.
- One of the remaining old-builder goldens changes.
- `mask old-path-loc` stays above 26,900 after AC1. Report what is left in the
  deleted builders' dependency closure, rather than deleting more.
- `examples_smoke` fails on an example this story does not touch.

## Out of Scope

- Tutorials (`docs/tutorials/01_getting_started.md`, `02_data_binding.md`) that
  show the old builders: T6's docs-as-tests rewrites them. Add a one-line "out
  of date until T6" note at the top of each.
- `ComposedChart`, `DynChart`, secondary y: RFC-001 S11.
- Horizontal bars, curves, custom tick formatters: not scheduled (see the
  table).

## Technical Tasks

- [x] Delete the four builder files and re-exports; fix compile errors by
      deletion only.
- [x] Delete the goldens' cases, PNGs, tests and bench; switch or delete the
      doctests.
- [x] Port four examples; delete ten; update the gallery, INDEX, README and skip
      lists; run `examples_smoke`.
- [x] Migration table; LOC; INDEX header; RFC-001 "S10 findings".

## Dependencies

### Prerequisite Stories

- GUP-465: RFC-001 S10f Dogfood on gup-core 📋 — no dogfood task uses the old
  builders.
- GUP-462, GUP-463, GUP-464 📋 — the replacements (through GUP-465).

### Enables Stories

- RFC-001 S11: only `ComposedChart`, `DynChart` and secondary y remain to
  replace.
- RFC-001 S14: a smaller old path to delete.

## Testing Strategy

- **Build and test**: `cargo test -- --test-threads=1`, `cargo test --doc`,
  `cargo check --examples`, `examples_smoke` (ignored tests),
  `mask visual-regression` or the old goldens' test target.
- **Visual**: the four ported examples' screenshots.

## Success Metrics

- [x] Old-path LOC at most 26,900.
- [x] One way to draw each of scatter, line and bar charts.

## Risk Assessment

- **Low**: deleting a test hides a regression in shared old code
  (`ComposedChart` render paths). _Mitigation_: the remaining old goldens still
  exercise `ComposedChart` through the other builders, and must not change.

## Definition of Done

- [x] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: the four ported examples
- [x] `mask old-path-loc` at most 26,900, recorded in INDEX.md
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- Deleted `src/chart_builder/builders/{scatter,line,bar,composite}.rs`, all
  re-exports, `range_to_ndc` (dead afterwards), five goldens' cases and PNGs,
  `tests/composite_chart_integration.rs`, `benches/bar_chart_100k.rs`, ten
  examples.
- Deleted tests: `chart_builder.rs` 7 (scatter hover/tooltip, line and bar
  hover, three render-ready/draw-marks tests); `axis_integration_tests.rs` 6;
  `color_scale_integration.rs` 2; `log_scale_integration.rs` 1;
  `linear_scale_integration.rs` 1; `builders.rs` 4 (`range_to_ndc`); five golden
  cases. No file was left empty except the deleted composite one.
- Doc examples: `builders.rs` and `grid.rs` use `area()`; `chart_builder.rs`
  module doc uses `area()`; `lib.rs` "Building a chart" example deleted.
- Ported to `gup-core` (22, 40, 36 and 35 lines): `01_hello_chart`,
  `03_line_chart`, `04_bar_chart`, `composite_mixed_data`. Screenshots read
  under Xvfb and lavapipe: title, tick labels, legend (line, bar) and data
  inside the plot rect.
- Migration table in `gup_core::builder` module docs; RFC-001 "S10 findings";
  tutorial notes; parked-crate README lines (`gup-egui/README.md` is new).
- `mask old-path-loc` TOTAL 28,910 to **26,601**. Dogfood 7 PASS, 4 XFAIL, 0
  FAIL. `mask wasm-size`: scatter 320,243 B gz (pass). `mask wasm-browser` PASS.
  `mask all-check` clean. Workspace tests 2362 lib tests pass; doc tests 203
  pass; `mask smoke-examples` passes.

## Retrospective

**Completed**: 2026-10-19

### Key Technical Learnings

#### Deleting by compile error

- **Challenge**: five files held tests that built the deleted builders, plus
  dead code left behind (`range_to_ndc`, three colour constants).
- **Solution**: delete the files, then loop on
  `cargo check --lib --tests --examples`; a small script deleted the enclosing
  `fn` of each error line. Dead-code warnings found the leftovers.
- **Pattern**: for "delete by compile error", script the enclosing-fn deletion
  and let `dead_code` find what only the deleted code used.

#### Smoke classifier and `gup_core::show`

- **Challenge**: `examples_smoke` classifies `gup_core::show(` examples as
  windowed with no headless path, so the four ported examples failed
  `skip_list_matches_examples`.
- **Solution**: followed the `hello_world` precedent and put them on the skip
  list (capture needs a display). The screenshots were read under Xvfb.
- **Pattern**: teaching the smoke test that `show` honours `GUP_SCREENSHOT_PATH`
  would let them run in `mask smoke-examples`.

### Architectural Decisions

#### Tutorial 01's full program fence

- **Decision**: changed one fence from `no_run` to `ignore` in
  `docs/tutorials/01_getting_started.md`, beyond the story's "one-line note",
  because the tutorials are compiled as doctests and the block names the deleted
  `scatter()`.
- **Trade-off**: the block is no longer compile-checked until T6 rewrites it.

### Development Workflow Insights

- `xvfb-run` defaults to a 640x480 screen; dogfood's `t4_linked` (1160x540
  window) then fails "left view inside the brush". Use
  `xvfb-run -a -s "-screen 0 1920x1080x24"`. The dogfood README only says
  `xvfb-run -a`.
- One `cargo test --doc` run failed `lib.rs - proc_macros` (E0365, `pub use gup`
  re-export) and passed on the next run with no change; not caused by this
  story.

### Follow-up Stories

None new. Teaching the smoke classifier about `gup_core::show` is under 30
minutes but changes the harness; left to the orchestrator.
