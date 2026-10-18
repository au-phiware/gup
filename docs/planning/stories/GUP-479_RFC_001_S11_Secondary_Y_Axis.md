---
implementer: sonnet
implementer-reason:
  API, layout arithmetic, interaction rules and the golden scene are fixed in
  Decisions; it mirrors the left y axis code in plot.rs
---

# GUP-479: RFC-001 Step S11: Secondary Y Axis

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-19

## Context

RFC-001 §11 row S11 reads "Composite / mixed-`T` layers, secondary y; delete
`ComposedChart`/`CompositeChart`/`DynChart`". Most of it is already done or
belongs elsewhere:

- **Mixed-`T` layers** have been `Plot`'s job since S7a (GUP-424): layers of
  different row types share scales and z order. GUP-466 ports
  `composite_mixed_data` onto it.
- **`CompositeChart`** (the old composite builder) is deleted by GUP-466.
- **`DynChart`** lives in the parked `gup-egui` and `gup-bevy` crates. GUP-483
  and GUP-484 (S13) replace it with `gup_core::Chart`.
- **`ComposedChart`** is the output type of the old area, boxplot, violin,
  density, heatmap and choropleth builders, which survive until the flip.
  GUP-486 (S14b) deletes it with them.

What is left is the secondary y axis. The old composite builder had
`.layer_with_y2(..)`, and GUP-466's migration table maps it to "— until RFC-001
S11". A dual-axis chart (temperature and rainfall, price and volume) needs two y
scales with independent domains over one x axis and one plot rect.

## Read First

- RFC-001 §11 "Story sequence" row S11 (line 786).
- `crates/gup-core/src/plot.rs`:
  - the `Plot` struct and `new` (lines 120–440), including `widened`, `zoomed`,
    `home`, `dragging`, `view_axes`;
  - `x`/`y` (lines 506–526), `zoom`/`pan`/`record_zoomed`/`reset_view` (lines
    615–760), the drag-zoom rectangle (lines 1160–1210);
  - `revision_sum` and `resolve_full` up to the scene (lines 1515–1990): fit,
    `layout` closure, `set_ranges`, widening, `thin`, the hold, the axis rules
    and labels (lines 1804–1870), titles (lines 1871–1910) and legend placement
    (lines 1912–1920);
  - the `Layout` struct (lines 46–70) and its construction (line 2008).
- `crates/gup-core/tests/multi_layer_png.rs` lines 1–60 (layers encoded through
  explicit `ScaleRef` handles).
- `crates/gup-core/tests/rect_png.rs` (how bars are built from `Rect`).
- [GUP-452](GUP-452_RFC_001_S9a_Stable_Plot_Rect_During_Interaction.md)
  "Decisions" (the hold).

## User Story

> "As a chart author, I want a second y axis on the right with its own scale, so
> that two measures with different units share one x axis and one plot."

## Acceptance Criteria

### AC1: API

- [ ] `Plot::y_right` and `Plot::y_right_title` exist with the signatures in
      Decisions, each with a doctest that renders a two-axis plot (doctests
      compile as an external crate).
- [ ] `Layout::y_right_ticks: Vec<(f64, f32)>` holds the right axis's tick
      values and pixel positions, and is empty without a right axis.

### AC2: Nothing changes without a right axis

- [ ] Every existing gup-core golden passes unchanged (none re-blessed), and so
      does `mask wasm-browser`.

### AC3: The golden

A new `crates/gup-core/tests/secondary_y.rs`, golden
`tests/golden/gup_core/secondary_y.png` at 720×450:

- Twelve months, x linear 1–12, `x_title("Month")`, title "Rainfall and
  temperature".
- Rainfall bars (`Rect`, 18 px wide, from 0 to the value) on the right axis,
  `y_right_title("Rainfall (mm)")`, values 20–180.
- Temperature (`Line`, 2 px) on the left axis, `y_title("Temperature (°C)")`,
  values 8–26.

- [ ] Layout checks in the test: every right tick label's ink box lies right of
      `layout.plot.right()` and inside the canvas; every left one left of
      `plot.left()`; the right tick pixels are those of the right scale
      (`y_right.read().eval(v)`); the top of the 180 mm bar and the 26 °C line
      vertex are both within 1 px of their axes' pixels.
- [ ] The implementer reads the golden: two labelled y axes, the bars against
      the right one and the line against the left one, both titles, x ticks,
      nothing overlapping.

### AC4: Interaction moves both y axes

In `crates/gup-core/tests/secondary_y.rs`:

- [ ] A wheel zoom at the plot centre changes both y domains about the pointer:
      the data value under the pointer is unchanged on both axes (to 1e-9
      relative).
- [ ] A drag pan of `dy` pixels moves both y domains by `dy` pixels' worth.
- [ ] `view_axes(Axes::X)` leaves both y domains alone; `reset_view` restores
      both.
- [ ] `Drag::Zoom` to a rectangle sets both y domains to the rectangle's pixel
      bounds.
- [ ] A clean resolve after a zoom writes no column bytes (`upload_stats`).

### AC5: Docs and RFC

- [ ] `crates/gup-core/src/builder/mod.rs`'s migration table entry for
      `.layer_with_y2(..)` (added by GUP-466) points to `Plot::y_right`.
- [ ] RFC-001 gains "S11 findings (GUP-479)": what S11 delivered, and where each
      named deletion went (GUP-466, GUP-483/484, GUP-486), with a one-line note
      on the S11 row of §11's table.

## Decisions

