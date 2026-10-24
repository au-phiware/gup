---
implementer: sonnet
implementer-reason:
  the bar builder gains an orientation type parameter and one setter; the API,
  slot order and axis order are fixed in Decisions
touches:
  [
    crates/gup-core/src/builder/bar.rs,
    crates/gup-core/src/builder/mod.rs,
    crates/gup-core/src/lib.rs,
  ]
---

# GUP-543: Horizontal Bars and Group Padding

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

## Context

`gup::bar` draws vertical bars only; its migration table says "`.orient(..)` /
`.horizontal()`: dropped" (`builder/mod.rs`). Horizontal bars are the standard
chart for ranked categories with long names (countries, products, survey
answers), which a vertical axis cannot label legibly. The strategic review lists
horizontal bars under N2 "bar completeness".

Grouped bars also use a fixed gap between the bars of a group,
`SLOT_PADDING = 0.1` of a slot (`builder/bar.rs`), with no setter; the review's
triage lists `group_padding`. The band's own padding (`padding`) already has a
setter.

The `Plot` underneath already supports both: `Band` works on a y axis
(`tests/rect_png.rs` line 260, `tests/tick_labels.rs` line 124), and
`Rect::HEIGHT` with `band_width_px` sizes a bar across a y band as `Rect::WIDTH`
does across an x band.

## Read First

- `crates/gup-core/src/builder/bar.rs` (whole file, 488 lines).
- `crates/gup-core/src/scale/band.rs` lines 108–300 (`Band`, `band_width`,
  `band_width_px`, `sub_band`, `sub_band_width_px`).
- `crates/gup-core/src/marks.rs` lines 64–140 (`Rect`'s edge and extent
  channels).
- `crates/gup-core/tests/builders.rs` lines 540–760 (the bar tests and goldens).
- `crates/gup-core/src/builder/mod.rs` lines 60–100 (the migration table).

## User Story

> "As an analyst, I want horizontal bars with my category names down the left,
> so that long names stay readable and ranked data reads top to bottom."

## Acceptance Criteria

### AC1: `gup::barh`

- [ ] `gup::barh(rows)` returns `BarChart<T, Horizontal>` with `y(key)` (the
      category, on a band y axis), `x(value)` (drawn from 0) and
      `x_scale(scale)`, and every orientation-independent setter of `BarChart`
      (`padding`, `group_padding`, `color`, `color_scale`, `fill`, `group`,
      `stack`, `opacity`, `legend_title`, `tooltip`, the plot setters,
      `layers_mut`, `plot_mut`, `into_plot`, `pick`).
- [ ] `BarChart<T>` keeps compiling unchanged (default type parameter): every
      existing bar test, doctest and example passes without edits.
- [ ] Categories run top to bottom in first-seen order: the first row's category
      is the topmost band (test from `Layout::y_ticks`).
- [ ] Grouped horizontal bars: within each band the first-seen group key is the
      topmost bar (test), so bars read in legend order.
- [ ] Stacked horizontal bars stack rightward from 0 in first-seen key order,
      negatives leftward (test reusing `stack_tables`).
- [ ] A `rust,compile_fail` doctest on `barh` shows `.x(|r| r.name.as_str())`
      does not compile (x is the value on a horizontal chart).

### AC2: `group_padding`

- [ ] `BarChart::group_padding(p)` sets the gap between the bars of a group as a
      fraction of a slot (default 0.1), for both orientations; it panics unless
      `0.0 <= p < 1.0`, with a message giving `p`.
- [ ] Test: with `group_padding(0.0)` adjacent group bars in a band touch (no
      background pixel between them on a row through the bars); with `0.5` each
      bar is half its slot wide (±1 px).

### AC3: Visual

- [ ] Golden `gup_core/builder_barh` (480×320): five countries with long names,
      grouped by two years, legend at the right. The worker reads it: names
      fully visible left of the axis, bars start at the y axis, the first
      country at the top, legend order matches the bar order.
