---
implementer: sonnet
implementer-reason:
  the rule, the stats field and every hook in the fit path are fixed in
  Decisions; plumbing through existing traits with defaulted methods
touches: [crates/gup-core/src/encoding.rs]
---

# GUP-562: Integer Tick Steps for Integer Data

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧
Planned **Created**: 2026-10-26

## Context

A linear axis over whole-number data gets fractional ticks when its domain is
short. Tutorial 6's "Week" axis (`tests/golden/tutorials/06_squares.png`) holds
weeks 0–3, widened to −0.5..3.5 for the squares' overhang, and is labelled −0.5,
0, 0.5 … 3.5. Counts, years as numbers, weekdays and ranks show the same thing:
"1.5 items" is a tick no reader wants.

The axis cannot tell today: `ColumnStats` carries `min`, `max` and `non_finite`
(`column.rs` lines 320–380), and a scale is fitted from the extent alone
(`ShaderFn::fit_domain`). The domain's ends are no guide: data from 0 to 1 may
be probabilities.

This story records, per column, whether every finite value is a whole number,
passes that to the scale with the extent, and gives a `Linear` scale fed only by
whole numbers a tick step of at least 1, automatically. Matplotlib offers this
as an opt-in (`MaxNLocator(integer=True)`); Gup's columns already see every
value, so it costs one comparison per value and no API.

## Read First

- `crates/gup-core/src/column.rs`: `ColumnStats` and `Acc` (lines 320–380), the
  store test that builds a `ColumnStats` literal (around line 1451).
- `crates/gup-core/src/encoding.rs`: `ShaderFn::begin_fit` (line 85),
  `EncodeFn::fit` (lines 196–211), the blanket
  `impl<S: ShaderFn> EncodeFn for S` (line 440), the chain's `fit` (lines
  562–567), the column encoding's `fit(stats)` (lines 1133–1137).
- `crates/gup-core/src/scale/mod.rs`: `Shared` (line 156), `ScaleRef`'s
  `begin_fit` and `fit_domain` (lines 303–330).
- `crates/gup-core/src/scale/linear.rs`: `Linear` fields, `ticks`, and the
  `step` helper GUP-561 adds.

## User Story

> "As a developer plotting counts, weeks or ranks, I want whole-number ticks
> without configuring anything, so that my axis never shows 0.5 of a week."

## Acceptance Criteria

### AC1: Columns know whether they are whole numbers

- [ ] `ColumnStats` has `pub integral: bool`: true when every finite value of
      the column (or chunk) is a whole number, `f64::fract() == 0.0`; merged
      across chunks and appends with `&&`; true for a column with no finite
      value. Unit tests in `column.rs`: `[0, 1, 2]` true, `[0, 0.5]` false,
      `[3, NaN]` true, appends of `[1]` then `[1.5]` false.
- [ ] Dictionary-code columns report `integral: true` (codes are integers).

### AC2: The rule

- [ ] Unit tests in `scale/linear.rs`, with the integral flag set: domain (−0.5,
      3.5) at count 8 ticks 0, 1, 2, 3; (0, 100) at count 8 is unchanged (step
      20); (1.2, 2.8) keeps its default fractional ticks (span < 2); with the
      flag clear, (−0.5, 3.5) keeps 0.5 steps.
- [ ] A `tick_unit` wins over the flag.

### AC3: The flag reaches the scale

- [ ] Integration test (`tests/tick_labels.rs`): a `Plot` with circles at y in
      {0, 1, 2, 3} on an automatic `Linear` y at 640×400 has y tick labels with
      no `.`; adding a second layer on the same scale with y = 1.5 brings the
      fractional ticks back (the flag is the AND over every layer fitted in the
      pass), and removing the data again (a new plot) restores integers.
- [ ] A chain (`Linear` then another function) passes the flag to its first link
      only.

### AC4: Tutorial 6 and the other goldens

- [ ] `tests/golden/tutorials/06_squares.png` re-blessed and read: the "Week"
      axis is labelled 0, 1, 2, 3 and nothing else.
- [ ] Any other golden that changes is read and listed with one line each in the
      Implementation Summary; each must change only by losing fractional ticks
      on an axis whose data are whole numbers. Anything else is an escalation.

### AC5: Cost

- [ ] `Acc::push` gains one comparison. Run `mask wasm-size` and report the
      scatter gz bytes. Time the first `resolve` of a 1,000,000-row
      `gup_core::scatter` (x and y from `f64` fields) in release, median of 5,
      with a throwaway example that is not committed, at the parent commit and
      after; report both. A slowdown above 3% is an escalation.

