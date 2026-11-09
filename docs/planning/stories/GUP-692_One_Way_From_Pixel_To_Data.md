---
implementer: sonnet
implementer-reason:
  delete one method, migrate its callers to `value`, and edit tutorial 4; the
  shape is decided below
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/tests/pixel_value.rs,
    crates/gup-core/tests/resolve_cache.rs,
    crates/gup-core/tests/window.rs,
    crates/gup-core/tests/axis_extent.rs,
    crates/gup-core/tests/events.rs,
    crates/gup-core/tests/compile_pass/positions_from_tuples.rs,
    crates/gup-core/wasm-size/scatter/src/lib.rs,
    docs/tutorials/04_interactions.md,
  ]
---

# GUP-692: One Way From Pixel to Data

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: ✅
Complete 2026-11-09 **Created**: 2026-11-08

## Context

GUP-685 added `Chart::value(at)` and inherent `Plot::value`/`value_right`. The
older `Plot::invert(at)` does the same, except that it returns `None` outside
the plot rect. Two names for one thing confuse users, and tutorial 4 had to
explain the difference. 0.1 is unpublished, so removing API is free.

## User Story

As a user mapping a pixel to data, I want one method, so that I do not have to
choose between two.

## Decisions

- Remove `Plot::invert`. Callers use `value`.
- The trait method `Chart::value(&self, at: Point)` stays as it is (`Chart` is
  used as `dyn Chart`, so it cannot take `impl Into<Point>`). The inherent
  `Plot::value` and `Plot::value_right` take `impl Into<Point>`, as `invert`
  did, so tuples and arrays keep working on a `Plot`.
- A caller that needs "outside the plot is `None`" checks the plot rect itself
  (the only one is the wasm scatter's `value_at`).

## Acceptance Criteria

- [x] `Plot::invert` is gone; no caller remains.
- [x] `invert`'s doctest assertions (the centre is the domain midpoint) live on
      `Plot::value`.
- [x] Tutorial 4 uses `chart.value(..)` and no longer compares the two.
- [x] `mask merge-check`, `mask all-check`, the `gup` tutorial doctests and
      `gup-egui` tests pass.

## Out of Scope

- Any new rect-containment API.

## Definition of Done

- [x] As above; CHANGELOG needs no entry (`invert` was never in [0.1.0]).

## Implementation Summary

`Plot::invert` is deleted. `Plot::value` and `value_right` take
`impl Into<Point>`; `Chart::value` keeps `Point` (object safety). Tests migrated
to `value` (the outside-the-rect cases now assert extrapolation); the wasm
scatter's `value_at` checks `layout().plot` itself; tutorial 4 uses
`chart.value(..into())` and drops the comparison prose. The centre-maps-to-
midpoint doctest is on `Plot::value`.

Note: `gup-egui` tour tests `every_chapter_has_its_code_and_summary` and
`the_code_is_on_the_page` already fail on main (tour.rs still names the removed
`pixel` region of chapter 8); not touched here.

## Retrospective

**Completed**: 2026-11-09

- Tuples need `.into()` against `Chart::value(Point)`; the geometry `Point` is
  not in the prelude (it clashes with the scale `Point`), so `.into()` reads
  better than an import.
