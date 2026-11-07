---
implementer: haiku
implementer-reason:
  one closure in one example and its tests; the rule and the boundary values are
  in Decisions
touches: [examples/gallery/scatter_bubbles.rs, tests/gallery.rs]
---

# GUP-659: Currency Tick Labels Below One Thousand Drop the k Suffix

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: ✅
Complete **Created**: 2026-11-07

## Context

The gallery's `scatter_bubbles` log x axis reads "$0.5k, $1k, $2k, $5k…". A
reader expects "$500". The library has no SI or currency formatter: the labels
come from the example's own `x_tick_format` closure
(`examples/gallery/scatter_bubbles.rs`, line 47), which divides every tick by
1,000 and appends `k`. The scale applies it through `TickFormat`
(`scale/mod.rs`) with no shared precision rule: a user format replaces the
scale's own. So the fix is in the example. The same chart is the `gallery/hero`
golden (`tests/gallery.rs`, line 154), so two goldens change.

## Read First

- `examples/gallery/scatter_bubbles.rs` (whole; 55 lines).
- `tests/gallery.rs` lines 55–70 and 150–156 (the two goldens that use it).

## User Story

> "As a reader of the gallery, I want a $500 tick to say "$500", so that the
> axis reads the way I would write it."

## Acceptance Criteria

### AC1: The closure

- [x] The closure becomes a named private function
      `fn dollars(v: f64) -> String` in the example: below 1,000 in absolute
      value, `format!("${v}")`; at or above, `format!("${}k", v / 1000.0)`.
- [x] Unit tests in the example file (`#[cfg(test)]`, or in `tests/gallery.rs`
      if the example is only included with `#[path]` there; the worker uses
      whichever compiles) assert: `dollars(0.5)` is `"$0.5"`, `dollars(500.0)`
      is `"$500"`, `dollars(999.0)` is `"$999"`, `dollars(1000.0)` is `"$1k"`,
      `dollars(2500.0)` is `"$2.5k"`, `dollars(1e6)` is `"$1000k"`.

### AC2: Seen

- [x] `gallery/scatter_bubbles` and `gallery/hero` goldens are re-blessed and
      read by eye: the axis labels below 1,000 have no `k`, those at or above
      keep it, none is clipped or overlaps its neighbour. Record the label
      strings seen in the retrospective.

## Decisions

- **Fix in the example, not the library.** There is no library SI formatter, and
  adding one is a public API change (out of scope).
- **`$1000k` for a million**: the example's axis tops out well below it; the
  test records the rule's behaviour, not a design goal.
- **CHANGELOG**: no line. The example's output is not API and the CHANGELOG
  lists library behaviour; the retrospective notes the decision. (If the
  orchestrator wants one, a "Changed" line is enough.)

## Escalation Triggers

- A shared precision rule turns up in the axis code that overrides the user's
  format (the labels do not change after the edit).
- A label now collides with its neighbour in the golden.

## Out of Scope

- A library SI or currency formatter (`TickFormat` helpers): a separate story if
  wanted.
- Other gallery charts' formats.

## Technical Tasks

- [x] Add `dollars` and use it in `chart()`.
- [x] Tests.
- [x] Re-bless the two goldens; look at them.

## Dependencies

### Prerequisite Stories

- GUP-594: Flagship Gallery Part One ✅ — the chart and its golden.

## Testing Strategy

- **Unit tests**: AC1's six values.
- **Visual validation**: the two goldens.

## Success Metrics

- [x] The axis shows "$500" where it showed "$0.5k".

## Risk Assessment

- **Low**: a wider label ("$500" against "$1k" is longer) may crowd a tick.
  _Mitigation_: read the golden; escalate if labels touch.

## Definition of Done

- [x] Done by the orchestrator in one commit (no checkpoint needed)
- [x] All Acceptance Criteria are satisfied and checked
- [x] Tests: `cargo test -p gup --test gallery` (12 passed); the pre-commit
      hook's checks
- [x] Lint and format: the pre-commit hook
- [x] The example compiles (the gallery test includes it)
- [x] Rendered output verified by eye (both goldens)
- [x] `mask wasm-browser`: not needed, no library, layout or wasm code changes
- [x] WASM size: unchanged: no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`examples/gallery/scatter_bubbles.rs` gains `dollars(v)` and its unit test (run
by `tests/gallery.rs`, which includes the example). Both goldens were re-blessed
and read: the x axis reads `$500, $1k, $2k, $5k, $10k, $20k, $50k, $100k`;
nothing clipped or overlapping.

## Retrospective

**Completed**: 2026-11-07, by the orchestrator (a one-function example change).

- No CHANGELOG line: the example's output is not library behaviour.
- The hero image also shows GUP-658's problem (two blues, two oranges among five
  regions); that story re-blesses it again.

### Follow-up Stories

None.
