---
implementer: sonnet
implementer-reason:
  the bar builder adds text layers (GUP-542) with placement, format, headroom
  and colour rules all fixed in Decisions
touches: [crates/gup-core/src/builder/bar.rs]
---

# GUP-544: Bar Value Labels

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

## Context

A bar chart in a report usually prints each bar's value at its end, so the
reader need not trace gridlines; matplotlib has `bar_label`, and most chart
tools a one-flag option. Gup could not, having no way to draw text at a data
position. GUP-542 adds `Plot::text` and `TextLayer`; this story uses it in the
bar builder, for simple, grouped and stacked bars, vertical and horizontal
(GUP-543).

Two details decide whether labels look right. A label above the tallest bar
needs room the automatic domain does not leave; GUP-555 makes text layers widen
an automatic axis on the side their labels reach (and inset an explicit one), so
the builder adds nothing for it. And a label inside a stacked segment must
contrast with that segment's fill, and must not spill out of a segment too thin
to hold it. The Decisions fix this.

## Read First

- GUP-542's Decisions (`TextLayer` API, `within_x`/`within_y`, `color_by`,
  `dx_with`, the annotation trait) and `crates/gup-core/src/text_layer.rs`.
- `crates/gup-core/src/builder/bar.rs` (whole file, after GUP-543:
  `materialise`, `stack_tables`, orientation).
- GUP-555's Decisions ("Per-side extents", "Automatic axes"): how the labels'
  room is made.
- `crates/gup-core/src/scale/mod.rs` lines 520–535 (`MINUS`, the label minus
  sign) and `scale/categorical.rs` lines 150–170 and 303–312 (`keys`, `eval`).
- GUP-541's Decisions (the plot's stored `formats`).

## User Story

> "As an analyst, I want each bar to show its value, so that readers get exact
> numbers without reading the axis."

## Acceptance Criteria

### AC1: The API

- [ ] `BarChart::value_labels()` and `BarChart::value_labels_with(f)` exist for
      both orientations (signatures in Decisions), documented with a doctest
      that asserts the labels' text from `Layout::texts`
      (`TextRole::Annotation`).

### AC2: Placement

- [ ] Simple and grouped vertical bars: a non-negative bar's label is centred
      over the bar with its ink bottom 4 px (±1) above the bar's top; a negative
      bar's label is centred under it, ink top 4 px below the bar's end (tests,
      from `PlacedText::bounds` and the bar's pixel edges).
- [ ] Grouped bars: each label is centred on its own slot, not the band (test:
      label centre x within 1 px of the bar's centre).
- [ ] Horizontal bars: the label sits right of a positive bar's end (ink left 4
      px past it), left of a negative one's, vertically centred on the bar.
- [ ] Stacked bars: each segment's label (its summed value) is centred in the
      segment, and a segment shorter (narrower, horizontal) than its label's ink
      plus 2 px has no label (test with one 3 px segment).
- [ ] Headroom (from GUP-555, no builder code): with an automatic value axis,
      the tallest positive bar's label lies wholly inside `Layout::plot` (and
      the most negative bar's, for negative data), for plot heights 160 and 400,
      and for all-positive data the bars' baseline stays on the axis line (the
      pixel of 0 is `plot.bottom()`, to 0.5 px) (tests).

### AC3: Text and colour

- [ ] Default text: the plot-level tick format of the value axis (GUP-541) if
      one is set, else the default format in Decisions (unit test: `3.0 → "3"`,
      `3.25 → "3.25"`, `3.256 → "3.26"`, `-1.5 → "−1.5"` with U+2212,
      `1234567.0 → "1234567"`).
- [ ] Stacked labels use white or `#111111`, whichever has the higher WCAG
      contrast ratio with the segment's fill (unit test on Okabe-Ito yellow →
      `#111111`, Okabe-Ito blue → white).
- [ ] Golden `gup_core/bar_value_labels` (640×400): grouped vertical bars with
      labels, including one negative bar. Golden
      `gup_core/bar_value_labels_stacked`: stacked horizontal bars with
      in-segment labels and one hidden thin segment. The worker reads both:
      labels legible, none clipped, none overlapping a bar edge.

## Decisions

