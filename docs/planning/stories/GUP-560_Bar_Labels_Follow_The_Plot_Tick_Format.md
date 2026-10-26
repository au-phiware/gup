---
implementer: sonnet
implementer-reason:
  a shared format slot on Plot and one closure change in the bar builder, all
  signatures fixed in Decisions
touches: [crates/gup-core/src/plot.rs]
---

# GUP-560: Bar Labels Follow the Plot Tick Format

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-27) Planned **Created**: 2026-10-26

## Context

GUP-544 prints bar values with the value axis's tick format. `Plot` keeps that
format in a private field (`formats: [Option<TickFormat>; 3]`, `plot.rs` line
172), so the bar builder keeps a second copy (`BarChart::value_format`,
`builder/bar.rs` line 232), written only by the builder's own `y_tick_format`/
`x_tick_format`. A format set through `chart.plot_mut().y_tick_format(..)`
labels the ticks but not the bars, so the two disagree (GUP-544 retrospective,
"Architectural Decisions").

There is a second problem behind the first. `plot_mut()` materialises the chart,
which builds the label text layers with their format closure already captured
(`add_value_labels`, `bar.rs` line 824). Any format change after that point is
too late for a captured copy, and a text layer evaluates its closures only once,
on the first resolve after one of its own setters ran (`TextLayer::changed`,
`text_layer.rs` line 168).

This story makes the plot's stored format the one source of truth: the slot is
shared, the label closure reads it when the labels are evaluated, and setting a
format re-evaluates the plot's text layers.

## Read First

- `crates/gup-core/src/plot.rs`: the `formats` field (lines 170–172),
  `x_tick_format`/`y_tick_format`/`y_right_tick_format`, `set_format` and
  `apply_format` (lines 620–707), and the key-scale check that reads `formats`
  (around line 1811).
- `crates/gup-core/src/builder/bar.rs`: `value_format` (line 232), the two
  `*_tick_format` methods (lines 286–330), `materialise`'s call of
  `add_value_labels` (line 768) and `add_value_labels` (lines 824–840),
  `default_label` (line 82), `plot_mut` (line 929).
- `crates/gup-core/src/text_layer.rs`: `changed` (line 168), the closure-caching
  note on `TextLayer` (lines 118–126), the `AnnotationLayer` trait (lines
  443–478).
- `crates/gup-core/tests/bar_value_labels.rs`: the `render` and `labels` helpers
  the new tests reuse.

## User Story

> "As a developer building a bar chart, I want value labels to use whatever tick
> format the value axis has, however I set it, so that the labels and the axis
> never disagree."

## Acceptance Criteria

### AC1: A format set through `plot_mut()` reaches the labels

