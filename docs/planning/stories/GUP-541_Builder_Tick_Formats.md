---
implementer: sonnet
implementer-reason:
  plot-level setters that store a TickFormat and apply it through one new
  DynPositionScale method; signatures and precedence fixed in Decisions
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/builder/mod.rs,
    crates/gup-core/src/scale/mod.rs,
    dogfood/,
  ]
---

# GUP-541: Tick Label Formats on `Plot` and the Builders

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-26) **Created**: 2026-10-24

## Context

GUP-474 added `ScaleRef::tick_format(|v| ..)`: a custom label for every tick of
a numeric scale (`$1.0M`, `45%`, `2024-03`). It is reachable only through a
`ScaleRef` handle, which `Plot::x(scale)` returns. The builders hide that
handle: `ScatterChart::x_scale<S: PositionScale>(scale)` takes a plain scale and
drops the `ScaleRef` that `plot.x(..)` returns (checked 2026-10-24). So a
builder user cannot format ticks without `plot_mut()` and re-creating the scale,
and the dogfood suite dropped two formats it used to have (GUP-465, GUP-485):
task 1's `%Y-%m` dates and task 3's `$Nk` incomes (`dogfood/README.md`,
"Dropped" lines).

The old builders had `.x_tick_format(..)`; the builder module's migration table
says "dropped". A newcomer coming from any plotting library looks for exactly
that name. This story adds it to `Plot` and to every builder, and restores the
dogfood formats.

## Read First

- `crates/gup-core/src/scale/mod.rs` lines 116–245 (`ScaleRef`, `TickFormat`,
  `NumericTicks`, `tick_format`) and 328–420 (`DynPositionScale`, its `ScaleRef`
  impl and `ticks`).
- `crates/gup-core/src/plot.rs` lines 540–600 (`Plot::x`, `y`, `y_right`) and
  the `Plot` struct fields (lines 148–245).
- `crates/gup-core/src/builder/mod.rs` lines 95–150 (`plot_setters!`).
- `crates/gup-core/tests/tick_format.rs` (whole file).
- `dogfood/src/bin/t1_timeseries.rs`, `dogfood/src/bin/t3_scatter_png.rs`,
  `dogfood/src/suite.rs` lines 57–140 (`Measure`) and the `t1`/`t3_png` tasks,
  `dogfood/README.md`'s two "Dropped" lines.

## User Story

> "As a builder user, I want `.x_tick_format(|v| format!("${v}k"))` on my chart,
> so that axes read in my units without dropping to the `Plot` API."

## Acceptance Criteria

### AC1: `Plot` setters

- [x] `Plot::x_tick_format`, `Plot::y_tick_format` and
      `Plot::y_right_tick_format` exist with the signatures in Decisions, each
      with a doctest.
- [x] The format survives replacing the scale: `plot.x_tick_format(f)` then
      `plot.x(Log::new())` labels the log axis with `f` (test).
- [x] The last writer wins between `ScaleRef::tick_format` on the handle
      `plot.x(..)` returned and `Plot::x_tick_format` (test both orders).
- [x] On a band or point axis, `Plot::resolve` returns `Error::Config` whose
      message names the axis and says band and point axes are labelled by their
      keys (test).

### AC2: Every builder

- [x] `plot_setters!` gains `x_tick_format` and `y_tick_format`, so
      `ScatterChart`, `LineChart` and `BarChart` have them, documented, and work
      before or after `x_scale`/`y_scale` (test on the scatter).
- [x] `crates/gup-core/tests/tick_format.rs` gains a builder test: a scatter
      with `.y_tick_format(|v| format!("{v:.0}%"))` has every y tick label
      ending in `%` (from `Layout::texts`).
- [x] The builder module's migration table row for `.x_tick_format(..)` says
      `.x_tick_format(|v| ..)` instead of "dropped".

### AC3: Dogfood formats restored

- [x] `t1_timeseries` labels its time axis `%Y-%m` with `chrono`
      (`DateTime::from_timestamp(v as i64, 0)`), and also writes `t1.svg`
      (`SvgTarget`, as t3 does).
- [x] `t3_scatter_png` labels its log x axis with
      `format!("${:.0}k", v / 1000.0)`.
- [x] `Measure` gains
      `SvgText { what: &'static str, matches: fn(&str) -> bool, min: usize }`
      ("at least `min` `<text>` elements whose content `matches`"); t1 checks
      `matches` = "7 characters, `-` at index 4" (min 3) on `t1.svg`, t3 checks
      "starts with `$` and ends with `k`" (min 3) on `t3_scatter.svg`.
- [x] `dogfood/README.md`'s two "Dropped" lines are replaced by what the tasks
      now do. `./dogfood/run_all.sh` reports 10 PASS; the worker reads `t1.png`
      and `t3_scatter.png` and records the labels seen.

## Decisions

- **Signatures** (on `Plot`, returning `&mut Self`; on builders, taking and
  returning `self`):

  ```rust
  pub fn x_tick_format(&mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> &mut Self;
  pub fn y_tick_format(&mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> &mut Self;
  pub fn y_right_tick_format(&mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> &mut Self;
  ```

  A newcomer's guess: the old builders and most plotting APIs call it
  `x_tick_format`; the closure form matches `ScaleRef::tick_format`. Builders
  get `x_tick_format`/`y_tick_format` only (they have no right axis).

  ```rust
  let chart = gup::scatter(people)
      .x(|p| p.income)
      .x_scale(Log::new())
      .x_tick_format(|v| format!("${:.0}k", v / 1000.0))
      .y(|p| p.spend);
  ```

