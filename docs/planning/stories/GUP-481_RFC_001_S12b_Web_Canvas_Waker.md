---
implementer: sonnet
implementer-reason:
  the waker's shape (a thread-local registry on wasm32) and the browser step are
  fixed in Decisions; small change to web.rs and the smoke harness
---

# GUP-481: RFC-001 Step S12b: Web Canvas Wakes on Appended Rows

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-19

## Context

GUP-480 adds `Appender` and `Chart::set_waker`: a host passes a
`std::task::Waker` that schedules a frame, and appenders wake it when rows
arrive. It wires `GupApp` (native windows). The browser host,
`gup_core::web::CanvasApp` (GUP-449), schedules frames with
`requestAnimationFrame` only on input or while `needs_frame()` is true. A page
that appends rows from a timer or a WebSocket callback, with no pointer input,
would not see them until the next pointer move.

`std::task::Wake` needs `Send + Sync`, and `CanvasApp`'s state is an
`Rc<RefCell<State>>`, so the waker cannot hold it. On `wasm32-unknown-unknown`
everything runs on one thread, so a thread-local registry can map a plain id to
the canvas's state.

## Read First

- [GUP-480](GUP-480_RFC_001_S12a_Appender_Handle.md) "Decisions" (`Appender`,
  `Chart::set_waker`).
- `crates/gup-core/src/web.rs` lines 270–335 (`CanvasApp`, `schedule`, `frame`,
  `dispatch`) and the constructor and `Drop` (search `impl CanvasApp`,
  `impl Drop`).
- `crates/gup-core/wasm-size/scatter/src/lib.rs` lines 765–830
  (`InteractiveScatter`, `show_scatter`).
- `scripts/browser_smoke.mjs` `drive` (from line 195) and the steps after
  `reset_view`.

## User Story

> "As a developer of a live dashboard in the browser, I want rows I append from
> a timer or socket callback to appear on the canvas without user input."

## Acceptance Criteria

### AC1: The waker

- [ ] `CanvasApp` calls `chart.set_waker(..)` once, when it is created, with a
      waker built as in Decisions.
- [ ] Waking schedules exactly one frame through the existing `schedule` (a
      second wake before the frame is a no-op, as `pending` already ensures).
- [ ] After the `CanvasApp` closes or is dropped, waking does nothing (no
      console error, no panic).

### AC2: Browser smoke step

- [ ] `InteractiveScatter` gains `appender` and
      `pub fn append_row(&self, x: f64, y: f64)`, and its tooltip closure no
      longer indexes `NAMES` out of range for rows past the sixth (Decisions).