- [x] New test in `crates/gup-core/tests/bar_value_labels.rs`: `gup_core::bar`
      over three rows (3.0, 5.25, −1.5) with `.value_labels()`, then
      `chart.plot_mut().y_tick_format(|v| format!("{v:.0}%"))`; after
      `render_resolved`, the sorted Annotation texts are exactly
      `["-2%", "3%", "5%"]` (the closure's own output, ASCII minus) and every y
      tick label ends in `%`.
- [x] The same for `gup_core::barh` with `plot_mut().x_tick_format(..)`.
- [x] A stacked chart (`.stack(..)`) with `plot_mut().y_tick_format(..)`: every
      in-segment label uses the format.

### AC2: A format changed after a resolve is picked up

- [x] New test: resolve a chart with `.value_labels()` and no format (labels
      read `3`, `5.25`), then
      `chart.plot_mut().y_tick_format(|v| format!("{v} u"))` and resolve again:
      the labels read `3 u`, `5.25 u`.

### AC3: Existing behaviour holds

- [x] The builder's own `y_tick_format`/`x_tick_format` still label both ticks
      and bars (the existing GUP-544 tests pass unchanged).
- [x] `value_labels_with(f)` still wins over any axis format (existing test).
- [x] `rg -n 'value_format' crates/gup-core/src/builder/bar.rs` prints nothing.
- [x] Every golden passes unchanged (no rendered output changes):
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`.

### AC4: Docs

- [x] `BarChart::value_labels`' doc says the text is "the value axis's tick
      format, however it was set (on the builder or through `plot_mut()`)", and
      its doctest is unchanged.

## Decisions

- **The shared slot** lives in `plot.rs`:

  ```rust
  /// An axis's tick format as the plot stores it, shared with what formats
  /// values the way the axis does (bar value labels).
  #[derive(Clone, Default)]
  pub(crate) struct AxisFormat(Arc<RwLock<Option<TickFormat>>>);

  impl AxisFormat {
      pub(crate) fn get(&self) -> Option<TickFormat>;
      fn set(&self, format: TickFormat);
  }

  impl Plot {
      /// The stored tick format slot of axis `axis` (0 x, 1 y, 2 right y).
      pub(crate) fn axis_format(&self, axis: usize) -> AxisFormat;
  }
  ```

  `formats: [AxisFormat; 3]`. Locks recover from poisoning with
  `unwrap_or_else(|p| p.into_inner())`, as `ScaleRef` does.

- **`set_format`** writes the slot, applies it to the scale as today, and calls
  a new `AnnotationLayer::invalidate(&mut self)` on every text layer (the
  `TextLayer` impl calls `self.changed()`), so labels evaluated earlier are
  evaluated again. Tick formats change rarely; invalidating every text layer is
  simpler than tracking which layers read the slot.
- **The bar builder** drops `value_format` and the `Format` copies in its
  `*_tick_format` methods (they forward to the plot only). For
  `LabelSpec::Default`, `add_value_labels` captures
  `self.plot.axis_format(if O::HORIZONTAL { 0 } else { 1 })` and formats with
  `slot.get().map_or_else(|| default_label(v), |f| f.apply(v))`.
- **Out of the slot's reach**: a format set only with `ScaleRef::tick_format` on
  a scale handle is not seen by the labels (documented in one sentence on
  `value_labels`). That path bypasses the plot, and the plot already documents
  that "the last of this and `ScaleRef::tick_format` wins" for ticks.

## Escalation Triggers

- Any golden image changes.
- `AxisFormat` makes `Plot` lose `Send` or `Sync`, or forces a public type to
  change.
- Invalidating text layers on `set_format` breaks a resolve-cache test
  (`tests/resolve_cache.rs`) in a way that is not just a recount of evaluations.

## Out of Scope

- Value-axis rounding to ticks and band padding (GUP-561).
- Labels for heatmap cells or other builders (not planned in N2).

## Technical Tasks

- [x] Add `AxisFormat` and `Plot::axis_format`; switch `formats` and its three
      readers (`set_format`, `apply_format`, the key-scale check).
- [x] Add `AnnotationLayer::invalidate` and call it from `set_format`.
- [x] Remove `value_format` from `BarChart`; read the slot in
      `add_value_labels`.
- [x] Tests for AC1 and AC2; doc line for AC4.

## Dependencies

### Prerequisite Stories

- GUP-544: Bar Value Labels ✅ — the labels and the builder copy this replaces.
- GUP-541: Builder Tick Formats ✅ — `Plot::*_tick_format` and the stored
  formats.

### Enables Stories

None directly.

## Testing Strategy

- **Integration tests**: AC1 and AC2 in `tests/bar_value_labels.rs`, asserting
  `Layout.texts` with `TextRole::Annotation`.
- **Visual validation**: none needed beyond every golden passing unchanged.

## Success Metrics

- [x] One stored copy of each axis's format (`value_format` gone).
- [x] AC1's three orientations/arrangements and AC2 pass.

## Risk Assessment

- **Low**: invalidating all text layers on a format change costs one
  re-evaluation per layer. _Mitigation_: formats are set at build time, not per
  frame.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no golden changes (AC3)
- [x] `mask wasm-browser` not needed: no layout, input or wasm code changes
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`AxisFormat` (shared `Arc<RwLock<Option<TickFormat>>>`) replaces
`Plot::formats`; `Plot::axis_format` hands the slot out. `set_format`
invalidates every text layer through the new `AnnotationLayer::invalidate`.
`BarChart::value_format` is gone; default labels read the slot at evaluation.
Files: `plot.rs`, `text_layer.rs`, `builder/bar.rs`, `tests/bar_value_labels.rs`
(4 new tests; 14 in the file pass). Full `gup-core` suite, `mask merge-check`
and `cargo test --doc -p gup` pass; no golden changed. `mask wasm-size`: scatter
gz 342158 bytes (limit 344733).

## Retrospective

**Completed**: 2026-10-27

### Key Technical Learnings

#### One shared slot beats two copies

- **Challenge**: Labels captured the format at materialise, too early for
  `plot_mut()` changes.
- **Solution**: Capture the slot, not the value; read it at evaluation, and
  invalidate text layers on set.
- **Pattern**: Share the mutable source of truth and invalidate caches on write.

### Architectural Decisions

#### Invalidate every text layer on a format change

- **Decision**: As the story specified.
- **Trade-off**: One extra re-evaluation per layer; formats are set at build
  time.

### Development Workflow Insights

- No resolve-cache test was affected.

### Follow-up Stories

None.
