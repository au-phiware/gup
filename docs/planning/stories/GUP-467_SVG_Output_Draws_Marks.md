---
implementer: sonnet
implementer-reason:
  mapping per mark kind, draw order and the skip rule are fixed in Decisions;
  follows the CPU mirrors picking already uses
---

# GUP-467: SVG Output Draws Marks

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-18

## Context

`SvgTarget` (`crates/gup-core/src/svg.rs`) writes guides (axes, ticks, text,
legends) but not data marks. Its module docs say "Data marks are not written
yet", and a scene with an `ItemKind::Marks` item is an error; callers pass
`Scene::guides` to get the rest. An SVG export of a plot therefore has no data.
Dogfood task 3's "SVG has data points" check is a tracked expected failure until
this lands (GUP-465 owns the dogfood tasks).

GUP-445 added CPU read-back (`ColumnStore::cpu_values`, `cpu_eval`) that picking
uses to evaluate mark positions on the CPU. GUP-430 gave marks fill-opacity and
stroke, and GUP-431 added `DrawOrder` (`crates/gup-core/src/order.rs`). The
pieces needed to emit vector marks exist.

## Read First

- `crates/gup-core/src/svg.rs` lines 1–30 (module docs), 350–380
  (`VectorTarget for SvgTarget`, the `Marks` error).
- `crates/gup-core/src/scene.rs` lines 130–175 (`ItemKind::Marks`, `MarkBatch`).
- `crates/gup-core/src/selection.rs` (`Retain` at line 57; how picking evaluates
  CPU positions) and `crates/gup-core/src/column.rs` line 1042 (`cpu_values`).
- `crates/gup-core/src/order.rs` (`DrawOrder`, `draw_entries`,
  `polyline_spans`).
- `crates/gup-core/tests/svg.rs` and `tests/msaa.rs` (resvg rasterising and
  comparison helpers; resvg 0.45 is a dev-dependency).

## User Story

> "As a developer exporting a chart for a report, I want the SVG to contain the
> data marks as vector shapes, so that the file is a complete, scalable copy of
> what the window shows."

## Acceptance Criteria

### AC1: Marks are written

- [ ] An `ItemKind::Marks` item no longer errors in `SvgTarget`; it writes one
      `<g>` per layer with the layer's clip.
- [ ] Circle writes `<circle>`, Rect `<rect>`, Segment `<line>` with
      `stroke-linecap="square"`, Line `<polyline>` with
      `stroke-linejoin="round"` and `stroke-linecap="round"`.
- [ ] Fill colour, `fill-opacity` and stroke (width, colour) come from the
      resolved GUP-430 values; per-row colours are written per element.
- [ ] Rows are written in `DrawOrder` order; null rows (NaN position or null
      mask) are skipped, as on the GPU.

### AC2: Matches the PNG goldens

- [ ] For each existing gup-core golden scene (the PNG tests in `tests/*_png.rs`
      and `tests/msaa.rs`), a test renders the SVG, rasterises it with resvg at
      the PNG's size and compares to the PNG golden within that harness's
      existing tolerance. Scenes whose layers are skipped under AC3 are listed
      in the test with the reason and are excluded, not loosened.
- [ ] The implementer reads at least two of the rasterised SVGs (one scatter,
      one line) and confirms by eye that marks lie inside the plot area.

### AC3: Layers without CPU positions

- [ ] A layer whose mark has no CPU description (user `#[derive(Mark)]` marks)
      or whose columns are not retained (`Retain::GpuOnly`, or `Auto` above
      `Retain::AUTO_MAX_ROWS`; `Retain::Rows` drops CPU columns of uploaded
      chunks) is skipped, with an XML comment
      `<!-- layer N skipped: <reason> -->` in the SVG, not an error.
- [ ] The story retrospective lists which layers and which `Retain` modes hit
      this.

### AC4: Dogfood and size

- [ ] The dogfood task 3 "SVG has data points" expected failure is removed (grep
      for the string in `examples/` and `docs/`; if the task binary is owned by
      an unmerged GUP-465 change, remove it on whichever side exists and note it
      in the retrospective).
- [ ] The file size of the SVG for a 10 000-point scatter is reported in the
      retrospective (bytes, and gzipped).

### AC5: Docs and callers

- [ ] The `svg.rs` module docs no longer say marks are unwritten; they describe
      the skip rule. Every caller or doctest that uses `Scene::guides` only to
      avoid the error is updated or left with a comment saying why.

## Decisions

- Positions and colours are evaluated by the CPU mirrors picking uses
  (`cpu_values`/`cpu_eval`), not a new path, so SVG and picks agree.
- Pixel coordinates are written with at most 2 decimals.
- Marks are always vector; there is no raster fallback above a mark-count
  threshold in this story (the size in AC4 informs a later one).
