---
implementer: sonnet
implementer-reason:
  channels, stroke placement and the compositing formula settled in Decisions;
  shader changes extend circle.wgsl and rect.wgsl's existing coverage
---

# GUP-430: RFC-001 Step S6d: Stroke and Fill Opacity for Circle and Rect

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-16

## Context

[GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md)'s
retrospective and RFC-001's "S5b findings → Proposed adjustments" list "fill
opacity and a stroke. S5a's `sqrt_radius` discs are opaque and unstroked, so big
discs hide small ones, and a diverging midpoint vanishes on white". The
`diverging_anomaly` golden works around the second problem with a tinted plot
background (`crates/gup-core/tests/ordinal_png.rs`, `PLOT_TINT`, with the
comment "circles have no stroke until RFC-001 S6"). RFC-001 "S5c findings" adds:
"A user colour function returns straight alpha like the built-ins, so S6's fill
opacity and stroke must compose with it, not assume a library palette". That
rules out baking opacity into palettes: opacity must be its own channel,
multiplied into whatever colour the fill encoding returns.

This story was split out of [GUP-422](GUP-422_RFC_001_S6a_Derive_Mark_V2.md) on
2026-10-16. It adds the same three channels to both quad marks, `Circle` and
[GUP-429](GUP-429_RFC_001_S6c_Rect_Mark_And_Derived_Constants.md)'s `Rect`.
RFC-001 §4 lists `Unit(pub f32)` among the visual types; it does not exist yet
and is the natural type for an opacity.

## Read First

- RFC-001 §4 (lines 259–318): the `Circle` sketch with `stroke` and
  `stroke_width`, and the visual types.
- RFC-001 "S5c findings" → "Proposed adjustments to S6, S7 and later" (lines
  2993–3014), the S6 bullet.
- `crates/gup-wgsl/library/circle.wgsl` (all),
  `crates/gup-wgsl/library/rect.wgsl` (from GUP-429) and
  `crates/gup-wgsl/library/coverage.wgsl` (`box_coverage`).
- `crates/gup-core/src/channel.rs` lines 59–100 (`GpuType`, `Visual`, `Px`) and
  the `const_value`/`ZERO` additions from GUP-422.
- `crates/gup-core/tests/ordinal_png.rs` lines 170–225
  (`diverging_temperature_anomaly_with_a_ramp_legend`).

## User Story

> "As a visualization developer drawing a bubble chart or scatter plot, I want
> semi-transparent fills and an optional stroke on circles and rects, so that
> overlapping marks stay distinguishable and a near-white fill is still visible
> on a white background."

## Acceptance Criteria

### AC1: `Unit` and the new channels

- [ ] `gup_core::Unit(pub f32)` exists (crate root and prelude), a `Visual` with
      WGSL type `f32`, `const_value` and `ZERO` as GUP-422 defined for `Px`.
