---
implementer: sonnet
implementer-reason:
  Theme fields, the null-colour plumbing, title placement and truncation are
  settled in Decisions; layout follows the chart title's existing pattern
---

# GUP-425: RFC-001 Step S7b: `Theme` and Axis Titles

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-11

## Context

`crates/gup-core/src/plot.rs`' `style` module (lines 26–41) is provisional:
"Fixed S0a styling (a `Theme` is RFC-001 S7 / GUP-392)". It hard-codes colours,
text sizes, gaps and tick spacing as `const`s, and `Plot::resolve` draws one
chart title and no axis titles: no gup-core chart has axis titles. RFC-001's
"S5b findings → Proposed adjustments" lists, for S7: "`theme.null_color`
replaces `NULL_COLOR` in the glue literal and in `Categorical`'s `Params`" and
"Axis titles. No chart has them yet."

The null colour is used in two places. The glue's `Expr::OrNull`
(`crates/gup-core/src/shader/glue.rs` lines 178–185) prints `NULL_COLOR` as a
WGSL literal for a null numeric colour input (a `Sequential` or `Diverging`
null). `Categorical::params` (`scale/categorical.rs` lines 241–250) passes
`NULL_COLOR` to its WGSL, which draws it for `NULL_CODE` or an out-of-domain
code, and `Categorical`'s CPU mirror returns it too (line 275). A theme value
must reach both without making the glue text depend on a colour.

On 2026-10-16 this story was split to fit one session: tick-label quality (log
`nice()`, count suffixes, collision-aware labels) moved to
[GUP-435](GUP-435_RFC_001_S7f_Tick_Label_Quality.md), and the second font weight
for `Time` year ticks to
[GUP-436](GUP-436_RFC_001_S7g_Semibold_Time_Year_Ticks.md).

## Read First

- RFC-001 "S5b findings" → "Proposed adjustments to S5c, S6, S7" (lines
  2791–2826), the S7 bullets.
- `crates/gup-core/src/plot.rs` lines 26–41 (`style`), 197–270 (resolve steps
  1–5: measurement and the plot rect) and 300–380 (tick labels, title, texts).
- `crates/gup-core/src/shader/glue.rs` lines 170–190 (`Expr::OrNull`) and
  330–365 (where `OrNull` is emitted).