- **API** (on `BarChart<T, O>`, both orientations):

  ```rust
  pub fn value_labels(self) -> Self;
  pub fn value_labels_with(self, format: impl Fn(f64) -> String + Send + Sync + 'static) -> Self;
  ```

  ```rust
  let chart = gup::bar(sales)
      .x(|s| s.quarter)
      .y(|s| s.total)
      .group(|s| s.region)
      .value_labels_with(|v| format!("${v:.1}M"));
  ```

  Why: "value labels" says what is labelled (not the axis, not the legend); the
  bare form gives a sensible default, and `_with` takes the same closure shape
  as `x_tick_format`. The last call of the two wins.

- **Implementation**: at materialise, for each bar layer, the builder computes
  label rows `BarLabel { key: Arc<str>, value: f64, base: f64, top: f64 }` (per
  row for simple and grouped bars; per category and key, summed, for stacked
  ones, from `stack_tables`), and adds text layers with `plot.text(..)`:
  - simple/grouped: two text layers per bar layer, one for `value >= 0`
    (vertical: `Anchor::BOTTOM`, `dy(-4.0)`; horizontal: `Anchor::LEFT`,
    `dx(4.0)`) and one for negatives (`Anchor::TOP`, `dy(4.0)`; `Anchor::RIGHT`,
    `dx(-4.0)`), positioned at the band key (`x_key`/`y_key`) and `top`; grouped
    layers add the slot offset with `dx_with`/`dy_with` (the same `Derived`
    offset `sub_band` uses);
  - stacked: one text layer per stack layer at `(base + top) / 2`,
    `Anchor::CENTER`, with `within_y(base, top)` (horizontal: `within_x`).
- **Default format**: round to 2 decimals, trim trailing zeros and a trailing
  point, and write the minus as U+2212 (`scale::MINUS`). No thousands
  separators.
- **Headroom**: none in the builder. GUP-555 gives each text layer per-side
  extents: an automatic value axis widens only at the end the labels reach (so
  the baseline stays on the axis), an explicit one is inset there, up to a
  quarter of the plot per side. No `fit_value` or `extents` hook, and no edit to
  `plot.rs` or `scale/mod.rs`.
- **Colours**: simple and grouped labels use `Theme::label`. Stacked labels use
  `color_by`: the segment fill is `fill(c)` when set, else the colour scale's
  colour for the stack key (`Categorical::keys()` position, then
  `CpuMirror::eval`), read at resolve; with `.color(key)` overriding the stack
  colours, labels use `Theme::label`.
- **Size**: `Theme::tick_size`; weight `Regular`.

## Escalation Triggers

- GUP-555's room leaves a label clipped at plot height 160 (AC2's test), or
  lifts the baseline off the axis: stop and report the measured overflow; do not
  add a builder-side headroom.
- Value labels change any existing bar golden (charts without labels must be
  untouched).
- `within_y` hides labels that visibly fit, or shows ones that spill (GUP-542's
  rule is wrong for this use): report with the pixel numbers.

## Out of Scope

- Stack totals above stacked bars; labels on other builders (a scatter's point
  labels use `Plot::text` directly).
- Collision avoidance between neighbouring labels.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] Label rows and text layers for simple and grouped vertical bars; tests;
      commit.
- [ ] Headroom tests at both heights (GUP-555 makes the room); commit.
- [ ] Stacked and horizontal placement, contrast colours; goldens; commit.

## Dependencies

### Prerequisite Stories

- GUP-542: Text layer ✅ — `Plot::text`, `TextLayer`, anchors.
- GUP-543: Horizontal bars ✅ — the orientation parameter.
- GUP-555: Text labels make room at the plot edge 📋 — the headroom above
  (beside) bar ends, per side.
- GUP-537: owned category keys ✅ (soft, via GUP-543) — bar keys are labels
  (`Arc<str>`), which `BarLabel::key` reuses.
- GUP-541: Builder tick formats 📋 (soft) — the stored value-axis format for the
  default text.

### Enables Stories

- GUP-536: the chart tour shows labelled bars.

## Testing Strategy

- **Unit tests**: the default format; the contrast choice.
- **Integration tests**: placement, slot centring, hiding, headroom.
- **Visual validation**: the two goldens, read by eye.

## Success Metrics

- [ ] `.value_labels()` labels every bar of every bar chart variant legibly.

## Risk Assessment

- **Low**: headroom comes from GUP-555's pixel extents, not a data-unit guess.
  _Mitigation_: the tests pin the smallest supported height.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the two goldens)
- [ ] `mask wasm-browser`: not needed, builder only
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
