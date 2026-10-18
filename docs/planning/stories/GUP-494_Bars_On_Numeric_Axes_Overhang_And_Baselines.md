# GUP-494: Bars on Numeric Axes, Overhang and Baselines in Automatic Domains

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete 2026-10-19
**Created**: 2026-10-19

## Context

GUP-479's worker drew bars with a raw `Plot` and hit two pitfalls
(`crates/gup-core/tests/secondary_y.rs` worked around both):

1. A `Rect` with a constant non-zero width (or height) on a numeric axis does
   not report half that extent as overhang, so GUP-435's widening
   (`Plot::resolve`, `widen`, `mark_overhang`) makes no room and the end bars
   are half-clipped by the plot rect.
2. A `Derived` or constant channel value is not included in the automatic domain
   fit, so a bar baseline of 0 is not in the y domain. The auto domain started
   at 20 and the 20 mm bar vanished.

## Decisions

- A `Rect` reports `width/2` of overhang on x (and `height/2` on y) when width
  or height is a constant or `Derived` in pixels, through the same overhang path
  size channels use.
- Constant and `Derived` position values (`x_end`/`y_end` baselines, constant
  x/y) take part in the automatic domain union, like data extents (GUP-424's fit
  pass).
- Bars in the bar builder (GUP-464) use a Band axis, which already makes room;
  they must be unchanged.

## Escalation Triggers

If (2) would change domains for existing scatter or line plots (no constant
positions, so it should not), stop with `BLOCKED:`.

## Acceptance Criteria

- [ ] (deferred: `secondary_y.rs` is not on this branch; GUP-479 is unmerged.
      The orchestrator drops its two workarounds after merging both.)
      `secondary_y.rs` drops its workarounds (`encode(|_| 0.0)` and the fixed x
      domain 0.5-12.5) and still renders correctly; re-bless if needed, with the
      reason.
- [x] New tests: end bars fully inside the plot rect (pixel check); a bar from a
      `Derived` 0 baseline is visible with an automatic domain.
- [x] Every other golden unchanged unless the change is the intended fix; any
      change listed with its reason.
- [x] `mask wasm-browser` passes; `mask wasm-size` reported.

## Out of Scope

Band-axis bars, scatter/line domain behaviour.

## Definition of Done

ACs met, `mask all-check` clean, evidence recorded below.

## Implementation Summary

- `Role::Width` and `Role::Height` (channel.rs, macro allow-list); `Rect`'s
  `width`/`height` carry them. `Layer::overhang` returns
  `Overhang {size, x, y}`: half a constant or `Derived` width reaches past x
  only, half a height past y only. `Plot::resolve` uses per-axis overhang for
  widening and insets; the extent is ignored on an ordinal (band) axis, which
  makes its own room.
- `Layer::fixed_positions` and `DynPositionScale::fit_position`: after the data
  fit, constant and `Derived` position pixels are inverted through the scale and
  unioned into an automatic domain (snapped to 1e-5 of the span so an `f32`
  pixel round trip leaves 0 at 0).
- Test: `numeric_axis_bars_fit_and_a_derived_baseline_is_in_the_domain`
  (rect_png.rs). All gup-core tests pass; no golden changed. `mask wasm-browser`
  passes; `mask wasm-size` all checks pass (scatter 321075 gz, +2% of baseline).

## Retrospective

**Completed**: 2026-10-19

- A first, scalar overhang changed the bar builder golden (Band widths are
  `Derived`). Overhang must be per axis and skip ordinal axes. Pattern: a
  mark-wide scalar cannot describe an axis-specific extent.
- A `Derived` position is pixels, so its data value is recovered with `invert`;
  this is exact for any scale state, but needs snapping.
- `secondary_y.rs` was not on the branch; its workaround removal is left to
  after GUP-479 merges. No follow-up stories.
