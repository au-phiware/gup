---
implementer: sonnet
implementer-reason:
  API, scope of use and measurement rules are fixed in Decisions; follows the
  existing tick label path
---

# GUP-474: Custom Tick Label Formats

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-18)
**Created**: 2026-10-18

## Context

GUP-435 made SI-style suffixes the default tick label ("1B" for billions;
`format_log_tick` in `crates/gup-core/src/scale/log.rs` line 226, and the
equivalents on the other scales) on every axis. The orchestrator accepted that
on condition that users can override it, and no override exists. Users need
"$1.2M", "45%" and their own date formats.

Tick labels feed three consumers: axis labels, legend ramp ticks (GUP-426,
GUP-437) and hover tooltips' default text. GUP-435 thins labels by measured
width and GUP-452 holds the plot rect during interaction from measured widths;
both must see the formatted strings.

## Read First

- `crates/gup-core/src/scale/mod.rs` lines 100–135 (`ScaleRef`, `Shared`), lines
  240–330 (`ticks`, `Ticks`) and `scale/time.rs` line 140 (`Time::ticks`).
- `crates/gup-core/src/scale/log.rs` lines 220–245 (`format_log_tick`).
- `crates/gup-core/src/plot.rs` (grep `ticks(`, `thin`, `Ticks`): where axis
  labels are measured and thinned, and where legend ticks are built.
- `crates/gup-core/tests/tick_labels.rs` (a test to copy) and
  `tests/theme_png.rs` (a golden to copy).
- `crates/gup-core/src/scene.rs` line 284 (tooltip lines) and where the hover
  tooltip text is made (grep `Z_TOOLTIP`, `tooltip`).

## User Story

> "As a developer making a chart for a report, I want to format tick labels
> myself (currency, percent, dates), so that the axis reads in my domain's
> units."

## Acceptance Criteria

### AC1: API from an external crate

- [x] `ScaleRef<S>::tick_format(&self, f: impl Fn(f64) -> String + Send + Sync + 'static)`
      exists for the position scales Linear, Pow, Log and Symlog, in the setter
      shape `ScaleRef` already uses (match it).
- [x] `ScaleRef<Time>` gets the same, and its argument is the value `Time` ticks
      use (an `f64`; read `time.rs` to state the unit in the doc comment and
      doctest).
- [x] Band and Point scales do not get it; their labels stay from keys. A
      `compile_fail` doctest or test pins this.
- [x] A doctest or `tests/` file (an external-crate consumer) sets a format and
      reads the label strings back.

### AC2: Used everywhere ticks are labelled

- [x] Axis tick labels, legend ramp ticks and, if a tooltip default text renders
      a scale value, the tooltip text use the format. If the tooltip does not
      format scale values, the retrospective says so.
- [x] With no format set, output is byte-identical to today (the existing
      goldens still pass unchanged).

### AC3: Measurement uses the formatted strings

- [x] A test sets a very wide format (e.g.
      `|v| format!("{v:.0} million dollars")`) and asserts GUP-435 thinning
      drops ticks it would keep with the default labels.
- [x] A test asserts the GUP-452 held plot rect is computed from the widest
      formatted label.

### AC4: Goldens read by eye

- [x] Three goldens: a currency axis ("$1.2M"), a percent axis ("45%") and a
      date format (Time scale). The implementer reads each PNG and confirms
      labels are not clipped and the plot area is not overlapped.

### AC5: Callers and size

- [x] Search for every place that formats tick values by hand (tests, examples,
      browser scenes, doctests; grep `format_log_tick`, `{}k`, `SI`) and update
      or note why left.
- [x] Run the browser smoke test (layout changes with label width) and report
      pass or fail.
- [x] WASM size (`gup-core/wasm-size/scatter`) is reported before and after in
      the retrospective.

## Decisions

- (Orchestrator, 2026-10-18) `Time`'s `tick_format` receives f64 **Unix
  seconds**, the unit of its domain since GUP-418's hi/lo column. Tooltips are
  not affected: they are user closures (`Selection::tooltip`).

- The formatter is `Arc<dyn Fn(f64) -> String + Send + Sync>` stored in
  `Shared<S>` next to the scale (so clones of a `ScaleRef` share it), wrapped in
  a small type with a manual `Debug` so the derives keep working.
- Setting a format bumps the same version counter `ScaleRef::write` bumps, so
  caches rebuild.
