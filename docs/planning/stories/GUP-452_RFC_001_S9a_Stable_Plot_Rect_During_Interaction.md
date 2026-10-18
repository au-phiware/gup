---
implementer: sonnet
implementer-reason:
  hold rule, release points and Plot::invert are settled in Decisions; the
  change is local to Plot::resolve_full's layout closure and Plot::handle
---

# GUP-452: RFC-001 Step S9a: Stable Plot Rect During Interaction

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-18

## Context

RFC-001 S9 adds zoom, pan and brush behaviours (§9 "Events", §11 S9). They all
rest on one promise: the value under the pointer stays under the pointer. Today
that promise breaks whenever the tick labels change width. `Plot::resolve_full`
(`plot.rs`, the `layout` closure) places the plot rect's left edge at
`pad + widest y label + label_gap + tick_len`, and its right edge
`max(pad, last x label / 2 + 2) + legend width` from the canvas edge. When a
zoom turns the y labels from `8` into `7.5`, the rect moves by a pixel or more,
and so does every mapping.

GUP-444 met this in its wheel tests and worked around it with data whose labels
keep their width (its retrospective, "The plot rect follows tick-label width").
It recommended a stable gutter for S9. Pan (GUP-453) would show the jump on
every drag that changes the labels.

This story **holds the plot rect's horizontal margins while the pointer is
interacting**. They grow only when a label would not fit even using the outer
`pad`, and they re-flow when the pointer leaves or the chart is resized. It also
adds `Plot::invert`, which pan, brush and the tests need. The top and bottom
margins come from font metrics, not label text, so they do not move and are not
held.

## Read First

- [GUP-444](GUP-444_RFC_001_S8a_Cached_Resolve_And_Input.md) Decisions "Range
  policy" and retrospective "The plot rect follows tick-label width".
- `crates/gup-core/src/plot.rs` lines 140–200 (`Plot` fields), 415–450 (`zoom`),
  545–625 (`handle`, `set_hover`) and 820–960 (`resolve_full` steps 3–5: the
  `layout` closure, `set_ranges`, widening, `thin`).
- `crates/gup-core/src/theme.rs` lines 30–45 (`pad`, `label_gap`, `tick_len`).
- `crates/gup-core/tests/resolve_cache.rs` (the wheel tests) and
  `crates/gup-core/wasm-size/scatter/index.html` lines 137–245 (the interactive
  steps).

## User Story

> "As a person exploring a chart, I want the point under my mouse to stay put
> while I zoom, so that the chart does not wobble as its tick labels change."

## Acceptance Criteria

### AC1: `Plot::invert`

- [ ] `Plot::invert` exists as in Decisions. A doctest resolves a plot, takes
      the plot rect's centre, and checks that `invert` gives the domain
      midpoints (relative error ≤ 1e-9) for a linear x and y. It returns `None`
      before the first resolve and 1 px outside the plot rect.
- [ ] `Chart` is unchanged (not added to the trait).

### AC2: The value under the pointer holds across label-width changes

In `tests/resolve_cache.rs`, a plot with explicit domains, x `(0, 100)` and y
`(0, 8)` (or other data the implementer finds and records, as long as the
conditions below hold), resolved at 400 × 300. With explicit domains, "a fresh
plot at the same domains" below has the same range policy.

- [ ] A sequence of 6 `Input::Wheel { lines: 1.0 }` steps at a fixed anchor
      inside the plot rect, then 6 with `lines: -1.0`, with a resolve after
      each. After every step, `(plot.invert(anchor) − v) × px_per_unit` is at
      most 0.01 px on each axis, where `v` is `plot.invert(anchor)` before the
      step.
