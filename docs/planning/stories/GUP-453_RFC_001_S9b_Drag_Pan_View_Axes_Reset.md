---
implementer: sonnet
implementer-reason:
  API, drag state machine, pan formula, reset semantics and host changes are
  settled in Decisions; builds on Plot::handle, zoom and GUP-452's hold
---

# GUP-453: RFC-001 Step S9b: Drag Pan, View Axes and Reset

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-18)
**Created**: 2026-10-18

## Context

RFC-001 §9 lists the built-in behaviours (tooltip, wheel zoom, drag pan, brush)
and says that they mutate only scale domains, so they cost uniform writes only.
S8 delivered the tooltip and wheel zoom through `Chart::handle` (GUP-444,
GUP-446), in a window (GUP-448) and in a browser canvas (GUP-449). This story
adds drag pan, a choice of which axes the view moves, and a way back to the
starting view. Brush and drag-zoom follow in GUP-454.

Today a primary-button press followed by a move does nothing; `handle` only uses
the press for clicks (`CLICK_SLOP` = 4 px). Wheel zoom always moves both axes.
It also stops at the first error: on a `Band` x axis, `zoom` returns the x error
before zooming y, so a bar chart cannot be zoomed at all. There is no way back
to the fitted domains once a zoom has made them explicit.

GUP-444 fixed the range policy (an axis widened by resolve keeps its widened
range after a zoom, through `Plot::zoomed`). It noted that pan should record its
domain the same way. GUP-452 holds the plot rect during interaction and adds
`Plot::invert`. This story builds on both.

## Read First

- RFC-001 §9 "Events" (lines 655–666) and "S0b findings → What changed" item 3
  (lines 1303–1308, zoom in pixel space through `invert`).
- [GUP-444](GUP-444_RFC_001_S8a_Cached_Resolve_And_Input.md) Decisions "Range
  policy"; [GUP-452](GUP-452_RFC_001_S9a_Stable_Plot_Rect_During_Interaction.md)
  Decisions.
- `crates/gup-core/src/plot.rs` lines 140–200, 415–450 (`zoom`), 545–650
  (`handle`, `set_hover`, `dispatch`) and 870–885 (the `kept` policy check).