- `crates/gup-core/src/scale/categorical.rs` lines 15–20, 225–280.
- `crates/gup-core/src/encoding.rs` lines 120–145 (`EncodeFn`), 400–430
  (`Then`'s `EncodeFn`) and 735–810 (`DynColumnEncoding` and impls).
- `crates/gup-core/src/selection.rs` lines 405–420 (`Layer`) and 470–498
  (`prepare`'s `Encodings` bytes).
- `crates/gup-core/src/scene.rs` lines 255–300 (`TextStyle`, `TextRole`,
  `TextRun`).

## User Story

> "As a visualization developer, I want axis titles and a `Theme` I can override
> (including the colour drawn for missing values), so that a default chart is
> labelled and its styling is not hard-coded."

## Acceptance Criteria

### AC1: `Theme` reproduces today's styling

- [ ] `gup_core::Theme` (crate root and prelude) exists with the fields in
      Decisions; `Theme::default()` equals the old `style` values and
      `NULL_COLOR` (unit test, field by field).
- [ ] The `style` module is deleted; `resolve` reads `self.theme`.
- [ ] Every existing gup-core golden passes without re-blessing.

### AC2: Themed null colour

- [ ] `Plot::theme(&mut self, theme: Theme) -> &mut Self`. With
      `null_color = #FF00FF`: a `Sequential` fill with a NaN row draws that
      row's centre in `#FF00FF` within 2/255, and a `Categorical`
      `encode_nullable_key` fill with a `None` row draws in `#FF00FF` within
      2/255.
- [ ] `Categorical::null_color(self, c: Color) -> Self` overrides the theme for
      that scale (test: theme `#FF00FF`, scale `#00FF00`, the categorical null
      row draws `#00FF00`).

### AC3: Axis titles

- [ ] `Plot::x_title(&mut self, t: impl Into<String>) -> &mut Self` and
      `Plot::y_title(..)`, placed as in Decisions.
- [ ] Golden `gup_core/axis_titles` (720×450): the `sqrt_radius` scatter with
      title "Wealth and health", x title "GDP per capita (US$)" and y title
      "Life expectancy (years)". A test asserts every pair of `layout.texts` ink
      boxes is disjoint, every box lies inside the canvas, and no axis title box
      intersects `layout.plot`.
- [ ] Read by eye: titles legible, the x title centred under the x tick labels,
      the y title above the y axis, left-aligned. The retrospective describes
      the PNG.

### AC4: Long titles are truncated

- [ ] A 300-character x title on a 320×200 plot resolves without error; its
      run's text ends in "…" and its ink box fits within the plot width. The
      chart title and y title follow the same rule (test each).

## Decisions

- **`Theme`** (`crates/gup-core/src/theme.rs`),
  `#[derive(Clone, Debug, PartialEq)]`, all fields `pub`, defaults in brackets:
  `background: Color` (`WHITE`), `axis: Color` (`#333333`), `label: Color`
  (`#333333`), `title: Color` (`#111111`), `title_size: Px` (18),
  `axis_title_size: Px` (13), `tick_size: Px` (12), `tick_len: Px` (5),
  `label_gap: Px` (4), `pad: Px` (12), `x_tick_spacing: Px` (80),
  `y_tick_spacing: Px` (45), `null_color: Color` (`NULL_COLOR`, `#999999`). No
  font or palette field: nothing would read them yet (builders, RFC-001 S10).
- **Glue null colour**: when the glue emits any `OrNull`, it declares a
  `null_color: vec4<f32>` field in the `Encodings` struct and `OrNull` reads
  `encodings.null_color` instead of printing a literal. `Layer::prepare` becomes
  `prepare(&mut self, cx: &Context, theme: &Theme)` and writes
  `theme.null_color` into that field.
- **Categorical null colour**: `Categorical` gains `null_color: Color` and
  `null_color_set: bool` (private), used by `params()` and `eval()`. `ShaderFn`
  gains a provided `fn apply_theme(&mut self, theme: &Theme) {}`; `Categorical`
  overrides it to adopt `theme.null_color` unless set by
  `Categorical::null_color`; `ScaleRef` forwards through `write()`. `EncodeFn`
  gains `fn apply_theme(&mut self, theme: &Theme)` (blanket impl forwards to
  `ShaderFn::apply_theme`; `Then` applies to both halves); `DynColumnEncoding`
  and `KeyEncoded` forward to their `func`; `Layer` gains
  `apply_theme(&mut self, theme: &Theme)`. `Plot::resolve` calls it on every
  layer before fitting. `NULL_COLOR` stays as the default's value.
- **Placement**: the x title is centred on the plot rect, its top `label_gap`
  below the x tick labels' bottom; the bottom margin grows by its height (cap
  height + descent) plus `label_gap`. The y title is horizontal, left-aligned at
  `x = pad`, its bottom `label_gap` above the plot rect's top; the top margin
  grows by its height plus `label_gap` (below the chart title when there is
  one). No rotated text.
- **Truncation**: a title wider than its space (chart and x title: the plot
  width; y title: `width - 2 * pad`) drops trailing characters until the text
  plus "…" (U+2026, in the bundled subset) fits. A helper
  `fn fit_text(text: &TextSystem, s: &str, size: f32, max: f32) -> String` in
  `plot.rs`.
- **`TextRole::AxisTitle`** is a new variant.

## Escalation Triggers

- Any existing golden changes with the default theme.
- Adding `null_color` to the `Encodings` struct changes the uniform layout of
  programs that have no nulls.
- A trybuild snapshot changes (new provided trait methods should not affect
  them).

## Out of Scope

- Log `nice()`, count suffixes, collision-aware labels: GUP-435.
- A second font weight and `Time` year ticks: GUP-436.
- Legends: GUP-426.
- Theme presets (dark), fonts, palettes in the theme: not yet a story.

## Technical Tasks

- [ ] Add `theme.rs`; replace `style` with `Plot::theme`.
- [ ] Glue `null_color` field; `Layer::prepare` takes the theme.
- [ ] `apply_theme` through `ShaderFn`, `EncodeFn`, `Then`, `DynColumnEncoding`,
      `KeyEncoded`, `ScaleRef`, `Layer`; `Categorical`'s field and builder.
- [ ] Axis titles, truncation helper, `TextRole::AxisTitle`.
- [ ] Tests for AC1–AC4; golden `axis_titles`.

## Dependencies

### Prerequisite Stories

- GUP-424: Multi-Layer Plot 📋 — the multi-layer `resolve` this story edits
  (avoids conflicting rewrites of the same function).
- GUP-419: RFC-001 S5b ✅ — `NULL_COLOR`, `Categorical`.

### Enables Stories

- GUP-426: legends read the theme and sit beside axis titles.
- GUP-435, GUP-436: tick labels in the themed layout.

## Testing Strategy

- **Unit tests**: `Theme::default()` values; `fit_text`.
- **Integration tests**: AC2 null-colour renders; AC3 non-overlap; AC4
  truncation.
- **Visual validation**: `gup_core/axis_titles`.
- **Performance**: none.

## Success Metrics

- [ ] No hard-coded style constants remain in `plot.rs`.

## Risk Assessment

- **Medium**: `apply_theme` touches many trait impls. _Mitigation_: all are
  provided or forwarding methods; the compiler lists every impl.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