## Decisions

- **Stats**: `ColumnStats { min, max, non_finite, integral }`; `Acc` gains
  `integral: bool` (starts `true`; `push` of a finite `v` with
  `v.fract() != 0.0` clears it; `merge` ANDs). Adding a public field is fine
  before 0.1; update the one literal in `column.rs`'s tests.
- **Hooks**, all defaulted so no user impl breaks:

  ```rust
  pub trait ShaderFn {
      /// Whether every value fitted in this pass is a whole number; called
      /// after `fit_domain` with the column's `ColumnStats::integral`.
      fn fit_integral(&mut self, integral: bool) {}
  }
  pub trait EncodeFn {
      /// Pass `integral` to the first link (the one that reads the column).
      fn fit_integral(&mut self, integral: bool) {}
  }
  ```

  The blanket `impl<S: ShaderFn> EncodeFn for S` forwards to
  `ShaderFn::fit_integral`; the chain forwards to `self.first` only. The column
  encoding's `fit(stats)` calls `self.func.fit_integral(stats.integral)` after
  `self.func.fit(..)`. Dictionary fitting (`fit_dictionary`) does not call it.

- **`ScaleRef`** ANDs the flag across the pass: `Shared` gains
  `integral: Mutex<Option<bool>>`, cleared to `None` by `begin_fit`;
  `fit_integral(v)` stores `Some(old.unwrap_or(true) && v)` and writes the
  result to the scale with `self.write().fit_integral(..)`.
- **`Linear`** gains `integral: bool` (default false), set by its
  `fit_integral`. In the private `step(lo, hi, count)` helper (GUP-561), the
  unit is `self.tick_unit.or((self.integral && hi - lo >= 2.0).then_some(1.0))`.
  So `ticks` and `nice_to` agree. The `hi - lo >= 2.0` guard keeps at least two
  ticks: a zoom into (1.2, 1.8) of whole-number data keeps fractional ticks
  instead of showing none. A unit of 1 through `nearest_125` only raises a step
  below 1; steps of 1 or more are already whole.
- **Not affected**: `fit_position` (constant and derived positions such as a bar
  baseline) leaves the flag alone, since integer ticks are never wrong, only
  sparser. `Pow`, `Log`, `Symlog`, `Time`, `Band`, `Point` ignore the flag (the
  default no-op). Text layers do not fit domains.

## Escalation Triggers

- GUP-561 has not merged (`Linear::step` missing): stop; this story builds on
  it.
- A golden changes in a way other than losing fractional ticks on whole-number
  data.
- Adding `fit_integral` to `ShaderFn` changes a trybuild `.stderr` snapshot or
  the `#[wgsl_function]` expansion needs an edit.
- AC5's slowdown is above 3%.

## Out of Scope

- An explicit API (`Linear::integer_ticks()`): not needed while the automatic
  rule holds; a later story if a user asks for integer ticks on fractional data.
- Integer steps on `Pow`, `Symlog` or `Time` scales.

## Technical Tasks

- [ ] `ColumnStats::integral` and `Acc` (AC1).
- [ ] `ShaderFn::fit_integral`, `EncodeFn::fit_integral`, the blanket and chain
      forwards, the column encoding's call.
- [ ] `ScaleRef` accumulator; `Linear::integral` and the unit in `step` (AC2).
- [ ] AC3 integration tests; re-bless and read tutorial 6 (AC4); AC5 numbers.

## Dependencies

### Prerequisite Stories

- GUP-561: Axis Ends Fit Bars and Bands 📋 — the shared `Linear::step` helper
  that `ticks` and `nice_to` use.

### Enables Stories

None directly.

## Testing Strategy

- **Unit tests**: `column.rs` stats; `linear.rs` rule.
- **Integration tests**: AC3 through `Plot` and `Layout`.
- **Visual validation**: tutorial 6's golden, read.
- **Performance**: AC5.

## Success Metrics

- [ ] Tutorial 6's Week axis reads 0, 1, 2, 3.
- [ ] No public signature changes except the new defaulted methods and field.

## Risk Assessment

- **Low**: a dataset of whole numbers with a span just over 2 gets 2–3 ticks
  where it had 5. _Mitigation_: that is what such data supports; the guard keeps
  at least two.
- **Low**: user `ShaderFn` impls that wrap a `Linear` do not forward the flag.
  _Mitigation_: they keep today's behaviour.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (tutorial 6's golden, AC4)
- [ ] `mask wasm-browser` (tick placement can change on its scenes)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