- **API** (on `Plot`):

  ```rust
  /// Use `scale` for a second y axis on the right of the plot rect and
  /// return a handle to encode with. Plain closures (`.y(|r| ..)`) still
  /// bind to the left axis; encode a layer on this one through the
  /// handle: `.attr(Line::Y, right.encode(|r| r.v))`.
  pub fn y_right<S: PositionScale>(&mut self, scale: S) -> ScaleRef<S>;
  /// Title of the right y axis: horizontal, right-aligned above it.
  pub fn y_right_title(&mut self, title: impl Into<String>) -> &mut Self;
  ```

  Stored as `y_right: Option<Box<dyn DynPositionScale>>` and
  `y_right_title: Option<String>`; both bump `revision`, and `y_right` bumps
  `axes_generation`. No `y2` name: right says where it is, and avoids a trailing
  digit should it ever reach WGSL (RFC-001 "S5c findings").

- **Fit and range**: the right scale fits like `y` (the per-resolve union over
  every channel encoded through it), is `nice`d when auto, widened by the same
  rule with its own `widened` flag, and gets `set_range(bottom, top + inset)`
  exactly as `y` does.
- **Ticks**: its own tick count, starting from the y count and thinned by the
  same `collides(.., vertical = true)` loop; `TickLayout` gains `y_right_ticks`
  and `y_right_label_w`.
- **Layout arithmetic** (`layout` closure): with
  `right_axis_w = tick_len + label_gap + y_right_label_w` (0 without a right
  axis),
  `right = width − legend_w − max(pad + right_axis_w, last_x_label_w / 2 + 2)`.
  Without a right axis this is today's formula. The top margin reserves the
  axis-title row when either y title is set. The hold (GUP-452) applies to the
  new `right` unchanged.
- **Guides**: an axis rule at `plot.right() + 0.5` with the same vertical extent
  rule as the left; ticks outward (to the right); labels at
  `plot.right() + 0.5 + tick_len + label_gap`, `HAlign::Start, VAlign::Middle`,
  `TextRole::TickLabel`; the right title at
  `(plot.right() + right_axis_w, plot.top() − label_gap)`,
  `HAlign::End, VAlign::Bottom`, `TextRole::AxisTitle`. The legend column starts
  at `plot.right() + right_axis_w + COLUMN_GAP`.
- **Grid**: only the left y axis draws horizontal grid lines. Two grids rarely
  align and read as noise.
- **Interaction**: everything that moves the y axis moves the right axis too, by
  the same pixel operation: `zoom` (about the pointer's y), `pan` (by `dy`), the
  drag-zoom rectangle, `reset_view`, and `view_axes`'s `Y` flag covers both. The
  per-axis arrays (`widened`, `zoomed`, `home`, `dragging`) grow from 2 to 3
  entries, index 2 being the right axis. `Plot::invert` stays (x, left y); no
  `invert_right`.
- **`revision_sum`** adds the right scale's revision.
- **Builders**: `scatter`/`line`/`bar` get no right-axis setter; a dual-axis
  chart is a `Plot` (or `plot_mut()`).
- **Golden data**: rainfall `[78, 64, 52, 38, 26, 20, 22, 30, 48, 90, 140, 180]`
  mm, temperature `[26, 25, 23, 20, 16, 12, 8, 10, 13, 17, 21, 24]` °C.

## Escalation Triggers

- Chunk culling (`visible_entries`, S9f) or picking reads the plot's `y` scale
  rather than each layer's own encoding, so right-axis layers cull or pick
  against the wrong scale.
- An existing golden or browser scene changes.
- A `Band` scale on the right axis (keys) needs anything beyond what `y` does.

## Out of Scope

- Deleting `ComposedChart` (GUP-486), `DynChart` (GUP-483, GUP-484) and
  `CompositeChart` (GUP-466).
- A second x axis; right-axis grid lines; builder setters.
- SVG specifics: guides go through the scene, which `SvgTarget` already writes.

## Technical Tasks

- [ ] Fields, `y_right`, `y_right_title`, revision and generation.
- [ ] Fit, nice, widen, range, ticks and thinning for the right scale.
- [ ] Layout arithmetic, guides, title, legend offset, `Layout::y_right_ticks`.
- [ ] Zoom, pan, drag-zoom, reset, view axes.
- [ ] `secondary_y.rs`: golden and AC3–AC4 checks; bless and read.
- [ ] Migration-table entry, RFC findings.

## Dependencies

### Prerequisite Stories

- GUP-466: RFC-001 S10g Delete Old Builders 📋 — its migration table entry for
  `.layer_with_y2` (soft: only AC5's first line needs it).
- GUP-452/GUP-453: hold, zoom and pan ✅.

### Enables Stories

- None directly; T5 builders can expose it later.

## Testing Strategy

- **Integration tests**: `secondary_y.rs` (golden, layout, interaction,
  uploads).
- **Regression**: every existing gup-core golden; `mask wasm-browser`.
- **Visual validation**: read `secondary_y.png`.

## Success Metrics

- [ ] A two-axis chart in about 15 lines on `Plot`.
- [ ] No existing image changes.

## Risk Assessment

- **Medium**: `plot.rs` is 2,800 lines with intertwined per-axis state; missing
  one array (`home`, `dragging`) gives a right axis that zooms but does not
  reset. _Mitigation_: AC4 tests each operation.
- **Medium**: trybuild `.stderr` churn if `selection.rs`, `encoding.rs` or
  `marks.rs` lines shift (the snapshots quote them). _Mitigation_: re-bless with
  `TRYBUILD=overwrite` only for line-number diffs; escalate on message changes.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-browser` passes (this story changes layout and input)
- [ ] `mask wasm-size` run and the scatter and per-row-type figures reported in
      the Implementation Summary (the orchestrator re-records the budget)
- [ ] `mask perf-budget` table pasted (story-worker step 9)
- [ ] Rendered output verified by eye: `secondary_y.png`
- [ ] `mask old-path-loc` recorded (expected unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
