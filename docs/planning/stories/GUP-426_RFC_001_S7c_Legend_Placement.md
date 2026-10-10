# GUP-426: RFC-001 Step S7c: Legend Placement

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-11

## Context

[GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md) built the
`ColorScale::legend()` machinery — `Legend::Swatches` for `Categorical`,
`Legend::Ramp` for `Sequential`/`Diverging` — and its own tests place them by
hand: RFC-001's "S5b findings → Proposed adjustments to S5c, S6, S7" records
"Legend guides: place `Legend::Swatches` and `Ramp`, with titles. The tests
place them by hand today (`tests/common/legend.rs`)." That is, the _data_ a
legend needs (swatch colours and labels, or a ramp's LUT and ticks) already
exists and is exercised by goldens like `categorical_species` and
`diverging_anomaly`, but `Plot::resolve` has no code path that measures a
legend, reserves space for it in the plot rect, and emits its
`ItemKind::Gradient`/swatch scene items automatically — every current legend
golden constructs the legend's `Item`s directly in test code, not through
`Plot`.

This story is the automatic placement RFC-001 §9's `Layout` design calls for
(`pub legend: Option<Rect<Px>>`) and §7's default z-order already reserves space
for ("legend and title 200"). It depends on
[GUP-424](GUP-424_RFC_001_S7a_Multi_Layer_Plot_And_Shared_Scales.md)'s
shared-dictionary fix, because a legend for a categorical scale shared by two
layers must list the union of their keys with one colour per key — building the
legend from only one layer's (possibly incomplete, possibly differently-coded)
dictionary would reproduce S7a's bug at the legend layer. It also benefits from
[GUP-425](GUP-425_RFC_001_S7b_Axes_Theme_And_Tick_Quality.md)'s `Theme` and
axis-title margin computation, since the legend competes with axis titles and
the chart title for the same plot-rect-shrinking budget.

## User Story

> "As a visualization developer, I want a chart's colour legend to be placed
> automatically, with a title, without manually constructing its scene items or
> getting its layout wrong next to the axis titles and chart title, so that a
> categorical or sequential colour encoding is self-explanatory by default."

## Acceptance Criteria

### AC1: Automatic legend placement

- [ ] `Plot::resolve` detects when a layer's colour (or other legend-bearing)
      channel is bound through a `ColorScale`, measures its `Legend` (`Swatches`
      or `Ramp`) using `gup-text` for label widths, reserves a rectangle for it
      (a documented default position — for example to the right of the plot rect
      — matching `Layout::legend: Option<Rect>`'s RFC-001 §9 shape), and shrinks
      the plot rect accordingly, the same way axis titles and the chart title
      already shrink it.
- [ ] `categorical_species` and `diverging_anomaly` (or their current
      equivalents) are re-rendered with the legend placed by `Plot::resolve`
      itself rather than constructed by hand in test code;
      `tests/common/     legend.rs`'s manual construction is removed or reduced
      to only what genuinely still needs manual control.
- [ ] A golden with a long chart title, axis titles (if GUP-425 has landed) and
      a legend together is read by eye: none overlaps another, and the plot rect
      is the remaining space after all three guides are placed.

### AC2: Legend titles

- [ ] A legend has its own title (for example the field name or a caller-
      supplied string, matching how axis titles are supplied), placed above or
      beside the swatches/ramp, measured and laid out with the same `gup-text`
      API as every other text run.
- [ ] A golden shows a titled legend, read by eye: the title is legible and does
      not overlap the swatches or ramp.

### AC3: Overflow and multiple legends

- [ ] A legend taller or wider than the available chart height/width (many
      categorical keys, or past the 64-colour Okabe-Ito-extension threshold from
      GUP-419) is handled without panicking or drawing off-canvas — wrapping
      into columns, scrolling (not applicable to a static render), truncation
      with a "+N more" indicator, or a documented minimum-size behaviour; the
      chosen strategy is demonstrated in a test with a a long domain (for
      example 50+ keys).
- [ ] If more than one layer contributes a distinct legend-bearing scale (for
      example one layer's categorical colour and another's sequential size — if
      size legends exist by this point, otherwise document as deferred),
      multiple legends are placed without overlapping each other; if this case
      cannot yet arise (no second legend-bearing scale exists in `gup-core` at
      this story's implementation time), this sub-AC is marked deferred with a
      note, per the pattern GUP-421's AC3 used for its own not-yet-applicable
      case.

## Technical Tasks

- [ ] Extend `Layout` (`crates/gup-core/src/plot.rs`) with the
      `legend:     Option<Rect>` field RFC-001 §9 specifies, if not already
      present.
- [ ] Implement legend measurement (swatch/ramp size from the `Legend` data,
      label widths from `gup-text`) and placement in `Plot::resolve`, following
      the existing title/axis-label margin computation pattern
      (`plot.rs:236-258` and its neighbours).
- [ ] Emit the legend's `ItemKind::Gradient`/swatch/text scene items from
      `resolve` rather than from test code; update or remove
      `tests/common/legend.rs`'s manual construction accordingly.
- [ ] Add legend-title support (text run, measured and laid out).
- [ ] Implement the chosen overflow strategy (AC3) for long domains.
- [ ] Re-bless `categorical_species`, `diverging_anomaly` and any other
      legend-bearing golden to go through automatic placement.
- [ ] Add the combined-guides golden (title + axis titles + legend) and the
      long-domain overflow test.

## Dependencies

### Prerequisite Stories

- GUP-424: RFC-001 S7a Multi-Layer Plot and Shared Scales 📋 — the
  shared-dictionary fix a correct multi-layer legend depends on.
- GUP-419: RFC-001 S5b Colour Scales and Dictionary Hook ✅ —
  `ColorScale:: legend()`, `Legend::Swatches`/`Ramp`, and the manual
  legend-placement test helpers this story automates.

### Soft Dependencies

- GUP-425: RFC-001 S7b Axes, Theme and Tick Quality 📋 — not required to start,
  but the combined-guides golden (AC1's third bullet) is most meaningful once
  axis titles and `Theme` exist; without GUP-425, this story's combined golden
  covers chart title plus legend only.

### Enables Stories

- RFC-001 S10 (builders; not yet written as a story) — a `.color(...)` call on a
  builder should produce a legend with no further caller effort, which needs
  this story's automatic placement.

## Testing Strategy

- **Unit tests**: legend measurement (swatch/ramp size) against known inputs;
  overflow-strategy triggering at the threshold.
- **Integration tests**: `Plot::resolve` placing a legend without manual
  scene-item construction; combined-guides non-overlap.
- **Visual validation**: re-blessed `categorical_species`/`diverging_anomaly`, a
  titled-legend golden, a long-domain overflow golden — all read by eye.
- **Performance**: legend measurement should be part of the existing resolve
  pipeline's one-time-per-change cost (RFC-001 §9); confirm it participates in
  GUP-424's resolve-cache rather than re-measuring on every frame.

## Success Metrics

- [ ] Every legend-bearing golden is placed by `Plot::resolve` itself, with
      `tests/common/legend.rs`'s manual construction removed or reduced.
- [ ] A combined-guides golden (title, axis titles if available, legend) shows
      no overlap, read by eye.
- [ ] A long-domain legend does not panic or draw off-canvas.

## Risk Assessment

- **Medium**: default legend placement (position, size budget relative to the
  plot rect) has no existing precedent in `gup-core` to match against; an
  arbitrary choice may need revisiting once real builder output is seen in T5.
  _Mitigation_: document the chosen default placement clearly as a first cut,
  and make it a `Plot` setting (not hard-coded) so T5 builders can override it
  without another RFC-001 story.
- **Low**: the 64-colour Okabe-Ito extension (GUP-419) means a legend can in
  principle need to show many swatches; AC3's overflow strategy must be chosen
  pragmatically rather than gold-plated. _Mitigation_: a simple "+N more"
  truncation is an acceptable first cut; document it as such.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