- [ ] The test asserts that its data exercises the bug: at least one step widens
      and at least one narrows the widest y tick label (measured from
      `layout.texts`, the y tick labels' ink widths), and no step widens it by
      more than `theme.pad`.
- [ ] `layout.plot` is identical (bitwise `f32`) after every step of the
      sequence.
- [ ] Control: the same steps on a plot whose hold is released after every step
      (send `Input::PointerLeft` before each resolve) move `layout.plot.x` at
      least once. That proves the hold is doing the work.

### AC3: Grow and release

- [ ] **Grow**: a wheel step whose widest y label widens by more than
      `theme.pad` over the held gutter moves `layout.plot.x` right by exactly
      the growth needed. Every y tick label's ink box then has `x ≥ 0`.
- [ ] **Release on leave**: after AC2's wheel-in steps, `Input::PointerLeft`
      returns `Response { redraw: true }`, and the next resolve gives the same
      `layout.plot` as a fresh plot resolved at the same domains (bitwise).
- [ ] **Release on resize**: resolving at a different size gives the same
      `layout.plot` as a fresh plot at that size and those domains.
- [ ] Calling `Plot::zoom` directly (not through `handle`) does not hold:
      `layout.plot` equals a fresh plot's at the zoomed domains.

### AC4: Every existing caller still passes

- [ ] `git grep -n "Input::Wheel\|Input::PointerLeft" -- crates examples dogfood`.
      Each hit is listed in the retrospective as "unaffected" or "updated", with
      the reason. In particular `tests/resolve_cache.rs::inputs_that_do_nothing`
      still gets `Response::default()` for `PointerLeft` (no hold was taken).
- [ ] `cargo test -p gup-core -- --test-threads=1` and all goldens pass without
      `GUP_BLESS` (no golden involves input).

### AC5: Browser

- [ ] The harness exports a hook `InteractiveScatter::plot_rect() -> Vec<f32>`
      (`[x, y, width, height]` of the current layout's plot rect).
- [ ] In `index.html`, a step `held-rect` after the existing `wheel` step:
      `plot_rect()` is unchanged from just before the wheel. That scene's y
      labels go from `0 … 10` to a set without `10`, so without the hold the
      rect would move left.
- [ ] Then `pointerleave` and two frames: step `release`, `plot_rect()[0]` is
      smaller than before the wheel.
- [ ] `mask wasm-browser` passes (this story changes layout behaviour), with
      `interactive=ok`.

### AC6: Record

- [ ] RFC-001 gains "S9a findings" (date, story), with the rule, the test data,
      and how often the grow case was seen.
- [ ] `mask wasm-size` passes. If `wasm.scatter.gz_bytes` grows beyond its +2%,
      re-record it with the reason in `PERF_BUDGETS.md`'s History.

## Decisions

- **API**

  ```rust
  impl Plot {
      /// The data values drawn at `at` (logical pixels) as of the current
      /// scales: `None` before the first resolve or outside the plot rect
      /// (edges inclusive). Band and point axes give a code.
      pub fn invert(&self, at: Point) -> Option<(f64, f64)>;
  }
  ```

  It reads `self.x.invert`/`self.y.invert` and the cached layout's plot rect. It
  is not added to `Chart`: hosts do not need it, and `Chart` stays small.

- **The hold.** A private field `hold: Option<Hold>` on `Plot`,
  `struct Hold { width: u32, height: u32, left: f32, right: f32 }`: the size as
  `f32::to_bits` (as `CacheKey` stores it), the plot rect's left edge, and its
  right margin (`width − plot.right()`).
- **Taken** by a private `Plot::hold_layout(&mut self)`, called from `handle`
  whenever an input changes a domain (today only `Input::Wheel`; GUP-453 and
  GUP-454 call it for pan and drag-zoom). It does nothing without a cached
  layout or with a hold already taken. Direct calls to `Plot::zoom` (and later
  `pan`) do not hold: a program that sets the view is not a pointer.
- **Used** inside `resolve_full`'s `layout` closure, when the hold's size equals
  this resolve's. `needed_left` and `needed_right` are computed as today. Then
  `left = held.left` if `needed_left − pad ≤ held.left`, else `needed_left`
  (grow), and the same for the right margin. After step 5, the hold takes the
  final rect's margins; it only ever grows. Widening, `set_ranges` and the
  tick-thinning relayout all use the same closure, so they all see the held
  margins.
- **Why `pad` is the allowance**: a label may extend into the outer padding, but
  never past the canvas edge. That covers a change of one or two characters at
  the default 12 px tick size and 12 px pad (`8` to `7.5`) with no motion.
- **Released** (`hold = None`, and `self.revision` bumped so the cached resolve
  is invalidated) on `Input::PointerLeft` when a hold existed, which then
  returns `redraw: true`. Also released at the start of `resolve` when the size
  differs from the hold's. GUP-453's `reset_view` will release it too.
- **No time-based release.** A debounce would need a clock in `handle` and make
  tests time-dependent.

## Escalation Triggers

- The held margins make `resolve` non-idempotent: two consecutive uncached
  resolves of an unchanged plot under a hold give different layouts.
- No data can be found for AC2 that widens a label by no more than `theme.pad`
  in a single wheel step at the default theme.
- A golden, a glue fixture or an upload count changes.
- `mask wasm-browser`'s existing steps fail because of the hold (for example,
  the hover step's tooltip position).

## Out of Scope

- Pan, view axes and `reset_view`: GUP-453.
- Brush and drag-zoom: GUP-454.
- Holding the vertical margins: they do not depend on label text.
- A user-set fixed gutter (`Theme` field): not requested.

## Technical Tasks

- [ ] `Plot::invert` and its doctest.
- [ ] `Hold`, `hold_layout`, the `layout` closure change, and release in
      `handle` and `resolve`.
- [ ] AC2/AC3 tests in `tests/resolve_cache.rs`.
- [ ] AC4 grep and retrospective list.
- [ ] Harness hook `plot_rect`, the `held-rect` and `release` steps.
- [ ] RFC-001 "S9a findings"; `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-444: RFC-001 S8a Cached Resolve and Input ✅ — `handle`, the range policy,
  `zoomed`.
- GUP-449: RFC-001 S8f Web Canvas Host ✅ — the browser scenario.
- GUP-451: WASM Size Cuts 📋 — headroom first (owner's ordering).

### Enables Stories

- GUP-453: Drag Pan, View Axes and Reset — pans without jitter and reuses
  `hold_layout` and `invert`.
- GUP-454: Brush and Drag-Zoom — data extents through `invert`.

## Testing Strategy

- **Integration**: AC2/AC3 in `tests/resolve_cache.rs`, with exact (bitwise)
  rect comparisons and 0.01 px value checks.
- **Doctest**: `Plot::invert`.
- **Browser**: `held-rect` and `release` steps in `mask wasm-browser`.
- **Regression**: full gup-core suite and goldens without blessing.

## Success Metrics

- [ ] Zero pixel motion of the plot rect across a wheel sequence whose labels
      change width.
- [ ] Layout returns to the un-held result as soon as the pointer leaves.

## Risk Assessment

- **Low**: a held gutter leaves the rect slightly narrower (or the labels
  slightly into the pad) than a fresh layout would, until the pointer leaves.
  _Mitigation_: that is the design; released on leave and resize.
- **Low**: GUP-447's closure axes bind with `axes_generation`; the hold does not
  touch it.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-browser` passes; goldens pass without `GUP_BLESS`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
