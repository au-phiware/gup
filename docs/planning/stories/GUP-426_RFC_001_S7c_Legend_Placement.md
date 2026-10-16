---
implementer: sonnet
implementer-reason:
  legend discovery hook, geometry, overflow rule and API are settled in
  Decisions; drawing code moves from tests/common/legend.rs
---

# GUP-426: RFC-001 Step S7c: Legend Placement

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-11

## Context

[GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md) built
`ColorScale::legend()` (`Legend::Swatches` for `Categorical`, `Legend::Ramp` for
`Sequential`/`Diverging`; `crates/gup-core/src/scale/legend.rs`), but
`Plot::resolve` never draws one: the goldens `categorical_species` and
`diverging_anomaly` place legends by hand with `tests/common/legend.rs`
(`swatches` and `ramp`, lines 127–226), after resolving the plot narrower than
the image. RFC-001 "S5b findings → Proposed adjustments": "Legend guides: place
`Legend::Swatches` and `Ramp`, with titles." RFC-001 §9's `Layout` has
`legend: Option<Rect<Px>>`, and §7 puts legends at z 200 with the title.

`Plot` does not know which scales colour its layers: a colour `ScaleRef` is
never registered with the plot. Each layer's encodings do know, through their
links (`&dyn DynShaderFn`), so a provided `legend()` on `ShaderFn` lets
`resolve` collect legends from each layer's `Color`-role channels. Two layers
sharing a colour scale produce equal `Legend` values, so equality removes
duplicates. Legends for chains (`Linear.then(Pow::sqrt()).then(viridis)`) and
user `#[wgsl_function]` colour functions moved to
[GUP-437](GUP-437_RFC_001_S7h_Legends_For_Chains.md) on 2026-10-16.

## Read First

- RFC-001 §9 (lines 618–666), the resolve pipeline and `Layout`.
- RFC-001 "S5b findings" → "Proposed adjustments" (lines 2791–2826).
- `crates/gup-core/src/scale/legend.rs` (all, 162 lines).
- `crates/gup-core/tests/common/legend.rs` lines 1–60 and 127–248.
- `crates/gup-core/tests/ordinal_png.rs` lines 95–135 (`categorical_species`)
  and 170–225 (`diverging_anomaly`).
- `crates/gup-core/src/plot.rs` `resolve` (after GUP-425: margins, titles,
  `Layout`).
- `crates/gup-core/src/encoding.rs` lines 811–870 (`DynShaderFn`).

## User Story

> "As a visualization developer, I want a chart's colour legend placed
> automatically, with a title, so that a categorical or sequential colour
> encoding is self-explanatory by default."

## Acceptance Criteria

### AC1: Automatic legends

- [ ] `categorical_species` and `diverging_anomaly` no longer build legend items
      or resolve narrower than the image: `Plot::resolve` places them. Both
      goldens are re-blessed and read by eye. Their existing data/legend colour
      assertions still pass, reading swatch and ramp positions from
      `layout.legend` and `layout.texts`.
- [ ] `Layout` has `pub legend: Option<Rect>`; it is `Some` exactly when a
      legend is drawn, lies inside the canvas, and does not intersect
      `layout.plot`.
- [ ] `Plot::legends(&mut self, on: bool) -> &mut Self` (default on); with
      `false` no legend is drawn and the plot rect is as without legends.

### AC2: Titles

- [ ] `Selection::legend_title(&mut self, channel: Channel<M, Color>, title:     impl Into<String>) -> &mut Self`;
      the title is drawn above its legend as a `TextRole::Legend` run.
- [ ] Golden `gup_core/legend_guides` (720×450): chart title, x and y axis
      titles, and a titled categorical legend. A test asserts all `layout.texts`
      ink boxes are pairwise disjoint and inside the canvas; read by eye.

### AC3: Long domains

- [ ] A 60-key categorical legend on a 720×300 plot resolves; the legend shows
      as many swatch rows as fit and a final "+N more" row, where N is the
      number of keys not shown; nothing is drawn below the canvas (test on
      `layout.texts` and `layout.legend`).

### AC4: Several legends

