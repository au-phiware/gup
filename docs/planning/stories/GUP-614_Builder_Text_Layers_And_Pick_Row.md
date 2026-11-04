---
implementer: sonnet
implementer-reason:
  two methods added once to the shared plot_setters macro, plus a tutorial and
  its golden mirror; signatures settled in Decisions
touches:
  [
    crates/gup-core/src/builder/mod.rs,
    docs/tutorials/07_chart_types.md,
    tests/tutorials.rs,
  ]
---

# GUP-614: Builder Text Layers and Pick Row

## Story Overview

**Initiative**: Polish after 0.1 **Status**: 📋 Planned **Created**: 2026-11-04

## Context

Two gaps make a builder user reach for `plot_mut()` where a builder method
should do:

- **Annotations.** `Plot::text(rows)` (GUP-542) adds a text layer, but no
  builder has it. Tutorial 7's "Annotations" block (and its golden mirror,
  `tutorial_7_text` in `tests/tutorials.rs` line 564) breaks the builder chain
  to write `chart.plot_mut().text(cities).x(..).y(..).text(..)`.
- **Picking by tuple.** GUP-563 made `Plot::pick_row`, `pick`, `zoom` and
  `invert` take `impl Into<Point>`, and every builder has an inherent
  `pick(impl Into<Point>)`. But `pick_row` on a builder is only the `Chart`
  trait method, `fn pick_row(&mut self, at: Point)` (`chart.rs` line 62), which
  cannot be generic because `Chart` is used as `dyn Chart`. So
  `chart.pick_row((120.0, 80.0))` does not compile on a `ScatterChart`, while
  `chart.plot_mut().pick_row((120.0, 80.0))` does. (Checked on main: tutorial 4
  does not call `pick_row` today; the gap is in the API, not a tutorial.)

All seven builders share the `plot_setters!` macro (`builder/mod.rs` line 235;
its `titles` arm is used by all seven, the full arm by five). Adding both
methods to the `titles` arm covers the class at once.

## Read First

- `crates/gup-core/src/builder/mod.rs` lines 233–316 (`plot_setters!`).
- `crates/gup-core/src/plot.rs`: `Plot::text` (around line 942) and its doc
  comment; `Plot::pick_row` (around line 1851).
- `crates/gup-core/src/builder/scatter.rs` lines 330–370: `plot_mut`, `pick` and
  the `Chart` impl (every builder has a private `materialise()`).
- `docs/tutorials/07_chart_types.md` "Annotations" (around line 292) and
  `tests/tutorials.rs` `tutorial_7_text` (line 564).

## User Story

> "As a visualization developer, I want to add labels and pick rows on a builder
> chart directly, so that I do not drop to `plot_mut()` for common tasks."

## Acceptance Criteria

### AC1: `.text(rows, layer)` on every builder

- [ ] Every builder (`scatter`, `line`, `area`, `bar`/`barh`, `histogram`,
      `heatmap`, `boxplot`) has the `text` method of Decisions, added once in
      the `titles` arm of `plot_setters!`.
- [ ] A doctest in the macro's doc comment builds a scatter with
      `.text(cities, |t| { t.x(..).y(..).text(..).anchor(Anchor::BOTTOM).dy(-6.0); })`,
      resolves it and asserts every label's run is in `Layout::texts` with role
      `TextRole::Annotation`.
- [ ] A test in `crates/gup-core/tests/builders.rs` calls `.text(..)` on each of
      the seven builders (a one-row label is enough) and asserts that one
      annotation run is laid out for each.
- [ ] `.text` works before and after the layer setters (it is a plot-level
      setter, not a recorded layer op), tested on one builder in each order.

### AC2: Inherent `pick_row` on every builder

- [ ] Every builder has the `pick_row` method of Decisions (same macro arm).
- [ ] A test resolves a scatter and asserts
      `chart.pick_row((x, y)) == chart.plot_mut().pick_row((x, y))` at a drawn
      row and `None` off the rows; and that `Chart::pick_row(&mut chart, p)`
      (the trait method through `&mut dyn Chart`) still compiles and agrees.

### AC3: Tutorial 7 uses the builder

- [ ] The "Annotations" block in `docs/tutorials/07_chart_types.md` uses
      `.text(cities, |t| ..)` in the builder chain; the prose no longer says to
      call `plot_mut()` for labels (it may mention `plot_mut()` for other `Plot`
      methods).
- [ ] `tutorial_7_text` in `tests/tutorials.rs` repeats the new calls, and the
      `tutorials/07_text` golden is unchanged (the same labels at the same
      places): the test passes without `GUP_BLESS`.
- [ ] `cargo test --doc -p gup` passes (the tutorial is a doctest of `gup`).

## Decisions

- **`text` signature**, in `plot_setters!(titles)`:

  ```rust
  pub fn text<R: Send + Sync + 'static>(
      mut self,
      rows: impl IntoIterator<Item = R>,
      layer: impl FnOnce(&mut crate::TextLayer<R>),
  ) -> Self {
      layer(self.plot.text(rows));
      self
  }
  ```

  A closure, because builders chain by value and `Plot::text` returns
  `&mut TextLayer<R>`. Named `text` to match `Plot::text`. It does not
  materialise the chart (text layers are annotations, independent of data
  layers), so it works at any time.

- **`pick_row` signature**, same arm:

  ```rust
  pub fn pick_row(&mut self, at: impl Into<crate::geom::Point>) -> Option<crate::RowId> {
      self.materialise();
      self.plot.pick_row(at)
  }
  ```

  Inherent methods win over trait methods in method resolution, so
  `chart.pick_row((x, y))` picks this one; `dyn Chart` callers keep the trait
  method. The trait signature does not change (a breaking change for every
  `Chart` implementer, for no gain).

- **Draw order**: a text layer added this way is an annotation like any other;
  bar value labels (added at materialise) come after it in the annotation list.
  No z change.

## Escalation Triggers

- A builder's `materialise` cannot be called from the macro (a builder lacks
  one, or it has a different signature).
- The `tutorials/07_text` golden changes.

## Out of Scope

- Forwarding other `Plot` methods (`invert`, `on_row`, `reset_view`, `zoom`) to
  builders: `plot_mut()` is the documented route (tutorial 4).
- Changing `Chart::pick_row`'s signature.

## Technical Tasks

- [ ] Add both methods to `plot_setters!(titles)`, with doc comments and the
      doctest.
- [ ] Tests in `crates/gup-core/tests/builders.rs`.
- [ ] Tutorial 7 and its mirror in `tests/tutorials.rs`.

## Dependencies

### Prerequisite Stories

- GUP-542: Text Layer for Annotations ✅ — `Plot::text`.
- GUP-563: Input Positions from Tuples ✅ — `impl Into<Point>` on `Plot`.
- GUP-536: Tutorial Tour of Chart Types ✅ — tutorial 7.

## Testing Strategy

- **Unit tests**: the macro doctest.
- **Integration tests**: seven builders with `.text`; `pick_row` agreement.
- **Visual validation**: `tutorials/07_text` unchanged.

## Success Metrics

- [ ] Tutorial 7 builds an annotated chart in one chain.

## Risk Assessment

- **Low**: additive methods. _Mitigation_: AC2 proves the trait path still
  works.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: `tutorials/07_text` unchanged
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code change
- [ ] WASM size: `mask wasm-size` scatter gz bytes (generic methods are linked
      only if used; expect no change)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