- [ ] `Circle` and `Rect` each gain, after `fill`: `fill_opacity: Unit` (default
      `Unit(1.0)`), `stroke: Color` (default `Color::TRANSPARENT`),
      `stroke_width: Px` (default `Px(0.0)`); their WGSL input structs match
      (the derive's check enforces it).
- [ ] With the defaults, every existing gup-core golden passes without
      re-blessing.

### AC2: Compositing is exact at known pixels

- [ ] A render test draws, on white with explicit domains, circle A (r 60, fill
      `#0072B2`, `fill_opacity 0.5`) then circle B (r 60, fill `#D55E00`,
      `fill_opacity 0.5`) 60 px to its right, both with stroke `#000000` width 3
      px. At the midpoint between the centres (inside both, away from both
      rings) the pixel equals B over A over white, computed on the CPU in sRGB
      with premultiplied "over", within 3/255 per channel.
- [ ] The pixel 1.5 px inside A's leftmost edge (centre of the ring) equals the
      stroke colour within 3/255, and the pixel 2 px outside it is white.
- [ ] The same two checks pass for two `Rect`s with the same styling.

### AC3: Golden `bubble_stroke`

- [ ] Golden `gup_core/bubble_stroke` (720×450): 12 deterministic overlapping
      circles, radii 20–60 px, fills from `Sequential::viridis()`,
      `fill_opacity Unit(0.6)`, stroke `#333333` width `Px(1.5)`.
- [ ] Read by eye: every circle's outline is visible where it overlaps another,
      and fills visibly blend. The retrospective describes the PNG.

### AC4: `diverging_anomaly` without the tint

- [ ] `diverging_temperature_anomaly_with_a_ramp_legend` sets stroke `#555555`,
      `stroke_width Px(1.0)` on its circles and no longer pushes the `PLOT_TINT`
      rect; its `"plot background"` expected colour becomes white. The golden is
      re-blessed.
- [ ] For the row whose anomaly is closest to 0, the darkest pixel on a
      horizontal line through its centre, within its radius, has every channel ≤
      0x99 (the outline is visible on white).
- [ ] Read by eye: the near-white discs are visible by their outlines.

## Decisions

- **Stroke inside the edge**: the ring spans `radius - stroke_width .. radius`
  (circle) and `half - stroke_width .. half` per axis (rect). The mark's
  footprint does not grow, so the plot inset (`overhang`, from the `Size` role)
  and bar extents stay exact and adjacent bars do not overlap. A `stroke_width`
  larger than the shape fills it with stroke.
- **Compositing** (premultiplied, per fragment): `outer` = coverage of the
  shape, `inner` = coverage of the shape shrunk by `stroke_width` (circle:
  `clamp(0.5 - (length(local) - max(radius - sw, 0)), 0, 1)`; rect:
  `box_coverage(local, max(half - sw, vec2(0)))`, physical pixels),
  `ring = outer - inner`. With `F` = fill premultiplied by
  `fill.a * fill_opacity` and `S` = stroke premultiplied by `stroke.a`:
  `out = S * ring + F * outer * (1 - S.a * ring)`. Fill lies under the stroke,
  as in SVG. Discard when `out.a <= 0`.
- **Varyings**: add `stroke: vec4<f32>`, `stroke_width: f32` (physical pixels)
  and fold `fill_opacity` into the fill's alpha in `vertex`.
- **Opacity applies to the fill only**, not the stroke (a translucent stroke
  uses the stroke colour's alpha).
- **Grey tint removal is unconditional** (AC4); `PLOT_TINT` and its rect are
  deleted from `ordinal_png.rs` if nothing else uses them.

## Escalation Triggers

- Any existing golden changes with default channel values (AC1).
- The AC2 midpoint differs from the CPU "over" by more than 3/255: the
  premultiplied blend state may not be what `shade` assumes.
- The ring at `stroke_width < 1 px` looks broken (gaps or doubled AA) in
  `bubble_stroke`.

## Out of Scope

- Draw order (size-descending, emphasis last): GUP-431.
- Strokes on `Segment`/`Line` (they are strokes): GUP-423, GUP-432.
- Dashed strokes, stroke opacity channel, centred or outer strokes: not planned.
- Removing the tinted background from the legend helper
  (`tests/common/legend.rs`, used by `scene_items`/`svg`): GUP-426.

## Technical Tasks

- [ ] Add `Unit` to `channel.rs` and export it.
- [ ] Add the three fields to `Circle` and `Rect` and to `CircleIn`/`RectIn`.
- [ ] Implement the ring coverage and compositing in both shaders.
- [ ] Tests for AC2; golden `bubble_stroke`; update and re-bless
      `diverging_anomaly`.

## Dependencies

### Prerequisite Stories

- GUP-429: `Rect` Mark and Scale-Derived Constants 📋 — `Rect` and `rect.wgsl`.
- GUP-422: `#[derive(Mark)]` v2 📋 — the derive and `const_value`/`ZERO`.

### Enables Stories

- GUP-431: draw order must be correct with translucent fills.

## Testing Strategy

- **Unit tests**: `Unit` const value.
- **Integration tests**: AC2 pixel checks for both marks.
- **Visual validation**: `bubble_stroke` (new), `diverging_anomaly`
  (re-blessed), both read by eye.
- **Performance**: `mask perf-budget` passes; the extra coverage term is one
  more `clamp` per fragment.

## Success Metrics

- [ ] `diverging_anomaly` needs no background workaround.
- [ ] Overlapping translucent, stroked bubbles composite to the CPU-computed
      colour.

## Risk Assessment

- **Low**: double antialiasing where the inner and outer ramps overlap on thin
  strokes. _Mitigation_: `ring = outer - inner` is a difference of coverages, so
  it stays in 0..1 and sums correctly.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
