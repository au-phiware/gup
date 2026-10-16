---
implementer: sonnet
implementer-reason:
  channels, caps and WGSL settled in Decisions; the shader is rule.wgsl's use of
  gup::coverage::line_quad behind the derive
---

# GUP-423: RFC-001 Step S6b: `Segment` Mark

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-11

## Context

RFC-001's S6 step lists "Circle, Rect, Rule, Segment/Line". This story delivers
the per-row line mark with independent endpoints, `Segment`; a reference rule is
a `Segment` with equal or constant endpoints, so there is no separate `Rule`
mark (the guide primitive `scene::Rule` / `ItemKind::Rules`,
`crates/gup-core/src/scene.rs` lines 150–159, is unrelated and unchanged). The
connected polyline, `Line`, needs a previous/next binding across column chunks
and join geometry, which is design work: it moved to
[GUP-432](GUP-432_RFC_001_S6f_Line_Mark.md) on 2026-10-16.

**GUP-421 landed the module as `gup::coverage`**
(`crates/gup-wgsl/library/coverage.wgsl`):
`line_quad(start, stop, width, cap, vertex_index, dpr)` returns the quad
corner's position plus `local`/`half` in physical pixels, and
`box_coverage(local, half)` the edge coverage; `cap` is `width / 2` for square
caps and 0 for butt caps. `rule.wgsl` is the reference use. A mark declares
`Mark::ANTIALIAS` (`Analytic` by default). GUP-421 also made 1× sampling the
default, with 1× and 4× renders agreeing within 2/255.

A segment has two position channels per axis (`x`, `x_end`), both normally
encoded through the plot's shared x scale. Today `fit_domain` replaces a scale's
domain (`Linear::fit_domain`, `scale/linear.rs` line 112), so the last channel
fitted wins;
[GUP-424](GUP-424_RFC_001_S7a_Multi_Layer_Plot_And_Shared_Scales.md) makes every
fit in a resolve union into the scale's domain. That is why this story depends
on GUP-424.

## Read First

- RFC-001 "GUP-421 findings" (lines 3016–3055), the `gup::coverage` bullet.
- `crates/gup-wgsl/library/coverage.wgsl` (all).
- `crates/gup-core/src/shaders/rule.wgsl` (all; the reference use of
  `line_quad`).
- `crates/gup-core/src/marks.rs` (after GUP-422) and
  `crates/gup-wgsl/library/circle.wgsl` (the mark contract).
- `crates/gup-core/src/plot.rs` lines 782–818 (test
  `a_cleared_validity_bit_hides_a_finite_row`: how a NaN makes a null row).

## User Story

> "As a visualization developer, I want a `Segment` mark with real pixel widths
> and antialiased square ends, so that I can draw error bars, whiskers,
> reference lines and vector fields from each row's own data."

## Acceptance Criteria

### AC1: `Segment` mark

- [ ] `gup_core::Segment` (crate root and prelude, with `SegmentAttrs`) is
      defined with `#[derive(Mark)]` over `crates/gup-wgsl/library/segment.wgsl`
      (`gup::marks::segment`), with the channels in Decisions.
- [ ] `Segment`'s doc comment has a doctest drawing error bars
      (`x = x_end = year`, `y = lo`, `y_end = hi`) that renders.

### AC2: Geometry is exact at known pixels

- [ ] A render test (explicit domains, white background) draws a horizontal
      segment 100 px long, width 4 px, its endpoints computed with the scales'
      `eval` after `resolve`. Its box is the segment extended 2 px past each end
      (square caps) and 2 px to each side. Every pixel whose centre lies at
      least 0.5 px inside the box equals the colour within 2/255; every pixel
      whose centre lies at least 1.5 px outside it is white within 2/255.
- [ ] A diagonal segment (45°, length 100 px, width 3 px) holds 309 ± 6 pixels'
      worth of ink (sum of coverage from one colour channel): the length plus
      both 1.5 px caps, times the width.

### AC3: Golden `segment_mark`