- Skipping, not erroring, for layers with no CPU positions: an export that drops
  one layer with a comment beats none.
- Code lives in `svg.rs` (or a child module `svg/marks.rs` if it exceeds ~400
  lines); no change to `VectorTarget`'s signature unless escalated.

## Escalation Triggers

- A mark kind whose CPU positions can't be evaluated for retained columns
  (`Retain::Columns`/`GpuOnly`): decide "skip with comment" and list which
  layers (AC3); stop if more than the user-mark case hits.
- `MarkBatch` does not carry what `SvgTarget` needs (scales, columns, order), so
  `VectorTarget` must change.
- A golden differs beyond tolerance because of antialiasing, not geometry.

## Out of Scope

- A raster fallback for very large mark counts.
- Embedding fonts (already `SvgOptions::embed_font`).
- Dogfood task rewrites (GUP-465).

## Technical Tasks

- [ ] Find how picking evaluates CPU positions and expose a `pub(crate)` helper
      the SVG writer can call per layer.
- [ ] Implement per-mark emitters and draw-order iteration.
- [ ] Write the skip comment path.
- [ ] Add the golden-comparison test and the 10k size test.
- [ ] Remove the dogfood expected failure; update docs.

## Dependencies

### Prerequisite Stories

- GUP-445 ✅ — CPU read-back of columns.
- GUP-430 ✅ — fill-opacity and stroke.
- GUP-431 ✅ — `DrawOrder`.

### Enables Stories

- GUP-465: Dogfood Tasks on `gup-core` — task 3's SVG check passes.

## Testing Strategy

- **Unit tests**: per-mark element text; null skipping; skip comment.
- **Integration tests**: SVG vs PNG golden for every golden scene (AC2).
- **Visual validation**: read the rasterised SVGs named in AC2.
- **Performance**: report the 10k-point size.

## Success Metrics

- [ ] Every golden scene with CPU positions matches its PNG via resvg.
- [ ] No `ItemKind::Marks` error remains in `SvgTarget`.

## Risk Assessment

- **Medium**: resvg antialiasing differs from the GPU's, so tolerances may need
  per-scene tuning. _Mitigation_: use the existing MSAA harness tolerance and
  escalate rather than loosening.
- **Low**: large SVGs for big scatters. _Mitigation_: report only.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-10-18)

**Question**: AC2 says compare each golden scene's SVG raster to the PNG within
the harness tolerance (ΔE 3.0, 0.25% of pixels). Per pixel that fails for 9 of
17 scenes, purely from antialiasing, not geometry (Escalation Trigger 3). May
the comparison average both images over 4×4 pixel blocks first, keeping the same
ΔE 3.0 / 0.25% numbers?

**Evidence** (all 17 golden scenes, text regions masked, per-pixel pixels over
ΔE 3 of 324 000; the same scenes in 4×4 blocks):

- Per pixel: scatter 1715 (0.53%), grid 1708, rect_bars 1719, segment_mark 1703,
  line_series 1094 (0.38%), diverging_anomaly 3049 (0.94%), time_years 3368
  (1.04%), time_tick_hierarchy 1777 (0.82%), categorical_nulls 1174 fail;
  composite, legend (2), band_months, categorical_species, sqrt, symlog and
  axis_titles pass. Max per-pixel ΔE 6.6 to 19.3, mean ΔE under 0.08.
- Geometry matches: a row through a scatter circle's centre differs by 5/255 at
  the edge pixels only; colours are identical. The cause is resvg (tiny-skia)
  quantising vertical coverage to quarter steps: the Apr bar's top edge at y =
  101.65 has exact coverage 0.35, the GPU draws 0.353, resvg 0.247.
- In 4×4 blocks every scene passes the default tolerance: at most 20 of 20 160
  blocks (0.10%, segment_mark) exceed ΔE 3; max block ΔE 4.6.
  `tests/common/svg_marks.rs` implements it (`BLOCK`); a mark moved by more than
  about 0.3 px, a wrong colour or a missing mark still fails.

**Options**: (A) block-averaged comparison, same tolerance (recommended; the
code is in this commit); (B) per-pixel with `max_differing_fraction` of about
1.5%, which a 1 px shift of every mark could also pass; (C) compare per-mark ink
totals only.

**Done so far**: vector emission for all four marks (`src/vector.rs`,
`src/svg.rs`), CPU colour mirrors (`ShaderFn::cpu_color`, macro and scales),
`Arc`-shared chunk bytes (`CpuColumns`), golden comparison wired into every
`*_png.rs` test. **Not done**: unit tests, 10k size report, docs sweep for
`Scene::guides` callers, dogfood grep, `msaa.rs` scene, checks and retro.