- `crates/gup-core/src/scale/mod.rs` lines 85–110 (`PositionScale`) and 235–360
  (`DynPositionScale`, its `zoom`, `ScaleRef`'s impl); and `scale/band.rs` lines
  330–360 (`Band`/`Point` refuse `set_domain`).
- `crates/gup-core/src/web.rs` lines 395–470 (listeners) and `Cargo.toml`'s
  `web-sys` features.
- `crates/gup-core/wasm-size/scatter/src/lib.rs` lines 760–923 and `index.html`
  lines 137–245.

## User Story

> "As a person exploring a chart, I want to drag to move around after zooming,
> and get back to where I started, so that I can inspect any part of the data
> with the mouse alone."

And for the chart's author:

> "As a chart author, I want to choose which axes the mouse may move, so that a
> time series scrolls only in time."

## Acceptance Criteria

### AC1: API

- [x] `Axes`, `Drag`, `Plot::view_axes`, `Plot::drag`, `Plot::pan` and
      `Plot::reset_view` exist as in Decisions. `Axes` and `Drag` are in the
      prelude. A doctest builds a plot with `.view_axes(Axes::X)`, resolves it,
      sends a press, two moves and a release through `handle`, and checks that
      only the x domain moved.
- [x] `PositionScale::clear_domain` is implemented for `Linear`, `Log`, `Pow`,
      `Symlog`, `Time`, `Band` and `Point`. A unit test per scale: after
      `set_domain` (where allowed) and `clear_domain`, `is_auto()` is true.

### AC2: Pan through `handle`

In `tests/resolve_cache.rs`, with a plot of explicit domains and one with
automatic (widened) domains, 400 × 300, a layer with a tooltip and `Enter`,
`Leave` and `Click` handlers:

- [x] Press at `P` over a row; move to `P + (2, 0)`: no domain changes,
      `Response::default()`, the row is still hovered.
- [x] Move to `P + (30, 20)`, then `P + (60, −10)`: each returns `redraw: true`.
      After each resolve, `(plot.invert(current) − v) × px_per_unit ≤ 0.01` px
      on both axes, where `v = plot.invert(P)` before the press. `layout.plot`
      is bitwise unchanged (GUP-452's hold).
- [x] The hovered row gets `Leave` when the drag starts, `hovered()` is `None`
      during the drag, and no `Click` is dispatched by the release. After the
      release, a `PointerMoved` hovers again.
- [x] The same sequence on a `Log` y axis meets the 0.01 px bound.
- [x] On the automatic plot, after the drag, each axis keeps the widened range:
      the x scale maps its domain ends to `plot.left()` and `plot.right()`
      within 0.01 px (GUP-444's policy, recorded in `zoomed`).
- [x] `view_axes(Axes::X)`: the y domain is bitwise unchanged by the drag and by
      a wheel step. `Axes::Neither`: no domain changes at all. `Drag::Off`: the
      drag changes no domain.
- [x] A plot with a `Band` x axis and a `Linear` y axis: a wheel step and a drag
      change the y domain and leave the band axis alone, returning
      `redraw: true`.

### AC3: Reset

- [x] After two wheel steps and a drag on the automatic plot, `reset_view()`
      then a resolve gives a `Layout` equal (`==`) to the first resolve's. Its
      `ChartExt::render` RGBA is byte-identical. Both axes are `is_auto()`.
- [x] On the explicit plot, `reset_view()` restores the explicit domains
      bitwise.
- [x] `reset_view()` with no prior view change is a no-op: it does not bump the
      revision, so the next resolve is cached (`resolve_generation` unchanged).

### AC4: Changed default — drag pans

- [x] `git grep -n "PointerDown" -- crates examples dogfood` lists every sender.
      Each is listed in the retrospective as "no move beyond 4 px, so
      unaffected" or "updated". `tests/events.rs`'s click tests (press and
      release with no move between) pass unchanged.
- [x] The docs of `Plot::handle`, `GupApp`, `show` and `web::CanvasApp` list
      drag pan among the behaviours, with `Plot::drag(Drag::Off)` to turn it
      off.

### AC5: Hosts

- [x] `CanvasApp` calls `set_pointer_capture` on `pointerdown` and
      `release_pointer_capture` on `pointerup` (errors ignored), and sets the
      canvas's `touch-action` to `none` when attached.
      `cargo clippy --target     wasm32-unknown-unknown -p gup-core --features web`
      is clean.
- [x] `GupApp` needs no change; a unit test in `app.rs` checks that
      `CursorMoved` with a button held still maps to `Input::PointerMoved`.

### AC6: Browser

- [x] The harness exports
      `InteractiveScatter::value_at(x: f32, y: f32) -> Vec<f64>`
      (`Plot::invert`, empty when `None`). In `index.html`, a step `pan` after
      GUP-452's `release` step: `pointerdown` at the plot centre, three
      `pointermove`s to `+30, +20` in equal steps, `pointerup`, then two frames.
- [x] `value_at(end)` equals `value_at(start)` taken before the press (relative
      error ≤ 1e-9 of each domain's width); `clicks()` is unchanged.
- [x] **Pixels**: `row_px(3)` moved by `(30, 20)` ± 0.5 px, and in
      `render_rgba()` the pixel at the new `row_px(3)` is within ±8 per channel
      of the dot's fill `#2a6fdb`.
- [x] `mask wasm-browser` passes with `interactive=ok`.

### AC7: Record

- [x] RFC-001 gains "S9b findings": the pan formula, the band-axis rule, and the
      default.
- [x] `mask wasm-size` passes. Growth over 6,000 B gz on `wasm.scatter` is an
      escalation, not a re-record. `wasm.per_row_type.gz_bytes` moves by less
      than 1% (pan adds no `Layer` method).

## Decisions

- **Types** (`crates/gup-core/src/input.rs`, re-exported at the root and in the
  prelude):

  ```rust
  /// Which axes pointer input may zoom and pan.
  #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
  #[non_exhaustive]
  pub enum Axes { #[default] Both, X, Y, Neither }

  /// What a primary-button drag does.
  #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
  #[non_exhaustive]
  pub enum Drag { #[default] Pan, Off }      // GUP-454 adds Brush and Zoom
  ```

- **Plot API**

  ```rust
  impl Plot {
      /// Axes the wheel zooms and a drag pans (default both).
      pub fn view_axes(&mut self, axes: Axes) -> &mut Self;
      /// What a primary-button drag does (default `Drag::Pan`).
      pub fn drag(&mut self, drag: Drag) -> &mut Self;
      /// Move the view so the content moves `(dx, dy)` logical pixels
      /// (both axes; like `zoom`, a direct call ignores `view_axes`).
      pub fn pan(&mut self, dx: f32, dy: f32) -> Result<()>;
      /// Back to the domains before the first zoom or pan: automatic axes
      /// fit the data again, explicit ones get their domain back.
      pub fn reset_view(&mut self) -> &mut Self;
  }
  pub trait PositionScale { /* … */ fn clear_domain(&mut self); }
  ```

  `view_axes` and `drag` bump the revision only if the value changes.

- **Pan formula**: `DynPositionScale::pan(&self, dpx: f64) -> Result<()>`, a
  provided method next to `zoom`. With domain `(d0, d1)` and `r = eval(d)`, the
  new domain is `(invert(r0 − dpx), invert(r1 − dpx))`. Like zoom, it works in
  pixels, so log, pow and time scales pan in their own space.
- **Band and point axes are not moved by input.** `handle` skips an axis whose
  scale `reads_keys()`, and zooms or pans the other one. `Plot::zoom` and `pan`
  called directly keep returning `Band`'s error, as `zoom` does today.
- **Drag state machine** in `handle` (primary button only; others unchanged):
  - `PointerDown` stores `press` as today.
  - `PointerMoved` with `press` set and `drag == Pan`: once the pointer is more
    than `CLICK_SLOP` from the press, the drag starts. Hover is cleared
    (`set_hover(None, at)`, so `Leave` runs), `hold_layout()` (GUP-452) is
    called, the view's home is recorded (below), and the domains at the press
    are stored. On this move and every later one, each `view_axes` axis's domain
    is set to the stored domain panned by the total delta `at − press`. Panning
    from the press, not from the last move, keeps rounding from accumulating.
    While dragging, `PointerMoved` does no picking.
  - `PointerUp` ends a drag without a click. Without a drag, the click rule is
    unchanged.
  - `PointerLeft` ends a drag (the domains stay) and releases the hold
    (GUP-452).
  - Both pan and wheel zoom record `zoomed[i]` for widened axes, as `zoom` does.
- **Home**: a private `home: Option<[Home; 2]>`, with
  `enum Home { Auto, Fixed(f64, f64) }`. It is recorded from `is_auto()` and
  `current_domain()` on the first view change since the last reset, whether by
  `zoom`, `pan` or `handle`. `reset_view` restores each axis (`clear_domain` for
  `Auto`, `set_domain` for `Fixed`), then clears `home` and `zoomed`, releases
  the hold and bumps the revision. `clear_domain` is a no-op on `Band` and
  `Point`, which are always automatic.
- **Default `Drag::Pan`**: `show` and `CanvasApp` already zoom on the wheel by
  default, and a chart that zooms but cannot pan strands the user. Charts that
  want the old behaviour set `Drag::Off`.
- **Browser host**: pointer capture keeps a drag alive when the pointer leaves
  the canvas mid-drag. `touch-action: none` stops touch drags from scrolling the
  page. It is set through `HtmlElement::style().set_property`, which needs the
  `web-sys` features `HtmlElement` and `CssStyleDeclaration`.

## Escalation Triggers

- Panning from the stored press domains on a `Time` or `Log` axis fails the 0.01
  px bound (the inversion is not exact enough).
- `reset_view` cannot reproduce the first resolve's layout exactly (some resolve
  state other than domains survives the view change).
- Pointer capture breaks the existing `leave` browser step.
- `wasm.scatter.gz_bytes` grows by more than 6,000 B gz.

## Out of Scope

- Brush selection and drag-zoom (`Drag::Brush`, `Drag::Zoom`): GUP-454.
- Zoom limits, keyboard navigation, pinch and multi-touch, and inertia.
- Double-click to reset: there is no double-click `Input`; hosts or users call
  `reset_view`.

## Technical Tasks

- [x] `Axes`, `Drag`, prelude; `Plot::{view_axes, drag, pan, reset_view}`.
- [x] `PositionScale::clear_domain` for seven scales; `DynPositionScale::pan`.
- [x] Drag state machine and band-axis skip in `handle`.
- [x] Tests (AC2, AC3) and the AC4 grep.
- [x] `web.rs` capture and `touch-action`; `app.rs` test; docs.
- [x] Harness hook `value_at`, the `pan` step.
- [x] RFC-001 "S9b findings"; `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-452: RFC-001 S9a Stable Plot Rect During Interaction 📋 — `hold_layout`,
  `Plot::invert`, the browser `release` step this story's step follows.
- GUP-444: RFC-001 S8a Cached Resolve and Input ✅ — `handle`, `zoomed`.
- GUP-449: RFC-001 S8f Web Canvas Host ✅ — `CanvasApp`.

### Enables Stories

- GUP-454: Brush and Drag-Zoom — extends `Drag` and the drag state machine.
- GUP-456: GPU Pick in Hover and Click — runs alongside the drag rules.

## Testing Strategy

- **Integration**: synthetic `Input` sequences in `tests/resolve_cache.rs` (AC2,
  AC3) with 0.01 px and bitwise checks.
- **Unit**: `clear_domain` per scale; `pan` on `DynPositionScale` for linear and
  log; `app.rs` mapping.
- **Browser**: the `pan` step, by values and by pixels.
- **Doctest**: AC1.

## Success Metrics

- [x] The value under the pointer at the press is under the pointer for the
      whole drag (≤ 0.01 px native, 1e-9 relative in the browser).
- [x] `reset_view` returns exactly to the first view.

## Risk Assessment

- **Low**: drag pan on by default might surprise a chart that expected a drag to
  do nothing. _Mitigation_: `Drag::Off`; the AC4 grep.
- **Low**: synthetic `pointermove` with capture in headless Chromium may behave
  differently from a real mouse. _Mitigation_: GUP-449 found synthetic pointer
  events reliable; capture errors are ignored.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] `mask wasm-browser` passes; the implementer reads `browser_scatter.png`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Axes`, `Drag` (`input.rs`, root and prelude);
  `Plot::{view_axes, drag, pan, reset_view}`; `PositionScale::clear_domain`
  (seven scales); `DynPositionScale::pan`. The drag state machine, band-axis
  skip and `home` are in `Plot::handle`. `web.rs` captures the pointer and sets
  `touch-action`; docs of `handle`, `GupApp`, `show` and `CanvasApp` list drag
  pan.
- Tests: seven in `tests/resolve_cache.rs` (explicit and automatic panning, log
  axis, widened range, view axes and `Drag::Off`, band axis, reset), one scale
  unit test per scale and a `pan` test, an `app.rs` mapping test, a doctest.
  `gup-core` suite passes (`--test-threads=1`). Browser `pan` step passes
  (`interactive=ok`).
- RFC-001 "S9b findings" added. `wasm.scatter.gz_bytes` 296,149 to 296,974 (+825
  B, limit +6,000). `wasm.per_row_type.gz_bytes` 1,003 to 1,144 (a gzip
  differential; no `Layer` method added). `mask perf-budget` all pass.

## Retrospective

**Completed**: 2026-10-18

### Key Technical Learnings

#### Pan from the press

- **Challenge**: Panning by each move's delta accumulates rounding and, with the
  hold, drifts on widened axes.
- **Solution**: Store the domains when the drag starts and set each to the
  stored domain panned by the total delta. The 0.01 px bound held on linear and
  log axes with no escalation.
- **Pattern**: Interactions that are a function of (start state, total input)
  are idempotent per move.

#### Auto axes need no special reset

- **Challenge**: `reset_view` must reproduce the first layout exactly.
- **Solution**: Every full resolve refits and niceifies automatic scales, so
  `clear_domain` only clears the `explicit` flag; the Layout and RGBA matched.

### Architectural Decisions

#### Band skip lives in `handle`, not in `zoom`

- **Decision**: `zoom_axes`/`pan_from` take an axis mask; `handle` passes
  `view_axes` minus keyed axes; public `zoom` and `pan` pass both and keep the
  band error.
- **Trade-off**: A band axis cannot be panned by input at all (no category
  scrolling) until a story needs it.

### Development Workflow Insights

- AC4 grep (`PointerDown` senders): `tests/events.rs` (click tests, 3 senders,
  press and release with no move between: unaffected; the 5 px test moves only
  at release), `tests/resolve_cache.rs` (`inputs_that_do_nothing`: no move after
  the press, unaffected; new GUP-453 tests: updated by design), `src/app.rs`
  (mapping and a new test), `src/web.rs` (the host: now captures the pointer),
  `src/plot.rs` (`handle` and its doctest). No examples or dogfood crates send
  it.
- Worktree baseline for the wasm-size comparison needed its own target dir.

### Follow-up Stories

None. (GUP-454 brush and drag-zoom already extends `Drag`.)
