---
implementer: sonnet
implementer-reason:
  a two-line range change in Plot::resolve with the rule fixed in Decisions; the
  work is re-blessing and reading the goldens it moves
touches: [crates/gup-core/src/plot.rs, crates/gup-core/wasm-size/]
---

# GUP-540: Marks on an Explicit Domain's Near Edge Stay Inside the Plot

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

## Context

With an automatic domain, `Plot::resolve` widens the domain in data units so a
mark's overhang (a circle's radius, half a constant bar width) fits at both ends
(GUP-435, GUP-494). With an explicit domain, nothing is widened: the far end of
each range (right, top) is inset by the overhang plus `Theme::plot_padding`, and
the near end (left, bottom) is not inset at all, "so the axes meet at the corner
and a baseline sits on its axis" (`plot.rs`, `set_ranges`, step 5 of
`resolve_full`).

So a point at the domain's lower-left corner is cut to a quarter disc. The egui
example shows it: its scatter has `x_scale(Linear::new().domain(0.0, 10.0))` and
a point at `(0, 0)` with radius 5, clipped at the plot corner. The same happens
to the first bar of a numeric-axis bar chart with a constant width and an
explicit x domain (the end-bar padding the review's triage lists under N2 bars).
Explicit domains are common: a newcomer who wants an axis to start at 0 writes
`.domain(0.0, 10.0)` and gets clipped marks.

The baseline concern is real but narrower than the rule: a bar's baseline sits
on the axis because a `Rect` with data-driven edges has no overhang on that
axis. Insetting the near end by the overhang alone (no padding) leaves bar
charts unchanged and gives circles room.

## Read First

- `crates/gup-core/src/plot.rs` lines 1806–1870 (overhang per axis, `inset`,
  `set_ranges`, `widen`, `kept`) and 1974–2000 (axis line extents).
- `crates/gup-core/src/selection.rs` lines 1170–1215 (`Layer::overhang`).
- `crates/gup-core/src/scale/mod.rs` lines 465–483 (`widen`).
- GUP-494's Implementation Summary (overhang is per axis and ignored on band
  axes).
- `gup-egui/examples/egui_chart.rs` lines 14–25 (the clipped case).

## User Story

> "As a visualization developer, I want a point at the edge of the domain I set
> to be drawn whole, so that setting `.domain(0.0, 10.0)` does not cut off my
> data."

## Acceptance Criteria

### AC1: The near end is inset by the overhang

- [ ] For an axis that is not widened (an explicit domain, and not a zoomed axis
      that kept the widened policy), the near end of its range is inset by that
      axis's overhang (`over[i]`, without `plot_padding`): x maps from
      `plot.left() + over[0]`, y and the right y from `plot.bottom() - over[i]`.
      The far end is unchanged (`over[i] + plot_padding`). Widened axes are
      unchanged (range = the plot rect).
- [ ] New test in `crates/gup-core/tests/scatter_png.rs`: a scatter with `x` and
      `y` domains `0..10`, radius 5, points at `(0, 0)`, `(10, 10)` and
      `(0, 10)`: every disc's full circle lies inside `Layout::plot` (checked by
      sampling the disc's colour 4 px inside its edge in the four directions),
      and `Layout::x_ticks[0]` is at `plot.left() + 5.0` (±0.5).
- [ ] New test in `crates/gup-core/tests/rect_png.rs`: bars on a numeric x axis
      with constant `Rect::WIDTH` 20 px and an explicit x domain whose minimum
      equals the first bar's x: the first bar's left edge is inside the plot
      rect (pixel check), and the bars' bottoms still sit on the x axis (y has
      no overhang): the pixel row just above the axis line is bar colour across
      the bar.

### AC2: Bar charts and lines are unchanged

- [ ] `gup_core/builder_bars`, `gup_core/grouped_bars`, `gup_core/rect_bars`,
      `gup_core/line_series` and `gup_core/builder_line` goldens pass without
      re-blessing.

### AC3: Every moved golden is listed and read

- [ ] Every golden that changes is re-blessed (`GUP_BLESS=1`) and listed in the
      Implementation Summary with its reason (circles on an explicit domain are
      expected, e.g. `msaa_1x`, `msaa_4x`); the worker reads each re-blessed PNG
      and confirms the edge marks are whole and the axes still meet at the
      corner.
- [ ] Tests that compute pixels from a domain by hand (not from `Layout` ticks
      or a `ScaleRef::eval` after resolve) and break are fixed to read `Layout`;
      each is listed.

### AC4: Every host agrees

- [ ] `mask wasm-browser` passes. Scene code in `crates/gup-core/wasm-size/`
      that computes expected pixels by hand is updated, listed.
- [ ] `mask egui-screenshot` passes, and the worker reads its PNG: the egui
      scatter's `(0, 0)` point is a whole disc.
- [ ] `./dogfood/run_all.sh` reports 10 PASS.

## Decisions

- **Inset by the overhang only, never the padding, at the near end**: a bar's
  baseline, a line's start and an area's base have no overhang on that axis and
  stay on the axis line; a circle moves in by its radius. Padding stays a
  far-end-only allowance, as today.
- **Axis lines keep their extents**: the x axis line still starts at the corner
  (`ay_x`) and ends at `plot.right() - inset[0]` for an explicit domain; the
  first tick moves in with the range. The axes therefore still meet at the
  corner.
- **No new option.** `Theme::plot_padding` remains the only knob; turning the
  inset off is not offered (a clipped mark is never what a user wants).
- **Ordinal axes are untouched**: overhang on a band or point axis is already 0
  through `over()` (`reads_keys`), and bands make their own room.

## Escalation Triggers

- More than 15 goldens change: stop and list them before re-blessing; the
  orchestrator decides whether to proceed.
- Any bar-chart golden (AC2) changes: stop; the rule is wrong somewhere.
- A zoom test (`zoom_uploads.rs`, `resolve_cache.rs`, `events.rs`) fails in a
  way that is not a pixel position (for example a non-zero column write count):
  stop and report.

## Out of Scope

- Changing automatic-domain widening (GUP-435).
- Not clipping marks to the plot rect (marks stay clipped).
- Builder tick formats (GUP-541).

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] Change `set_ranges` (a `near(i)` beside `far(i)`); run gup-core tests and
      list failures; commit.
- [ ] Add the two tests; re-bless and read goldens; fix hand-computed tests.
- [ ] Run `mask wasm-browser`, `mask egui-screenshot` and dogfood; commit.

## Dependencies

### Prerequisite Stories

- GUP-494: Bars on numeric axes, overhang and baselines ✅ — per-axis overhang.

### Enables Stories

- GUP-548 (histogram) and GUP-547 (area): their explicit-domain ends rely on
  this rule.

## Testing Strategy

- **Integration tests**: the two new pixel tests; every gup-core test.
- **Visual validation**: every re-blessed golden read by eye; the egui
  screenshot.
- **Hosts**: `mask wasm-browser`, dogfood.

## Success Metrics

- [ ] No mark whose position lies in an explicit domain is clipped by the plot
      rect at either end of the axis.

## Risk Assessment

- **Medium**: many goldens move by a few pixels, which can hide an unrelated
  change. _Mitigation_: re-bless only after the code change, in a commit of its
  own, and list every file with its reason.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (re-blessed goldens, egui screenshot)
- [ ] `mask wasm-browser` passes (layout change)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
