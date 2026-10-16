# GUP-423: RFC-001 Step S6b: Line, Segment and Rule Marks

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-11

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md)'s S6 migration step lists four
marks: "Circle, Rect, Rule, Segment/Line" (§11 "Story sequence").
[GUP-422](GUP-422_RFC_001_S6a_Derive_Mark_Circle_And_Rect.md) (S6a) delivers the
`#[derive(Mark)]` v2 macro plus `Circle` and `Rect`, both quad marks whose
geometry is a disc or an axis-aligned box computed from one row's own values.
`Rule`, `Segment` and `Line` are a different shape of problem: a line has a
direction and a width, so its quad must be expanded perpendicular to that
direction (not simply centred on a point), it needs join and cap geometry where
segments meet or end, and — for `Line`, which connects consecutive rows into a
polyline — RFC-001 §3 says the geometry "bind[s] the same column twice, offset
by one element, which gives a previous/next pair per instance", a binding
pattern none of today's marks use.

[GUP-421](GUP-421_Analytic_AA_Rules_Rects_1x_MSAA.md) (📋 Planned) was written
explicitly to give `gup-core`'s guide-only `rule.wgsl` (used by
`ItemKind::Rules` for axis lines, ticks and grid, not by any per-row `Mark`)
analytic edge coverage at 1× MSAA, and its own Context section says: "This
story's rule quad-expansion and edge-distance math (half-width extension for
square caps, per-edge coverage) should be written so S6 can reuse it for the
`Line` mark's shader rather than inventing a second lines-with-width
implementation; it does not need to build `Line` itself." Its Technical Tasks
ask for "a shared WGSL module (`gup::marks::line_coverage` or similar) rather
than code living only in `rule.wgsl`." This story is that reuse: it is the story
GUP-421 wrote itself to enable. Building `Line`/`Segment`/`Rule` (as per-row
marks) before GUP-421 lands would mean writing the quad-expansion and
edge-coverage math from scratch and then reconciling two implementations once
GUP-421 does land — this story depends on GUP-421 for that reason, not only as a
style preference.

A per-row `Rule` mark is distinct from the guide-only `Scene::Rule` /
`ItemKind::Rules` primitive (`crates/gup-core/src/scene.rs:150-159`): the guide
primitive is a fixed list of `p0`/`p1`/`width`/`color` structs built once per
`resolve` for axis lines and grid; a `Rule` _mark_ is a per-row mark a user
attaches to a `Selection<T, Rule>` to draw, for example, a reference line or an
error bar from each row's own data. `Segment` is the two-endpoints-per-row
sibling (each row independent, like `Rule`, but typically with distinct styling
per segment); `Line` is the connect-the-dots sibling, where row _n_'s end is row
_n+1_'s start, requiring the previous/next column-binding trick above. The three
marks share one coverage module and differ mainly in how their vertex shader
derives each instance's two endpoints.

**GUP-421 landed the module as `gup::coverage`**
(`crates/gup-wgsl/library/coverage.wgsl`):
`line_quad(start, stop, width, cap, vertex_index, dpr)` returns the quad
corner's position plus `local`/`half` in physical pixels, and
`box_coverage(local, half)` the edge coverage; `cap` is `width / 2` for square
caps and 0 for butt caps. `rule.wgsl` is the reference use. A mark declares
`Mark::ANTIALIAS` (`Analytic` by default).

## User Story

> "As a visualization developer, I want `Line`, `Segment` and `Rule` marks with
> real pixel widths, joins and caps, antialiased the same way circles and rects
> are, so that I can draw connected time series, independent reference segments,
> and single reference lines without approximating them with zero-width guide
> primitives."

## Acceptance Criteria

### AC1: A shared line-coverage WGSL module

- [ ] `gup::marks::line_coverage` (or the name GUP-421 actually lands with) is
      imported by all three marks' WGSL modules, not copy-pasted. If GUP-421
      named it differently, this story uses that name, not a new one.
- [ ] The module exposes per-pixel distance-to-segment coverage (long edges plus
      caps), parameterised by local offset, half-width and segment length,
      matching GUP-421's `rule.wgsl` technique
      (`clamp(0.5 - distance, 0.0,     1.0)` ramped over one physical pixel).

### AC2: `Rule` and `Segment` marks

- [ ] `Rule` (`gup::marks::rule`, a new per-row mark, not the guide primitive)
      draws one line per row from `X0`/`Y0`/`X1`/`Y1` (or two point-pair
      channels — document the chosen channel shape), `WIDTH`, `COLOR`, and a cap
      style (square, to start, matching GUP-421's rule geometry; round caps may
      be deferred — document if so).
- [ ] `Segment` is either the same mark as `Rule` under a different name (if the
      two have no behavioural difference once built) or a documented thin
      variant; the story states which and why, since RFC-001 lists them together
      ("Rule, Segment/Line").
- [ ] A golden (`gup_core/rule_mark` or similar) draws several rules/segments at
      varying widths and angles (including diagonal) and, read by eye, shows
      smooth antialiased edges at 1× sampling comparable to GUP-421's rule-fan
      comparison, not stair-stepped.

### AC3: `Line` mark connects consecutive rows

- [ ] `Line` binds its position column twice, offset by one row (the
      previous/next pair RFC-001 §3 describes), so each instance's quad is the
      segment between row _n_ and row _n+1_, with `N-1` instances for `N` rows.