- [ ] The migration table's `.orient(..)`/`.horizontal()` row says
      `gup::barh(rows)`.

## Decisions

- **API** (`BarChart<T, O = Vertical>`, with `O: Orientation`, a sealed trait
  implemented by the zero-sized `Vertical` and `Horizontal` in
  `gup_core::builder`):

  ```rust
  pub fn barh<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> BarChart<T, Horizontal>;

  impl<T: Send + Sync + 'static> BarChart<T, Horizontal> {
      pub fn y<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn x_scale<S: PositionScale>(self, scale: S) -> Self;
  }
  impl<T: Send + Sync + 'static, O: Orientation> BarChart<T, O> {
      pub fn group_padding(self, p: f64) -> Self;
      // and every existing orientation-independent setter, moved here
  }
  ```

  ```rust
  let chart = gup::barh(survey)
      .y(|r| r.answer.clone())
      .x(|r| r.share)
      .group(|r| r.year);
  ```

  Why: every builder names its accessors by the axis they land on
  (`scatter(..).x(..).y(..)`), so a horizontal chart's categories are `y`. A
  separate function keeps the types honest (the category accessor and the value
  accessor have different types, so one `.horizontal()` flag would make `x` mean
  "category" on a chart where categories are vertical). `barh` is matplotlib's
  name for the same chart; `#[doc(alias = "horizontal")]` on it helps docs
  search.

- **Exports**: `barh` from the crate root beside `bar`; `Orientation`,
  `Vertical`, `Horizontal` from `gup_core::builder` only (not the root or the
  prelude).
- **Axis order**: the horizontal chart passes its categories to the band's
  `.domain(..)` reversed, so the first-seen category takes the top band (a y
  band's range runs bottom to top).
- **Slot order**: in a horizontal chart, group key `i` uses slot `count - 1 - i`
  of `sub_band`, so the first key is the topmost bar.
- **One implementation**: the materialise code is shared; orientation decides
  which plot axis gets the band, which `Rect` channels get the band position and
  extent (`x`/`width` or `y`/`height`) and which get the value and baseline
  (`y`/`y_end` or `x`/`x_end`). No code is copied per orientation.
- **`group_padding` replaces `SLOT_PADDING`** as a field defaulting to 0.1.

## Escalation Triggers

- A `Band` on the y axis mislabels or misplaces ticks with reversed keys (a bug
  outside the builder): stop and report with a minimal `Plot` repro.
- Making `BarChart` generic changes any existing golden or breaks a public use
  outside `builder/bar.rs` (for example `gup-egui` naming `BarChart<T>`).
- `mask wasm-size` scatter grows by more than 4 KB gz (the scatter does not use
  bars).

## Out of Scope

- Value labels (GUP-544).
- Sorting categories by value (users order their rows; first-seen order).
- Horizontal versions of other builders.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] Introduce `Orientation` and move shared setters to the generic impl; all
      bar tests green; commit.
- [ ] Add `barh`, the axis and slot order, tests; commit.
- [ ] Add `group_padding` and its tests; the golden; docs; commit.

## Dependencies

### Prerequisite Stories

- GUP-464: bar builder ✅; GUP-461: offsets and sub-bands ✅.
- GUP-537: owned category keys 📋 — `y` and the moved setters take owned keys
  (`K: DictionaryKey`, through `builder::erase_key`), as `bar`'s do.

### Enables Stories

- GUP-544: bar value labels cover both orientations.

## Testing Strategy

- **Integration tests**: `builders.rs` bar module: order, slots, stacking,
  padding.
- **Visual validation**: `gup_core/builder_barh`, read by eye.

## Success Metrics

- [ ] A ranked horizontal bar chart is one call chain, with the same setters as
      the vertical one.

## Risk Assessment

- **Low**: the generic parameter shows in error messages. _Mitigation_: the
  default keeps `BarChart<T>` the common spelling; docs show `barh`'s type once.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (`gup_core/builder_barh`)
- [ ] `mask wasm-browser`: not needed, builder only (no layout rule change)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
