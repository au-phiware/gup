# GUP-013: Event Handling System

## Story Overview

**Initiative**: Interaction & Spatial Index
**Status**: 🚧 In Progress
**Created**: 2025-01-30

## Context

GUP-012 delivered a GPU-accelerated interaction system capable of performing
hit tests and element picks at high throughput. However, that system operates at
a low level — it returns raw hit results but provides no way for a developer to
register interest in those results or act on them. This story builds the
developer-facing event layer that sits on top of GUP-012.

The design is directly inspired by D3.js's `.on(event, handler)` API and DOM
event semantics, which are already familiar to the target audience of
visualization developers. The goal is that attaching a click handler to a
`Selection<T, Circle>` should feel as natural as it does in D3 or a modern
frontend framework — while internally routing through the GPU interaction
pipeline established in GUP-012.

GUP-002 established the `Selection<T, M>` type that developers work with day
to day. This story extends that type with the `.on()` method and the supporting
`EventManager` that bridges raw window input, GPU hit results, and typed Rust
closures. Event propagation (bubbling and cancellation) is required so that
overlapping elements and layered selections behave predictably.

## User Story

> "As a visualization developer, I want to register typed event handlers on
> selections using a familiar `.on(event, handler)` API so that I can add hover
> effects, click responses, drag behaviour, and other interactions without
> reasoning about low-level GPU hit-test plumbing."

## Acceptance Criteria

### AC1: `.on()` API on Selection

- [ ] `Selection<T, M>` exposes a chainable `.on(event: &str, handler: F) -> &mut Self`
      method where `F: Fn(InteractionEvent, &T) + Send + Sync + 'static`
- [ ] Handlers are keyed by a string event name (e.g. `"click"`, `"mouseenter"`,
      `"mousemove"`, `"mousedown"`, `"mouseup"`, `"touchstart"`, `"touchmove"`,
      `"touchend"`)
- [ ] Multiple handlers for the same event name on the same selection are all
      invoked in registration order
- [ ] The method compiles and passes `cargo check --examples`

### AC2: Event types and data structures

- [ ] `InteractionEvent` carries: event kind, cursor/touch position in
      visualization-space coordinates, a timestamp, keyboard modifier flags, and
      the `ElementHit` from GUP-012 (element id, selection id, data index)
- [ ] `EventType` covers mouse events (`Move`, `Down`, `Up`, `Enter`, `Leave`)
      and touch events (`Start`, `Move`, `End`)
- [ ] `EventResult` has at least two variants: `Continue` (allow propagation to
      continue) and `StopPropagation` (halt further bubbling for this event)
- [ ] All public types implement `Debug`

### AC3: Event routing and propagation

- [ ] `EventManager` receives a raw window input event, invokes GUP-012's hit
      test, and dispatches the resulting `InteractionEvent` to registered
      handlers in hit-depth order (front-most element first)
- [ ] When a handler returns `EventResult::StopPropagation`, no further handlers
      are invoked for that event dispatch
- [ ] A handler registered on selection A does not fire when the hit resolves to
      an element belonging to selection B
- [ ] Global (selection-independent) handlers can be registered and receive
      every dispatched event regardless of hit result

### AC4: Performance baseline

- [ ] End-to-end latency from raw input receipt to handler return is measurably
      below 16 ms for a chart with 10 000 visible elements and 50 registered
      handlers, verified by a benchmark or timing assertion in the test suite
- [ ] `EventManager` does not allocate per-event heap memory in the hot path
      (handler lookup and dispatch must be allocation-free for the common case)

## Technical Tasks

- [ ] Define `EventType`, `InteractionEvent`, `ElementHit` (re-export or
      re-use from GUP-012 where possible), and `EventResult` in a new
      `src/event.rs` module
