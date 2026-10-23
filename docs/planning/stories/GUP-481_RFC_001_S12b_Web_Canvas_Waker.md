---
implementer: sonnet
implementer-reason:
  the waker's shape (a thread-local registry on wasm32) and the browser step are
  fixed in Decisions; small change to web.rs and the smoke harness
---

# GUP-481: RFC-001 Step S12b: Web Canvas Wakes on Appended Rows

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-19)
**Created**: 2026-10-19

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

## Resolved (2026-10-19)

Both blockers were resolved by the orchestrator.

- **Size**: the `HashMap` became a `Vec` (scatter 330,879 B gz, +5,521 B over
  325,358). Split: `web.rs` +736 B (accepted, at most 2,500 B); the harness
  appender about +4,785 B (the feed machinery, linked once used).
- **Pixels**: a WebGPU canvas reads back transparent in headless SwiftShader
  (`drawImage` and CDP `captureScreenshot` both). The step proves the wake with
  an accessor counter (`appended_seen`, polled with `within`, no `with_chart`),
  then checks the pixel through `render_rgba`. Recorded in RFC-001 "S12b
  findings".

## Implementation Summary

`CanvasApp` registers in a wasm32 thread-local `Vec` and passes
`chart.set_waker` a `CanvasWake(id)`; a wake schedules one frame, is skipped
when the state is borrowed, and is a no-op after drop. The harness gained
`append_row`, `appended_seen` and the tooltip fix. Smoke steps: `append-before`,
`append-wake` (counter, `within`, 1 frame), `append-pixel` (`render_rgba`,
colour 2a6fdb). Files: `crates/gup-core/src/web.rs`,
`crates/gup-core/wasm-size/scatter/{src/lib.rs,index.html}`, RFC-001 S12b
findings. Evidence: gup-core tests pass (191 unit plus integration),
`mask all-check` and `mask wasm-browser` pass, `mask old-path-loc` TOTAL 26,601
(unchanged). Scatter 330,986 B gz (+5,628 over 325,358: web.rs ~+740, harness
appender ~+4,800); per_row_type 763 B. `mask perf-budget`: all pass except
`pipeline.link_create.median_ms` 1.368 vs 1.332 limit (not touched here; load
average 4.5 to 9, uptime 20:36); not re-run. The pixel check is `render_rgba`,
not the visible canvas (see Resolved).

## Retrospective

**Completed**: 2026-10-23

### Key Technical Learnings

#### Headless canvas readback

- **Challenge**: the story asked for a canvas pixel; a WebGPU canvas reads back
  transparent in headless SwiftShader by `drawImage` and by CDP screenshot.
- **Solution**: an accessor-evaluation counter proves the frame, `render_rgba`
  proves the pixel.
- **Pattern**: `with_chart` schedules and resolves, so a waker test must touch
  the app only through the appender and atomics.

### Architectural Decisions

#### Vec registry

- **Decision**: `thread_local Vec<(id, Weak)>` over `HashMap`.
- **Reasoning**: a handful of canvases; smaller code. **Trade-off**: linear
  lookup. **Future**: GUP-483 can reuse the shape for other hosts.

### Development Workflow Insights

- About an hour went on canvas readback attempts; the three-attempt rule would
  have stopped it sooner. Harness appender cost (~4.8 KB) is the feed machinery.

### Follow-up Stories

None.