- The format applies to every tick the scale produces, major and minor alike;
  `Time` major/minor weight flags stay as they are.
- Returned strings are used as is (no typographic rewriting by the library).
- No built-in helper formatters in this story; users pass closures.

## Escalation Triggers

- Tick labels are generated in a place that has no access to the `ScaleRef`
  (only to the bare scale), so the formatter has no route.
- Legend or tooltip text is produced by a path shared with non-position scales
  (colour, size), so the API shape needs to widen.
- Byte-identical default output (AC2) fails.

## Out of Scope

- Locale-aware formatting and built-in currency/percent helpers.
- Formats on colour, size or opacity scales.
- Band/Point label overrides.

## Technical Tasks

- [x] Add the formatter slot to `Shared<S>` and the `tick_format` setters.
- [x] Route axis, legend and tooltip label generation through it.
- [x] Make measurement and thinning read the formatted strings.
- [x] Add tests and the three goldens; update hand-formatted callers.

## Dependencies

### Prerequisite Stories

- GUP-435 ✅ — SI-style default labels and thinning.
- GUP-452 ✅ — held plot rect.
- GUP-426 ✅ — legends (ramp ticks).

## Testing Strategy

- **Unit tests**: formatter storage, sharing across clones, version bump.
- **Integration tests**: thinning and held-rect tests (AC3).
- **Visual validation**: the three goldens (AC4).
- **Performance**: WASM size delta.

## Success Metrics

- [x] Existing goldens unchanged.
- [x] Three new goldens read and correct.

## Risk Assessment

- **Medium**: tick labels may be generated in several places (axis, legend,
  tooltip). _Mitigation_: AC5's search plus the escalation triggers.
- **Low**: `dyn Fn` adds WASM size. _Mitigation_: reported.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `ScaleRef<S: NumericTicks>::tick_format(&self, f) -> &Self` (Linear, Pow, Log,
  Symlog, Time; Band/Point excluded by the `NumericTicks` marker, pinned by a
  `compile_fail` doctest). The format is a `TickFormat` (`Arc<dyn Fn>`, manual
  `Debug`) in `Shared<S>`; setting it bumps `revision`.
- Axis labels: `DynPositionScale::ticks` for `ScaleRef` replaces the labels, so
  measurement, thinning (GUP-435), held rect (GUP-452), scene text and SVG all
  read the formatted strings with no further changes.
- Legend ramp ticks: a hidden defaulted `tick_format` on `ShaderFn` and
  `EncodeFn` (ScaleRef returns its format, `Then` forwards its first link's);
  `Ramp::with_data_ticks` uses it.
- `ScaleRef::tick_labels(count)` reads labels back (doctest and tests).
- Tests: `crates/gup-core/tests/tick_format.rs` (7 tests; thinning, held rect,
  legend, three goldens with SVG checks) and two doctests. Goldens:
  `tests/golden/gup_core/tick_format_{currency,percent,date}.png`, read by eye:
  "$1.2M", "45%", "Jan 2021" labels complete, clear of the plot.
- Evidence: gup-core tests all pass; `mask wasm-size` scatter 319,085 -> 320,071
  gz bytes (+986 B, +0.3%); `mask wasm-browser` PASS; `mask perf-budget` all
  pass (load average about 3-4); old-path LOC 28910.

## Retrospective

**Completed**: 2026-10-18

### Key Technical Learnings

#### One route for all label consumers

- **Challenge**: labels feed several consumers (measurement, thinning, held
  rect, SVG).
- **Solution**: every consumer reads `Ticks.labels` from
  `DynPositionScale::ticks`, so formatting there covers all of them.
- **Pattern**: put an override at the single producer, not at consumers.

### Architectural Decisions

#### Legend route through `EncodeFn`

- **Decision**: legend ramp ticks are in the data's units, so the chain's first
  link's format is exposed through a hidden `tick_format` on `ShaderFn` and
  `EncodeFn`. No colour-scale format exists (out of scope); only a position
  `ScaleRef` that starts a colour chain carries one.
- **Trade-off**: two hidden defaulted trait methods; escalation trigger not
  fired because ScaleRef's API shape did not widen.

#### Tooltips and callers

- Tooltips are user closures and do not format scale values; unaffected.
- AC5 search: no hand-formatted tick callers in gup-core, examples or tests; the
  only SI formatter is the frozen root crate's `src/label`, left as is.

### Follow-up Stories

None.