- [ ] Implement `EventManager` with:
  - handler registration keyed by `(SelectionId, event name)`
  - global handler registration
  - `dispatch(&mut self, raw_input: RawInputEvent)` that calls GUP-012 hit
    test, builds `InteractionEvent`, and invokes matching handlers in order
  - `StopPropagation` short-circuit logic
- [ ] Add `.on()` method to `Selection<T, M>` that registers a typed closure
      with `EventManager`, wrapping the downcast from `&dyn Any` to `&T`
- [ ] Wire `EventManager::dispatch` to the window/surface event loop so that
      `winit` (or equivalent) input events reach the manager
- [ ] Convert raw cursor coordinates to visualization-space coordinates using
      the viewport transform already available in `GupContext`
- [ ] Add convenience methods `.on_click()`, `.on_hover()`, and `.on_drag()`
      to `Selection` as thin wrappers over `.on()`
- [ ] Write unit tests for: handler registration, propagation with
      `StopPropagation`, per-selection filtering (handler A does not fire for
      selection B's hit), and global handler receipt
- [ ] Write an integration test that constructs a chart, binds `.on("click")`
      to a selection, simulates a synthetic input event, and asserts the handler
      was invoked with the correct data
- [ ] Write or extend a benchmark asserting the <16 ms end-to-end latency
      requirement (AC4)
- [ ] Add a runnable example (`examples/interactive_circles.rs` or similar)
      demonstrating hover highlighting and click logging

## Dependencies

### Prerequisite Stories

- GUP-002: Core Selection Type ✅ — provides `Selection<T, M>` that `.on()` is
  added to
- GUP-012: GPU Interaction System ✅ — provides the hit-test pipeline that
  `EventManager` calls to resolve raw input to element hits

### Enables Stories

- GUP-014: Performance Validation — event handling latency is one of the
  performance targets validated in that story
- GUP-277: Zoom/Pan Interactions — zoom and pan are built as event handlers
  registered via the API delivered here

## Testing Strategy

- **Unit tests**: handler registration and dispatch in isolation (no GPU
  required); propagation halts correctly on `StopPropagation`; per-selection
  filtering prevents cross-selection handler leakage; global handlers receive
  all events
- **Integration tests**: full round-trip from synthetic `winit` cursor event →
  GPU hit test → typed Rust handler invoked with correct `&T` data; multiple
  overlapping selections dispatch in correct order
- **Performance**: benchmark or `assert!(elapsed < 16ms)` test covering the
  dispatch path with a realistically-sized chart (10 000 elements, 50 handlers)
- **Visual validation**: run the interactive example, hover over elements to
  confirm highlight updates, click to confirm console output — no GPU validation
  errors in the wgpu debug layer

## Success Metrics

- [ ] The `.on("click", handler)` pattern from the implementation strategy
      compiles and behaves correctly against a live wgpu surface
- [ ] All unit and integration tests pass: `cargo test -- --test-threads=1`
- [ ] The end-to-end event latency benchmark meets the <16 ms target
- [ ] An interactive example runs without GPU validation errors or panics

## Risk Assessment

- **Medium**: Bridging the async GPU hit-test from GUP-012 with synchronous
  closure dispatch may require care around `Send + Sync` bounds and lifetime
  management for captured references.
  _Mitigation_: Scope handlers to `'static` (owned data only) and poll the
  hit-test result synchronously within the dispatch call rather than spawning
  async tasks.

- **Medium**: Type-erasing `T` to `&dyn Any` for storage and recovering it via
  `downcast_ref` is a known Rust pattern but can produce confusing silent
  mismatches if `SelectionId` tracking is off by one.
  _Mitigation_: Assert in debug builds that every registered handler's
  `TypeId` matches the hit element's selection before downcasting.

- **Low**: Event coalescing for high-frequency `mousemove` events is not in
  scope for this story. If the event queue grows unbounded under rapid mouse
  movement, the latency target may be violated.
  _Mitigation_: Document the known limitation; coalescing is deferred to a
  follow-up story.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
