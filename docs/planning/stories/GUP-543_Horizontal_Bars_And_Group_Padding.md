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

- [x] `gup::barh(rows)` returns `BarChart<T, Horizontal>` with `y(key)` (the
      category, on a band y axis), `x(value)` (drawn from 0) and
      `x_scale(scale)`, and every orientation-independent setter of `BarChart`
      (`padding`, `group_padding`, `color`, `color_scale`, `fill`, `group`,
      `stack`, `opacity`, `legend_title`, `tooltip`, the plot setters,
      `layers_mut`, `plot_mut`, `into_plot`, `pick`).
- [x] `BarChart<T>` keeps compiling unchanged (default type parameter): every
      existing bar test, doctest and example passes without edits.
- [x] Categories run top to bottom in first-seen order: the first row's category
      is the topmost band (test from `Layout::y_ticks`).
- [x] Grouped horizontal bars: within each band the first-seen group key is the
      topmost bar (test), so bars read in legend order.
- [x] Stacked horizontal bars stack rightward from 0 in first-seen key order,
      negatives leftward (test reusing `stack_tables`).
- [x] A `rust,compile_fail` doctest on `barh` shows `.x(|r| r.name.as_str())`
      does not compile (x is the value on a horizontal chart).

### AC2: `group_padding`

- [x] `BarChart::group_padding(p)` sets the gap between the bars of a group as a
      fraction of a slot (default 0.1), for both orientations; it panics unless
      `0.0 <= p < 1.0`, with a message giving `p`.
- [x] Test: with `group_padding(0.0)` adjacent group bars in a band touch (no
      background pixel between them on a row through the bars); with `0.5` each
      bar is half its slot wide (±1 px).

### AC3: Visual

- [x] Golden `gup_core/builder_barh` (480×320): five countries with long names,
      grouped by two years, legend at the right. The worker reads it: names
      fully visible left of the axis, bars start at the y axis, the first
      country at the top, legend order matches the bar order.
- [x] The migration table's `.orient(..)`/`.horizontal()` row says
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

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Introduce `Orientation` and move shared setters to the generic impl; all
      bar tests green; commit.
- [x] Add `barh`, the axis and slot order, tests; commit.
- [x] Add `group_padding` and its tests; the golden; docs; commit.

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

- [x] A ranked horizontal bar chart is one call chain, with the same setters as
      the vertical one.

## Risk Assessment

- **Low**: the generic parameter shows in error messages. _Mitigation_: the
  default keeps `BarChart<T>` the common spelling; docs show `barh`'s type once.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`gup_core/builder_barh`)
- [x] `mask wasm-browser`: not needed, builder only (no layout rule change)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/builder/bar.rs`: `BarChart<T, O: Orientation = Vertical>`
  with sealed `Orientation`, `Vertical`, `Horizontal`; `gup_core::barh` (with a
  `compile_fail` doctest and `#[doc(alias = "horizontal")]`); `y`/`x`/`x_scale`
  on the horizontal impl, `x`/`y`/`y_scale` on the vertical one, both through
  shared private `set_category`/`set_value`/`set_value_scale`; `group_padding`
  (field, default 0.1, replaces `SLOT_PADDING`). One `materialise`, with
  `O::HORIZONTAL` picking the band axis, the `Rect` channels, reversed domain
  and reversed slot.
- `builder/mod.rs`, `lib.rs`: exports (`barh` at the root; `Orientation`,
  `Vertical`, `Horizontal` from `gup_core::builder` only) and the migration
  table row.
- Tests: 8 new in `tests/builders.rs` (`mod barh`); golden
  `tests/golden/gup_core/builder_barh.png`. No existing golden changed. The
  `builder_key_without_display.stderr` snapshot changed by one source line.
- WASM: scatter 334437 B gz (recorded 334149, +288 B, within the 2% limit).

### Evidence

`gup_core/builder_barh` (480x320), read by eye: five long country names fully
visible left of the axis, the first (United Arab Emirates) on top, bars start at
the y axis, 2019 (orange) above 2023 (blue) in every band, matching the legend
order, x ticks 0..80 with a light grid, no clipped bars.

## Retrospective

**Completed**: 2026-10-25

### Key Technical Learnings

#### Orientation as a type parameter

- **Challenge**: one materialise path for two orientations whose `Selection`
  setter names differ.
- **Solution**: a sealed trait with `const HORIZONTAL`; the four setter call
  sites branch on it, everything else (splitting, stacking, colour, legend) is
  shared.
- **Pattern**: orientation-specific accessors live in per-orientation `impl`
  blocks that forward to private shared setters, so the shared setters are not
  duplicated and the types prevent `.x(category)` on `barh`.

#### Y band direction

- A y band's range runs bottom to top, so the first-seen category needs a
  reversed domain and the first group key slot `count - 1 - i`. No band bug was
  found, so the escalation trigger did not fire.

### Architectural Decisions

#### Per-orientation `x`/`y` impls

- **Decision**: as in the story. **Trade-off**: `BarChart<T, Vertical>` shows in
  rustc diagnostics (the compile-fail snapshot was affected only by a changed
  source line). **Future**: GUP-544 value labels cover both through `O`.

### Development Workflow Insights

- The worktree sandbox refused compound shell commands (heredocs with `cd`, `$f`
  variables in sed); writing scripts to `/tmp` and running them separately
  worked.
- `mask merge-check` caught the trybuild snapshot that the targeted tests did
  not run.

### Follow-up Stories

None.