- [ ] Golden `gup_core/segment_mark` (720×450): 20 vertical error bars from a
      deterministic series, width 1.5 px, plus a fan of 8 diagonal segments at
      widths 0.5, 1, 2 and 4 px.
- [ ] Read by eye: edges are smooth at 1×, not stair-stepped; ends are square;
      the 0.5 px segments are visible at reduced strength. The retrospective
      describes the PNG.

### AC4: Per-row width and colour

- [ ] A render test encodes `width` through `Linear` (range `Px(1.0)..Px(6.0)`)
      and `color` through `Categorical`; the measured perpendicular ink width of
      the thinnest and thickest horizontal segments matches their encoded widths
      within 0.5 px, and each segment's centre pixel is its category's colour
      within 2/255.

### AC5: Nulls and domains

- [ ] A row whose `x_end` is NaN is not drawn (no ink within 10 px of its other
      endpoint) and the rest draw.
- [ ] With auto domains, `x` data in 0..10 and `x_end` data in 5..30 give an x
      domain covering 0..30 after `resolve` (GUP-424's union).

## Decisions

- **Channels**, in order:
  - `x`: `Px`, role `X`, default 0
  - `y`: `Px`, role `Y`, default 0
  - `x_end`: `Px`, role `X`, default 0
  - `y_end`: `Px`, role `Y`, default 0
  - `width`: `Px`, no role, default `Px(1.0)`
  - `color`: `Color`, role `Color`, default `Segment::DEFAULT_COLOR` (`#0072B2`)

  `width` has no role, so a segment on the domain edge may extend `width / 2`
  past the plot rect, where the plot clip cuts it.

- **Caps are square** (`cap = width / 2`), fixed: they match the guide rules
  (GUP-421) and make whiskers look complete. Butt caps are `Line`'s concern
  (GUP-432).
- **WGSL** (`segment.wgsl`): `SegmentIn` as above;
  `Varyings { clip, local, half, color, row }` like `rect.wgsl`; `vertex` calls
  `coverage::line_quad(vec2(x, y), vec2(x_end, y_end), width, width * 0.5, vertex_index, view.dpr)`;
  `shade` returns premultiplied colour with
  `a = color.a * coverage::box_coverage(local, half)`, discarding `a <= 0`.
- **No `Rule` mark** and no stroke/opacity channels (a segment is a stroke;
  `color`'s alpha is its opacity).

## Escalation Triggers

- AC5's domain test fails after GUP-424 landed (the union does not cover two
  channels of one layer).
- The diagonal ink in AC2 is off by more than 2% (the rotated-box approximation
  in `coverage.wgsl` may need revisiting; do not change it here).

## Out of Scope

- `Line` (connected polylines, joins): GUP-432.
- Round caps, arrowheads, dashes: not planned.
- Full-width reference rules tied to the plot rect (`ruleY`-style): a builder
  concern, RFC-001 S10, not yet a story.

## Technical Tasks

- [ ] Add `segment.wgsl` and `Segment` (derive); export both.
- [ ] Tests for AC2, AC4, AC5; golden `segment_mark`, read by eye.

## Dependencies

### Prerequisite Stories

- GUP-422: `#[derive(Mark)]` v2 📋 — defines `Segment`.
- GUP-424: Multi-Layer Plot and Shared Scales 📋 — unions `x`/`x_end` into one
  domain.
- GUP-421: Analytic AA ✅ — `gup::coverage::line_quad`.

### Enables Stories

- GUP-432: `Line` reuses `segment.wgsl`'s shading.

## Testing Strategy

- **Integration tests**: AC2, AC4, AC5 pixel and domain checks.
- **Visual validation**: `gup_core/segment_mark`.
- **Performance**: none expected.

## Success Metrics

- [ ] Error bars and reference lines draw with one mark and no hand-built scene
      items.

## Risk Assessment

- **Low**: a zero-length segment draws as a `width` square (documented in
  `line_quad`). _Mitigation_: accepted; documented on `Segment`.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