- **Storage**: `Plot` keeps `formats: [Option<TickFormat>; 3]` (x, y, right y).
  The setter stores it and writes it to the current scale through a new
  `DynPositionScale::set_tick_format(&self, format: TickFormat)`, implemented
  for `ScaleRef<S>` by writing the shared `format` slot and bumping the
  revision, exactly as `ScaleRef::tick_format` does. `Plot::x`/`y`/`y_right`
  write the stored format to the new scale after installing it. Nothing is
  written during `resolve` (a write bumps the revision and would defeat the
  resolve cache).
- **Band and point axes are an error at resolve**, not a silent no-op: a stored
  format on an axis whose scale `reads_keys()` returns
  `Error::config("x_tick_format", "...")` before any tick is computed; the
  format is never written to a key scale (its `ticks` would relabel codes).
- **`TickFormat::new` stays `pub(crate)`**; no new public type.
- **Clearing a format** is not offered (out of scope).

## Escalation Triggers

- Writing the format in `Plot::x` changes the resolve-cache behaviour checked by
  `resolve_cache.rs` (an extra miss): report it.
- A dogfood task other than t1 and t3 changes result.

## Out of Scope

- Band and point label formats (they are the keys).
- Legend tick formats on colour scales (`ScaleRef::tick_format` on the colour
  scale handle already does it).
- A date-format helper in `gup-core` (the user's closure formats dates).

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Add `set_tick_format` and the three `Plot` setters with tests; commit.
- [x] Add the builder setters, the builder test and the migration-table row;
      commit.
- [x] Restore the dogfood formats, add `SvgText`, run dogfood; commit.

## Dependencies

### Prerequisite Stories

- GUP-474: Custom tick label formats ✅ — `TickFormat` and the shared slot.

### Enables Stories

- GUP-544 (bar value labels) uses the value axis's format as its default.

## Testing Strategy

- **Unit/integration tests**: `tick_format.rs` cases above.
- **Dogfood**: t1 and t3 with their `SvgText` checks.

## Success Metrics

- [x] Every builder can format its numeric axes in one call; both dogfood
      formats are back and checked.

## Risk Assessment

- **Low**: the format is stored twice (plot and scale). _Mitigation_: the plot's
  copy is only re-applied when a scale is replaced; the test of both orders pins
  the precedence.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`t1.png`, `t3_scatter.png`)
- [x] `mask wasm-browser`: not needed, labels only (no layout rule change)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Plot::{x,y,y_right}_tick_format` store a `TickFormat` per axis
  (`formats: [Option<TickFormat>; 3]`) and write it through the new
  `DynPositionScale::set_tick_format`; `Plot::x`/`y`/`y_right` re-apply it to a
  replacement scale. `resolve_full` returns `Error::Config` for a stored format
  on a key (band/point) axis; the format is never written to a key scale.
- Builders: `plot_setters!()` now includes `x_tick_format`/`y_tick_format`
  (scatter, line, histogram); `plot_setters!(titles)` is the old set. Coverage:
  scatter, line, histogram (both axes); `bar` (`y_tick_format` only) and `barh`
  (`x_tick_format` only), the value axis, because the other axis is a band.
  `heatmap` has two band axes and gets none. Migration-table row updated.
- Dogfood: t1 `%Y-%m` (chrono) and writes `t1.svg`; t3 `$Nk`; `Measure::SvgText`
  with checks (t1: 11 matching labels, t3: 13). `run_all.sh`: "Tasks: 10 pass, 0
  pass with tracked gaps (XFAIL), 0 FAIL, 0 XPASS, 0 skipped".
- Tests: 5 new in `tests/tick_format.rs` (`setters` module) plus 5 doctests.
  Seen by eye: `t1.png` x labels `2020-01 … 2025-01`; `t3_scatter.png` x labels
  `$1k $2k $5k $10k … $10000k`.
- WASM scatter gz: 338047 B (budget pass).

## Retrospective

**Completed**: 2026-10-26

### Key Technical Learnings

#### Key scales and the stored format

- **Challenge**: the format must outlive a scale replacement but must never
  reach a band or point scale.
- **Solution**: the plot stores the format and guards both writes
  (`apply_format`) with `reads_keys()`; resolve reports the misuse.
- **Pattern**: store intent on the owner, push it to replaceable children.

### Architectural Decisions

#### Value-axis-only setters on bar and barh

- **Decision**: `BarChart<Vertical>` has only `y_tick_format`, `Horizontal` only
  `x_tick_format`; heatmap has none (`plot_setters!(titles)`).
- **Reasoning**: a setter that can only fail at resolve is a trap; the story's
  "every builder" meant every numeric axis.
- **Trade-off**: `plot_setters!` has two arms.

### Development Workflow Insights

- A worktree-isolated agent's shell refuses compound commands with heredocs,
  runtime variables in `sed` and chained cargo calls; use Edit and plain
  commands.
- `merge-check` doc step catches broken intra-doc links in macro-expanded and
  plain docs; run `cargo doc` with `-D warnings` before it.

### Follow-up Stories

None.