- [ ] A new step in `scripts/browser_smoke.mjs`, right after the existing
      `reset_view` step: call `append_row(50, 9)`, dispatch no event, await two
      animation frames, then read the canvas pixel at `row_px(6)`. It must be
      `#2a6fdb` (exact on SwiftShader, as the brush step's colour check is).
- [ ] `mask wasm-browser` passes, and the implementer reads
      `browser_scatter.png` if the step writes the canvas there (or a screenshot
      of the page at the step).

## Decisions

- **Registry** (wasm32 only, in `web.rs`):

  ```rust
  thread_local! {
      static CANVASES: RefCell<HashMap<u64, Weak<RefCell<State>>>> = Default::default();
  }
  struct CanvasWake(u64);
  impl std::task::Wake for CanvasWake {
      fn wake(self: Arc<Self>) { self.wake_by_ref() }
      fn wake_by_ref(self: &Arc<Self>) {
          // look up self.0, upgrade, schedule(&rc); a missing or dead entry is a no-op
      }
  }
  ```

  Ids come from a `static AtomicU64`. `CanvasApp` registers itself at creation
  and removes its entry when closed or dropped. Reason: `Wake` requires
  `Send + Sync`; an id is both, and wasm32 has one thread.

- **No polling and no extra timer**: the waker is the only new trigger.
- **Harness**: `InteractiveScatter` stores `Appender<(usize, f64, f64)>` from
  `with_chart` → downcast to `Plot` → `layer_mut` → `appender()` (or keeps the
  appender from before `show`). `append_row` pushes `(n, x, y)` where `n` is the
  next row index. The tooltip uses `NAMES.get(r.0).copied().unwrap_or("New")`.
- **Placement of the step**: after `reset_view`, so the domain is the fixed
  0–100 × 0–10 and (50, 9) is clear of the six dots.

## Escalation Triggers

- An earlier smoke step leaves a brush active at the new step, so the new dot is
  dimmed. Clear it with an existing hook; if there is none, stop and report
  rather than adding one.
- `wasm.scatter.gz_bytes` grows by more than 3,000 B (the GUP-458 escalation
  size) from the appender in the harness.

## Out of Scope

- Native `GupApp`'s waker: GUP-480.
- egui's waker: GUP-483.

## Technical Tasks

- [ ] Registry, `CanvasWake`, registration and removal, `set_waker` call.
- [ ] Harness `append_row` and tooltip fix.
- [ ] Smoke step; run `mask wasm-browser`.

## Dependencies

### Prerequisite Stories

- GUP-480: RFC-001 S12a Appender Handle 📋 — `Appender`, `Chart::set_waker`.
- GUP-449: Web Canvas Host ✅.

### Enables Stories

- None.

## Testing Strategy

- **Browser**: the new smoke step (the only test that runs this code).
- **Lint**: `scripts/clippy_wasm32.sh` (part of `mask all-check`).

## Success Metrics

- [ ] An appended row is drawn in the browser with no input event.

## Risk Assessment

- **Low**: a `RefCell` borrow conflict if a waker fires while `frame` holds the
  state (an appender pushed from a handler during `draw`). _Mitigation_:
  `schedule` already returns early when `pending`; take the registry borrow only
  to clone the `Weak`, then release it before calling `schedule`.
- **Low**: trybuild `.stderr` churn: none expected (no edits to `selection.rs`,
  `encoding.rs` or `marks.rs`); if any appears, re-bless only line-number diffs.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser` passes (this story is wasm and input)
- [ ] `mask wasm-size` run and the scatter and per-row-type figures reported in
      the Implementation Summary (the orchestrator re-records the budget)
- [ ] Rendered output verified by eye: the browser capture at the new step
- [ ] `mask old-path-loc` recorded (expected unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-10-19)

Two problems. Code state: `web.rs` registry, `CanvasWake` and `set_waker` are
implemented; the harness `append_row` and the tooltip fix are in. The smoke step
is not committed.

1. **Escalation trigger 2 fires.** `wasm.scatter.gz_bytes` is 330,965 against a
   recorded 325,358 (clean `main`, same command): +5,607 B, over the 3,000 B
   limit. Not yet split between `web.rs` (`Arc`, `HashMap`, `Wake`) and the
   harness appender. Options: accept and re-record; replace the `HashMap` with a
   `Vec<(u64, Weak<..>)>` and measure; or drop the harness appender.
2. **AC2 as written cannot be checked by pixels.** A WebGPU canvas reads back
   transparent in headless SwiftShader: `drawImage(canvas)` in the rAF round
   after the app's frame gives `[0,0,0,0]` even at a drawn dot, and a CDP
   `Page.captureScreenshot` of the canvas rect is wholly transparent too
   (`/tmp/shot.png`, RGBA, all zero). `render_rgba` and `row_px` go through
   `with_chart`, which schedules a frame itself and drains the feed, so they
   cannot prove the waker. The waker works: console logging showed `wake`
   followed by `frame` with no input. Options: (a) a CPU-side proof, counting
   accessor evaluations of the appended row through an `Arc<AtomicUsize>` in the
   `Circle::X` closure (no `with_chart`, no pixels); (b) chase the screenshot
   (WIP kept at `/tmp/index_wip.html`, `/tmp/browser_smoke_wip.mjs`, a
   `GUP SHOT` handshake); (c) a gup-core hook exposing a frame counter.
   Recommendation: (a), with the pixel check on `render_rgba` after the frame as
   a second step.