- [ ] Joins between consecutive segments (where row _n+1_ is shared by two
      instances) do not show a visible gap or an overlap seam at the join, read
      by eye at a sharp angle between two segments (a "V" shape) and at a
      shallow one.
- [ ] A null row (RFC-001 S4b's degenerate-quad or S5b's `NULL_CODE` hiding, per
      GUP-422's AC5 documentation) breaks the line at that row rather than
      interpolating across it or drawing a segment to/from an undefined point: a
      test with a null value in the middle of a series confirms the line is
      drawn as two disconnected pieces, not one continuous line through the gap.
- [ ] A golden (`gup_core/line_series` or similar) draws a multi-segment
      polyline (for example a time series) with visible width (not a hairline)
      and, read by eye, shows clean joins and the expected break at a null row.

### AC4: Width, colour and (optionally) per-row styling

- [ ] All three marks accept a `WIDTH: Channel<M, Px>` (constant or
      per-row/per-segment) and a `COLOR: Channel<M, Color>` channel, following
      the same typed-channel pattern as `Circle`/`Rect`.
- [ ] A test confirms a per-row (not just constant) width or colour renders
      correctly — for example alternating widths or colours along a `Line`
      series, read by eye in the golden.

## Technical Tasks

- [ ] Confirm GUP-421's landed module name/location for the shared line-coverage
      math; import it rather than re-deriving.
- [ ] Use GUP-422's `#[derive(Mark)]` macro to define `Rule`/`Segment`'s (and,
      if distinct, `Line`'s) channels and `Mark` impl.
- [ ] Write `gup::marks::rule` (and `gup::marks::segment` if distinct) WGSL:
      quad expansion perpendicular to the p0→p1 direction, by `WIDTH/2`, with
      square-cap extension at the ends; fragment coverage via the shared module.
- [ ] Write `gup::marks::line` WGSL: bind the position column at both `row` and
      `row + 1` (two vertex-buffer slots referencing the same buffer at an
      offset, or an equivalent instance-index trick within the existing 8-slot
      budget per RFC-001 §12 risk 7), expand perpendicular to the segment
      direction, handle the join between adjacent instances, and emit a
      degenerate instance for `N-1`'s last row with no next element and for a
      null row per AC3.
- [ ] Decide and document the join style (bevel, miter or round) achievable
      within this story's scope; defer anything requiring new geometry beyond
      the shared coverage module, with a note.
- [ ] Add goldens: `rule_mark`, `line_series` (and a null-row variant).
- [ ] Add the adversarial-angle join test and the null-row break test.

## Dependencies

### Prerequisite Stories

- GUP-421: Analytic AA for Rules and Rects; 1× MSAA 📋 — provides the shared
  quad-expanded-line coverage math (`gup::marks::line_coverage` or equivalent)
  this story reuses rather than re-implements. **Hard dependency**: this story
  should not start writing line-mark geometry before GUP-421 lands, per its own
  stated intent.
- GUP-422: RFC-001 S6a `#[derive(Mark)]` v2, Circle and Rect 📋 — provides the
  derive macro this story's marks are defined with, and the `clip`-contract
  documentation (null-row hiding mechanisms) AC3's null-row behaviour depends
  on.

### Enables Stories

- RFC-001 S7 stories (GUP-424–426) — guide rendering (axis lines, grid) may
  eventually be reunified with these per-row marks' coverage module, though that
  reunification is not required by S7 and is not in this story's scope.
- RFC-001 S10/S11 (line and area builders; not yet written as stories) — need a
  `Line` mark with real widths to replace the old path's line builder.

## Testing Strategy

- **Unit tests**: channel/`Mark` impl shape for `Rule`/`Segment`/`Line`
  (mirroring `Circle`'s existing tests via the derive macro).
- **Integration tests**: adversarial-angle join rendering; null-row break in a
  `Line` series; per-row width/colour variation.
- **Visual validation**: `rule_mark`, `line_series` goldens read by eye,
  comparable in antialiasing quality to GUP-421's rule-fan comparison.
- **Performance**: `mask perf-budget` — a `Line` series with the previous/next
  binding should not add a second full column upload; record the vertex-buffer
  slot usage against the 8-slot budget (RFC-001 §12 risk 7).

## Success Metrics

- [ ] `Rule`, `Segment` and `Line` marks exist, reusing one coverage module with
      GUP-421 rather than a second implementation.
- [ ] `line_series` and `rule_mark` goldens read by eye show clean joins, caps
      and a correct null-row break.
- [ ] No vertex-buffer slot budget violation (RFC-001 §12 risk 7) for `Line`.

## Risk Assessment

- **Medium**: binding one column twice at an offset for `Line`'s previous/next
  pair may need a second vertex-buffer slot per position component, competing
  with the 8-slot budget RFC-001 §12 risk 7 already flags as tight.
  _Mitigation_: consider deriving "next" from "previous" plus
  `instance_index + 1` addressed into the same buffer with a dynamic offset,
  rather than a second bound slot, if the first approach runs out of slots.
- **Medium**: join geometry (miter vs bevel vs round) affects visual quality at
  sharp angles and is more involved than GUP-421's independent-rule case (which
  has no shared vertices between rules). _Mitigation_: start with bevel joins
  (simplest, matches two independent rule caps meeting), document the
  limitation, and leave miter/round as a follow-up if the eye-read goldens show
  it matters.
- **Low**: this story is blocked on GUP-421, which is not yet implemented; if
  GUP-421's module shape changes during its own implementation, this story's AC1
  module name/import path will need to track that change.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
