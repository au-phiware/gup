---
implementer: sonnet
implementer-reason:
  when to use the GPU path, request coalescing, the needs_frame hook and the
  plot-level row events are settled in Decisions; builds on GUP-455's API
---

# GUP-456: RFC-001 Step S9e: GPU Pick in Hover, Click and Hosts

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-18

## Context

GUP-455 adds a GPU pick (`Plot::request_pick`, `poll_picks`) with the CPU pick's
semantics, for layers the CPU cannot read. Nothing uses it yet. `Plot::handle`
hovers and clicks through `pick_row`, which is synchronous and CPU-only, so a
`Retain::GpuOnly` layer past one chunk can be neither hovered nor clicked.
RFC-001 §9 says that `Plot` "resolves the hit (CPU, or the latest GPU pick)".

Two gaps besides the wiring:

- **Results arrive later.** `handle` is synchronous and has no `Context`. The
  pick has to be submitted at the next `resolve` (which has one) and applied
  when it lands, a frame or two later. Hosts redraw only when told to: `GupApp`
  on `Response::redraw`, and `CanvasApp` with one `requestAnimationFrame` per
  burst of input. So the chart needs a way to ask for another frame while a pick
  is in flight.
- **Handlers need rows.** `Selection::on` handlers receive `&T`, so a layer that
  dropped its rows runs none (GUP-446: "columns-only layers hover but run no
  handler"). Under `GpuOnly`, hovering would then be invisible to the program. A
  plot-level handler that receives the `RowId` fixes that.

## Read First

- [GUP-455](GUP-455_RFC_001_S9d_GPU_Pick_Pass.md) Decisions (API) and RFC-001
  "S9d findings" (written by GUP-455; it overrides GUP-455's Decisions where
  they differ).
- [GUP-446](GUP-446_RFC_001_S8c_Hover_Click_Tooltips.md) and
  [GUP-453](GUP-453_RFC_001_S9b_Drag_Pan_View_Axes_Reset.md) Decisions (events,
  drag state machine).
- `crates/gup-core/src/plot.rs` lines 448–530 (`resolve`, `sync_tooltip`) and
  545–720 (`handle`, `set_hover`, `dispatch`, `pick_row`).
- `crates/gup-core/src/chart.rs` (the `Chart` trait).
- `crates/gup-core/src/app.rs` lines 240–345 (redraw and event loop) and
  `src/web.rs` lines 240–320 (`draw`, `schedule`, `frame`, `dispatch`).
- `crates/gup-core/wasm-size/scatter/src/lib.rs` lines 760–923 and `index.html`
  lines 137–245.

## User Story

> "As a developer plotting a large `GpuOnly` dataset, I want hover and click to
> work in a window and in a browser, so that big data is as interactive as small
> data."

## Acceptance Criteria

### AC1: API

- [ ] `Chart::needs_frame` and `Plot::on_row` exist as in Decisions, with
      doctests. `needs_frame`'s default is `false`, so other `Chart`
      implementors compile unchanged.

### AC2: When the GPU path is used

- [ ] A plot whose layers are all CPU-readable keeps today's synchronous path:
      `tests/events.rs` passes unchanged, `needs_frame()` is always `false`, and
      `Context::submissions` does not move on `PointerMoved`.
- [ ] A plot with one `Retain::GpuOnly` layer past one chunk
      (`max_chunk_rows(4)`, 12 rows) uses the GPU path for every layer, both
      hover and click.

### AC3: Hover and click through the GPU (native)

With that fixture in `tests/events.rs`, and helper
`settle(plot, cx) = resolve until !needs_frame(), at most 10 times`:

- [ ] `PointerMoved` at row 5's position returns `redraw: true`, and
      `needs_frame()` is true. After `settle`, `hovered()` is row 5, and
      `on_row` saw `Enter` for row 5.
- [ ] Ten `PointerMoved`s between two resolves submit **one** pick, for the last
      position (count `Context::submissions`), and `hovered()` follows the last
      position.
- [ ] A result for an older position is dropped: move to row 5, resolve once
      (submitted), move to empty space, settle. `hovered()` is `None`, and
      `on_row` saw `Enter(5)` then `Leave(5)`, or nothing. It never sees an
      `Enter` that arrives after the `Leave`.
- [ ] Press and release within 4 px on row 7: after `settle`, `on_row` saw
      `Click` for row 7. Release beyond 4 px (a pan, GUP-453): no `Click`.
- [ ] A drag (pan) cancels a pending hover pick, and no `Enter` arrives during
      the drag.
- [ ] A `GpuOnly` row's tooltip: none (no rows). On a mixed plot, a hovered
      CPU-readable row's tooltip draws as before, anchored at its pick anchor. A
      GPU-only row's hover uses the pointer as its anchor.

### AC4: Hosts ask for frames

- [ ] `GupApp` requests another redraw after rendering while `needs_frame()` is
      true. The decision is a pure function in `app.rs`,
      `fn redraw_again(chart: &dyn Chart) -> bool`, called after each render and
      unit-tested with a fake `Chart` whose `needs_frame` is true, then false.
- [ ] `CanvasApp` schedules another `requestAnimationFrame` after a frame while
      `needs_frame()` is true.

### AC5: Browser

- [ ] The harness exports `show_gpu_scatter(canvas)`: the interactive scatter's
      rows, `Retain::GpuOnly`, `max_chunk_rows(4)`, 12 rows, with `on_row`
      counting `Click`s and recording the last hovered and clicked rows. Hooks:
      `hovered_row()`, `clicks()`, `last_clicked()`, and
      `px_of(x: f64, y: f64) -> Vec<f32>` (data to chart pixels through the
      plot's scales, since `row_px` needs CPU rows).
- [ ] In `index.html`, steps `gpu-hover`, `gpu-click` and `gpu-leave`. Each
      dispatches the event, then waits up to 8 animation frames for the hook to
      reach the expected value: row 5 hovered; one click on row 7; −1 after
      `pointerleave`.
- [ ] `mask wasm-browser` passes with `interactive=ok` and no console error.

### AC6: Record

- [ ] RFC-001 gains "S9e findings": frames to a hover result on native and in
      SwiftShader, the coalescing rule, and the switch rule.
- [ ] `mask wasm-size` passes. This is where GUP-455's pick pass is first linked
      into the harness, so growth up to 10,000 B gz is re-recorded with the
      reason; over 10,000 B gz is an escalation. `wasm.per_row_type.gz_bytes`
      moves by less than 2%.

## Decisions

- **API**

  ```rust
  pub trait Chart {
      // …
      /// The chart is waiting on the GPU (a pick in flight) and wants
      /// another `resolve` soon even without input. Hosts redraw while true.
      fn needs_frame(&self) -> bool { false }
  }

  impl Plot {
      /// Called for every `Enter`, `Leave` and `Click` with the row's id,
      /// whether or not its layer kept the rows (unlike `Selection::on`).
      pub fn on_row(&mut self, f: impl FnMut(&EventInfo) + Send + Sync + 'static) -> &mut Self;
  }
  ```

  `on_row` handlers run after the layer's own handlers, for both the CPU and GPU
  paths.

- **Switch rule**: the plot uses GPU picks for hover and click when, after the
  last resolve, some layer with a `gpu_pick` mark is not CPU-readable
  (`Layer::cpu_readable() -> bool` in `layer.rs`: CPU columns kept, or rows
  kept). Then every layer is picked on the GPU, so there is one semantics and
  one source of truth. Otherwise nothing changes.
- **Coalescing**: one hover pick in flight at a time. `PointerMoved` stores
  `pending = Some(at)`, replacing any earlier pending point. `resolve` (after
  its cache step) first applies finished picks, then submits `pending` if no
  hover pick is in flight. A finished hover pick is applied only if it is the
  latest submitted and no newer point is pending. Otherwise it is dropped, and
  the pending point is submitted. Clicks are their own requests (at most one in
  flight), never dropped, and dispatched when they land if the row is `Some`.
- **Applying a hover result** is `set_hover(row, at)`, with the anchor taken
  from the pick index when the layer has one, else the pointer position.
- **`needs_frame`** is true while a pick is pending or in flight.
- **Drag interaction**: starting a pan, brush or drag-zoom clears `pending` and
  marks any in-flight hover pick as dropped.
- **Resolve generation**: a result whose `generation` differs from the current
  one (the view changed since) is dropped for hover and re-requested at the last
  pointer position. A click result is applied regardless: the user clicked what
  they saw.

## Escalation Triggers

- GUP-455's shipped API differs from its Decisions in a way that this story's
  Decisions cannot absorb, beyond renames.
- `Retain::GpuOnly` with a small `max_chunk_rows` does not release the CPU copy
  in the browser harness, so the GPU path cannot be shown there.
- A hover result needs more than 8 animation frames in SwiftShader.
- `wasm.scatter.gz_bytes` grows by more than 10,000 B gz.

## Out of Scope

- Brushing GPU-only layers (GUP-454 records it as out of scope).
- Tooltips for GPU-only rows: there are no rows to format.
- egui and bevy hosts: RFC-001 S13.

## Technical Tasks

- [ ] `Chart::needs_frame`; `Plot::on_row`; `Layer::cpu_readable`.
- [ ] Pending, in-flight and coalescing state in `Plot`; the apply step in
      `resolve`; the drag interplay.
- [ ] `app.rs` and `web.rs` frame requests and their test.
- [ ] Native tests (AC2, AC3); harness scene, hooks and steps (AC5).
- [ ] RFC-001 "S9e findings"; `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-455: RFC-001 S9d GPU Pick Pass 📋 — `request_pick`, `poll_picks`.
- GUP-453: RFC-001 S9b Drag Pan, View Axes and Reset 📋 — the drag state machine
  this story cancels picks from.
- GUP-454: RFC-001 S9c Brush and Drag-Zoom 📋 (soft) — merged first, to avoid
  conflicting edits in `handle` and `index.html`.

### Enables Stories

- None in S9. RFC-001 S12 (`appender`, `Window`) can then make streamed
  `GpuOnly` charts interactive.

## Testing Strategy

- **Integration (native)**: `tests/events.rs` GPU fixture, synthetic `Input`
  sequences plus `settle` (AC3), with submission counts.
- **Unit**: the host redraw request with a fake `Chart`.
- **Browser**: `gpu-hover`, `gpu-click` and `gpu-leave` steps.

## Success Metrics

- [ ] A `GpuOnly` layer hovers and clicks in a window and in a browser.
- [ ] Hover costs at most one pick per frame, however fast the pointer moves.

## Risk Assessment

- **Medium**: hover latency of one or two frames is visible on a fast pointer.
  _Mitigation_: coalescing keeps it to the latest point; CPU-readable plots keep
  the synchronous path.
- **Low**: handlers now run from inside `resolve` (when a result lands).
  _Mitigation_: documented on `on_row` and `Selection::on`; the order of `Leave`
  and `Enter` is preserved.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-browser` passes; the implementer reads `browser_scatter.png`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
