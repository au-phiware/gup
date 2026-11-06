---
implementer: sonnet
implementer-reason:
  two transforms at the one place legends are collected, with the stack flag set
  by three builders; order and colour rules settled in Decisions
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/builder/bar.rs,
    crates/gup-core/src/builder/area.rs,
    crates/gup-core/src/builder/histogram.rs,
    crates/gup-core/tests/describe.rs,
    tests/golden/,
  ]
---

# GUP-616: Legends Match What Is Drawn

## Story Overview

**Initiative**: Polish after 0.1 **Status**: 🚧 In Progress **Created**:
2026-11-04

## Context

Two ways a legend disagrees with the marks it explains, both visible on main:

1. **Stacks read upside down.** A colour legend lists keys in first-seen order,
   and a vertical stack draws the first key at the bottom. In the gallery's
   `area_stacked` (`tests/golden/gallery/area_stacked.png`) the stack is, from
   top to bottom, Solar, Wind, Gas, Coal, and the legend reads Coal, Gas, Wind,
   Solar. The same happens for vertical stacked bars and stacked histograms.
2. **Translucent marks match no swatch.** The area builder draws overlapping
   series at half opacity (`builder/area.rs` line 443), and the scatter's
   `opacity` does the same for discs, but each swatch is drawn in the opaque
   scale colour. On a white background a 50% vermillion area is a pale salmon
   that no swatch shows, and where two areas overlap the blend matches neither.

Both are fixed where the plot collects its legends: one `legends` list
(`plot.rs` around line 2068, `guide::dedup` of every layer's `legends()`) feeds
the drawn legend column, the SVG and the text description (GUP-592), so a fix
there reaches all three.

## Read First

- `crates/gup-core/src/plot.rs` around lines 2060–2090 (collecting `legends`)
  and around line 2865 (`DescribeInput { legends: &legends, .. }`).
- `crates/gup-core/src/guide.rs` lines 30–48 (`dedup`).
- `crates/gup-core/src/scale/legend.rs` lines 20–56 (`Legend`, `Swatch`).
- `crates/gup-core/src/selection.rs`: the `Layer` trait (line 1191),
  `fn legends` (around line 1884) and how `Encoding::Const` values are read per
  channel (the vector path around line 1400).
- `crates/gup-core/src/marks.rs`: the `fill_opacity` channels of `Circle`,
  `Rect` and `Area` (lines 54, 126, 540).
- Builders: `bar.rs` `stack` (line 488), `area.rs` `stack` (line 321),
  `histogram.rs` `color` (line 271, which stacks).

## User Story

> "As a reader of a chart, I want the legend to list a stack in the order I see
> it and to show each series in the colour it is drawn, so that I can match a
> swatch to a mark at a glance."

## Acceptance Criteria

### AC1: Vertical stacks list top-down

- [ ] Vertical stacked bars (`gup::bar(..).stack(..)`), stacked areas
      (`gup::area(..).stack(..)`) and stacked histograms
      (`gup::histogram(..).color(..)`) list their colour legend's swatches in
      reverse first-seen order, so the top layer is first.
- [ ] Horizontal stacked bars (`gup::barh(..).stack(..)`), grouped bars,
      overlapping areas and every other chart keep first-seen order.
- [ ] The text description (GUP-592) lists the keys in the same order as the
      drawn legend; `crates/gup-core/tests/describe.rs` gains a stacked-bar case
      asserting it.
- [ ] `tests/golden/gallery/area_stacked.png` and
      `tests/golden/gallery/histogram_stacked.png`, and the
      `gup_core/builder_area_stacked` and `gup_core/bar_value_labels_stacked`
      goldens, are re-blessed and read: the legend's first swatch is the colour
      of the topmost band or segment, and the order matches the stack all the
      way down.

### AC2: Translucent swatches show the drawn colour

- [ ] Each swatch of a legend drawn for translucent marks is the scale colour
      composited at the marks' opacity over the theme background (rule in
      Decisions).
- [ ] An integration test renders `gup::area` with two overlapping keys (default
      0.5 opacity) on the light theme and asserts, for each key, that the
      swatch's centre pixel and a pixel inside that key's area where it does not
      overlap the other differ by at most 2 per channel (RGB, 0–255).