- [ ] Two layers, one with a `Categorical` fill and one with a `Sequential`
      fill, give two stacked legends that do not overlap (ink boxes and bars);
      two layers sharing one `ScaleRef<Categorical>` give one legend.
- [ ] Golden `gup_core/legend_two_layers`, read by eye.

## Decisions

- **Discovery**: `ShaderFn` gains a provided
  `fn legend(&self) -> Option<Legend> { None }`, forwarded by `DynShaderFn`.
  `Categorical`, `Sequential` and `Diverging` override it with
  `Some(ColorScale::legend(self))`; `ScaleRef` forwards through `read()`.
  `Layer` gains `fn legends(&self) -> Vec<(Option<String>, Legend)>`: for each
  `Color`-role channel whose encoding is a column with exactly one link that
  returns `Some`, its legend and title. Chains return nothing (GUP-437). `Plot`
  collects legends in layer order and drops any equal (`PartialEq`) to an
  earlier one, keeping the first non-`None` title.
- **Placement**: one column at the right. Its width is the widest legend:
  swatches `12 + 6 + max label width`; ramp
  `14 (bar) + 4 (tick) + 4 + max tick label width`; or the title's width if
  wider. The plot rect's right edge moves left by that width plus `16`. Legends
  stack from the plot rect's top, `theme.pad` apart; each is title (text
  height) + 6 px + body. Text uses `theme.tick_size` and `theme.label`.
- **Bodies**: swatch rows 20 px apart with 12 px squares (as
  `tests/common/legend.rs`); a ramp bar `min(200, space left)` tall, at least 60
  px, else the ramp is omitted with its title kept.
- **Overflow**: when swatch rows do not fit the space left, show `k - 1` rows
  and a final text row "+N more" (`N = keys - (k - 1)`), with no swatch. No
  wrapping into columns.
- **Code**: the drawing moves into `crates/gup-core/src/guide.rs`
  (`pub(crate)`), ported from `tests/common/legend.rs`' `swatches` and `ramp`;
  items at `Z_TITLE`. `tests/common/legend.rs` keeps `scene()` and the metadata
  helpers used by `scene_items` and `svg`, and loses `swatches`/ `ramp` once
  unused.

## Escalation Triggers

- `categorical_species`' or `diverging_anomaly`'s colour assertions fail after
  the move for a reason other than positions.
- Equal `Legend`s from different scales would be merged wrongly (two scales with
  identical domains and palettes); report rather than add identity.
- The `scene_items` or `svg` goldens change.

## Out of Scope

- Legends for chains and user colour functions: GUP-437.
- Size and opacity legends: not yet a story.
- Legend position options (bottom, inside): not planned until builders (RFC-001
  S10).

## Technical Tasks

- [ ] `legend()` hook on `ShaderFn`/`DynShaderFn`, overrides, `ScaleRef`.
- [ ] `Layer::legends`, `Selection::legend_title`, `Plot::legends`.
- [ ] `guide.rs` layout and drawing; `Layout::legend`; margin change.
- [ ] Migrate the two ordinal goldens; tests and goldens for AC2–AC4.

## Dependencies

### Prerequisite Stories

- GUP-424: Multi-Layer Plot 📋 — several layers' legends.
- GUP-425: `Theme` and Axis Titles 📋 — text styling and the margins legends
  share.
- GUP-419: RFC-001 S5b ✅ — `Legend`, `ColorScale::legend`.

### Soft Dependencies

- GUP-433: Shared Key Dictionaries 📋 — makes a shared categorical legend's keys
  the union of its layers'.

### Enables Stories

- GUP-437: legends for chains and user functions.
- RFC-001 S10 builders (not yet stories).

## Testing Strategy

- **Unit tests**: legend measurement and the overflow count.
- **Integration tests**: AC1–AC4 layout assertions.
- **Visual validation**: re-blessed `categorical_species`, `diverging_anomaly`;
  new `legend_guides`, `legend_two_layers`.
- **Performance**: legends are measured once per resolve; none expected.

## Success Metrics

- [ ] No gup-core golden builds legend items by hand except
      `wgsl_function_aftershocks` (a chain, GUP-437).

## Risk Assessment

- **Low**: the fixed right-hand column may be wrong for builders. _Mitigation_:
  `Plot::legends(false)` lets callers place their own.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
