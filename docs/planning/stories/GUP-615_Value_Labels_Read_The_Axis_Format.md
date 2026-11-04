---
implementer: sonnet
implementer-reason:
  a second format slot synced at resolve, following GUP-560's shared slot; the
  unit rule and the gallery fix are settled in Decisions
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/scale/mod.rs,
    crates/gup-core/src/builder/bar.rs,
    crates/gup-core/tests/bar_value_labels.rs,
    examples/gallery/bar_grouped_labels.rs,
    tests/golden/gallery/bar_grouped_labels.png,
    docs/tutorials/07_chart_types.md,
  ]
---

# GUP-615: Value Labels Read the Axis Format, and One Unit per Chart

## Story Overview

**Initiative**: Polish after 0.1 **Status**: 📋 Planned **Created**: 2026-11-04

## Context

Two problems with bar value labels (GUP-544, GUP-560), both checked on main:

1. **A format set on the scale handle is not seen.** `value_labels()` prints
   each bar's value with the value axis's tick format, read through the plot's
   stored slot (`Plot::axis_format`, `plot.rs` line 818; read in
   `BarChart::add_value_labels`, `bar.rs` line 836). That slot holds only
   formats set through `Plot::x_tick_format`/`y_tick_format` or the builders'
   setters. A format set with `ScaleRef::tick_format` on the axis's handle
   (`scale/mod.rs` line 264) changes the axis ticks but not the labels; the
   `value_labels` doc comment admits it ("A format set only with
   `ScaleRef::tick_format` on a scale handle is not seen").
2. **The unit is printed twice.** The gallery's `bar_grouped_labels` titles its
   axis "Revenue
   ($k)" and labels each bar `$596k`
   (`value_labels_with(|v| format!("${v:.0}k"))`),
   so the unit appears on the axis title, again on every label, and not on the
   ticks. There is no stated rule for where a unit goes.

## Read First

- `crates/gup-core/src/plot.rs`: `AxisFormat` (lines 88–104), `formats`,
  `axis_format`, `set_format` and `apply_format` (lines 776–850), and the start
  of `resolve` (around line 1235).
- `crates/gup-core/src/scale/mod.rs`: `TickFormat` (line 182),
  `ScaleRef::tick_format` (line 264), `DynPositionScale` (line 385) and its
  `ticks` impl (around line 486).
- `crates/gup-core/src/builder/bar.rs`: `value_labels` doc (around line 497) and
  `add_value_labels` (line 824).
- `examples/gallery/bar_grouped_labels.rs`.

## User Story

> "As a visualization developer, I want bar labels to read exactly like my value
> axis, however I set its format, and a clear rule for where the unit goes, so
> that a chart states each unit once."

## Acceptance Criteria

### AC1: Labels follow the axis's effective format

- [ ] With `value_labels()`, a format set only through the value axis's
      `ScaleRef::tick_format` (on the handle `plot_mut().y(..)` returned, or on
      a `ScaleRef` passed to `y_scale`) labels the bars, set before or after the
      first resolve. Tested in `crates/gup-core/tests/bar_value_labels.rs` for a
      vertical and a horizontal chart.
- [ ] Precedence matches the ticks: whichever of `y_tick_format` and
      `ScaleRef::tick_format` was set last labels both the ticks and the bars
      (two tests, one per order).
- [ ] Changing the handle's format after a resolve relabels the bars on the next
      resolve (the text layers are invalidated).
- [ ] The doc comment of `value_labels` drops the "is not seen" sentence and
      states the rule: labels use the value axis's tick format, however it was
      set.

### AC2: The unit rule, documented and followed

- [ ] The `value_labels` and `value_labels_with` docs state the rule of
      Decisions in two sentences.
- [ ] `examples/gallery/bar_grouped_labels.rs` follows it: axis title "Revenue
      ($k)", ticks bare, labels `value_labels_with(|v| format!("{v:.0}"))`.
- [ ] The golden `tests/golden/gallery/bar_grouped_labels.png` is re-blessed and
      read: every label is a bare whole number (no `$`, no `k`), labels sit
      above their bars, and the axis title still reads "Revenue ($k)".
- [ ] `git grep -n "value_labels" -- examples docs/tutorials tests/tutorials.rs`
      is checked against the rule; any other chart that repeats a unit is fixed
      and its golden re-blessed (none expected: `barh_ranked` and tutorial 7
      print bare numbers).

## Decisions

- **Mechanism**: a second slot per axis, `label_formats: [AxisFormat; 3]`,
  holding the axis's _effective_ format, which bar labels read instead of
  `formats`. `formats` keeps its meaning (the plot-level format that follows the
  axis to a new scale) and is not changed, so replacing a scale does not carry a
  handle-set format to the new scale.
- **Sync point**: at the start of every `resolve` (before any annotation is laid
  out), for each axis with a numeric scale, read the scale's current format
  through a new `DynPositionScale::tick_format(&self) -> Option<TickFormat>`
  (implemented with `ShaderFn::tick_format(self)`). If it differs from the
  stored `label_formats[axis]` (compare with a new crate-private
  `TickFormat::same(&self, other) -> bool`, `Arc::ptr_eq`; `None` vs `Some`
  differ), store it and call `invalidate()` on every annotation layer, as
  `set_format` does. Band and point axes store `None`.
- **Bar labels**: `add_value_labels` reads `label_formats` (a new
  `pub(crate) fn label_format(&self, axis) -> AxisFormat`) in place of
  `axis_format`; `axis_format` is removed if nothing else reads it.
- **The unit rule** (for docs, the gallery and tutorials): _A unit appears once:
  in the axis title when the ticks are bare numbers ("Revenue ($k)"), or in the
  tick format when the title names only the quantity ("Revenue", ticks "$200k").
  Value labels follow the tick format, so `value_labels()` obeys the rule either
  way; a `value_labels_with` format should print what the ticks print._ The
  library does not enforce it (it cannot read units out of a title).

## Escalation Triggers

- Syncing at the start of `resolve` invalidates annotation layers on every
  resolve (the comparison never matches), which would make zoom frames re-lay
  out every label; check with the existing label-cost tests.
- A cached resolve path (`Plot::resolve` returning early without a full resolve)
  skips the sync, so a changed format is not seen until the next full resolve.

## Out of Scope

- Text layers other than bar value labels reading axis formats.
- Detecting units in axis titles.

## Technical Tasks

- [ ] `DynPositionScale::tick_format`, `TickFormat::same`.
- [ ] `label_formats`, the sync at resolve, the bar label read.
- [ ] Tests (AC1); docs (AC1, AC2).
- [ ] The gallery example and its golden.

## Dependencies

### Prerequisite Stories

- GUP-560: Bar Labels Follow the Plot Tick Format ✅ — the shared slot.
- GUP-544: Bar Value Labels ✅.

## Testing Strategy

- **Integration tests**: label text from `Layout::texts` (role `Annotation`) for
  each way of setting the format.
- **Visual validation**: the gallery golden.

## Success Metrics

- [ ] The `value_labels` docs carry no known exception.
- [ ] The gallery states each unit once.

## Risk Assessment

- **Low**: the sync is a pointer comparison per axis per resolve. _Mitigation_:
  the escalation trigger above.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the gallery golden)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code change
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