- [ ] The same check passes for a scatter with `.opacity(0.6)` and a colour key
      (swatch vs a disc's centre pixel).
- [ ] Opaque marks are unchanged: every golden without translucent marks or a
      vertical stack is byte-identical (the full golden suites pass without
      `GUP_BLESS`).
- [ ] An area chart with `.outline(1.0)` (fill 0.5, outline lines opaque in the
      same colours) still draws one legend, whose swatches match the fill.

### AC3: All re-blessed images read

- [ ] Besides AC1's, every golden that changes (expected:
      `gup_core/builder_area`, `gup_core/area_mark` if it has a legend, tutorial
      7's area image, any scatter golden with `opacity` and a colour legend) is
      read by eye: the swatch colours match the translucent marks.

## Decisions

- **Where**: in `Plot::resolve`, after `guide::dedup` and before the legends are
  measured. Both transforms run there on the deduplicated list; nothing in
  `guide.rs` or `Legend` changes shape.
- **Stack order flag**: a crate-private
  `Plot::stack_legend(&mut self, on: bool)` storing `legend_top_down: bool`
  (default false). When true, every `Legend::Swatches` in the list has its
  swatches reversed; ramps are untouched. `BarChart` calls it in `materialise`
  when the arrangement is a stack and the orientation is vertical; `AreaChart`
  when `stack` was set; `HistogramChart` when `color` (its stack) was set.
- **Opacity source**: a new `Layer` trait method
  `fn legend_opacity(&self) -> f32` (default 1.0). `Selection` returns the
  constant value of its mark's channel named `fill_opacity` when that channel
  exists and is `Encoding::Const` (or unset: its default), else 1.0 (a column or
  derived opacity counts as opaque). Marks without `fill_opacity` (`Line`,
  `Segment`) return 1.0.
- **Dedup with opacity**: collect `(title, legend, opacity)` per layer; when
  `dedup` merges equal legends, keep the _minimum_ opacity of the merged layers.
  So an area's 0.5 fill and its opaque outline share one legend drawn at 0.5,
  matching the larger mark.
- **Compositing**: for each swatch colour `c` and opacity `o`, `α = c.a × o`; in
  linear light, `lin(out) = lin(bg) × (1 − α) + lin(c) × α`, with `bg` the theme
  background, and `out.a = 1`. Linear because the GPU targets are sRGB formats
  that blend in linear light; AC2's pixel test is the arbiter.
- **SVG**: writes the composited swatch colour like any other; SVG viewers blend
  translucent marks in sRGB, so an SVG swatch may differ from its SVG mark by a
  few levels. Accepted, not fixed here.

## Escalation Triggers

- AC2's pixel test fails by more than 2 levels with the linear composite but
  passes with an sRGB composite (the GPU blends in sRGB space somewhere): stop
  and report which target.
- A builder's stack is drawn with the first key on top (then the flag's meaning
  is reversed for it).
- Reversing swatches changes the legend's colours (it must only reorder them).

## Out of Scope

- Making overlap regions distinguishable (patterns, outlines by default).
- Legend order for `Plot` users (no public setter; ask if wanted).
- Ramps of translucent heatmaps.

## Technical Tasks

- [ ] `Layer::legend_opacity` and its `Selection` impl.
- [ ] Collect opacities, dedup with minimum, composite, reverse when flagged.
- [ ] `stack_legend` calls in three builders.
- [ ] Tests (AC1, AC2), describe test, goldens read.

## Dependencies

### Prerequisite Stories

- GUP-547: Area Builder and Stacked Areas ✅.
- GUP-592: Plot Describe Text Alternative ✅ — the description reads the same
  legends.

## Testing Strategy

- **Unit tests**: the composite function (white background, 50% `#D55E00` gives
  the value the test computes in linear light).
- **Integration tests**: AC2's pixel comparisons; legend order from
  `Layout::texts` (legend labels in vertical order).
- **Visual validation**: the goldens of AC1 and AC3.

## Success Metrics

- [ ] In the gallery's stacked area, the legend reads Solar, Wind, Gas, Coal.
- [ ] Every translucent series' swatch matches its unoverlapped fill within 2
      levels.

## Risk Assessment

- **Medium**: the colour space of blending is assumed, not read from code.
  _Mitigation_: the pixel test decides, and the trigger stops a wrong guess.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (AC1, AC3)
- [ ] `mask wasm-browser`: run it; the scenes draw colour legends, and their
      pixel checks must still pass
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-11-06)

**Question**: the Decisions section says to composite swatches in linear light,
and the escalation trigger says to stop if the pixel test fails with the linear
composite but passes with an sRGB composite. That is what happens: may the
composite be done in sRGB space (`out = bg * (1 - a) + c * a` on the encoded
values)?

**Evidence** (`crates/gup-core/tests/legend_swatches.rs`, rendered through
`render_resolved`, i.e. `ImageTarget`, 640 x 400, light theme):

- Area with `color` (fill 0.5), web `#E69F00`: drawn fill pixel
  `[243, 207, 128]`. Linear composite swatch: `[243, 214, 188]`. sRGB composite
  swatch: matches.
- Scatter, `.opacity(0.6)`, key a: disc centre `[240, 197, 102]` (blue = 255 x
  0.4). Linear swatch `[240, 205, 170]`. sRGB swatch matches.
- With the sRGB composite all three tests pass (area, outlined area, scatter).

So the image target blends on the encoded values (its colour target is not an
sRGB-format view). I did not check the window surface target.

**Options**:

1. Composite in sRGB space (recommended: the pixel test is the arbiter, per the
   story). The unit test and the Decision text change from "linear" to "sRGB".
2. Keep linear and change how the GPU blends (out of scope, touches every
   translucent golden).

**State**: WIP commit holds the sRGB composite, `Layer::legend_opacity`,
`collect_legends`/`dedup_min` in `plot.rs` (`guide::dedup` and its test removed
as dead code, replaced by a `plot.rs` test), `Plot::stack_legend` and its three
builder calls, a describe test, `tests/legend_swatches.rs` (new, not in
`touches:`), and the renamed `builder_area` stacked test. Not done: golden
re-bless and reading, CHANGELOG line, full test runs, `mask merge-check`,
retrospective.
