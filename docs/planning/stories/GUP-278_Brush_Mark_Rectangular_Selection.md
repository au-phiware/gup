# GUP-278: Brush Mark for Rectangular Selection

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress **Created**:
2025-07-25

## Context

Brushing — dragging to define a rectangular selection region — is one of the
most fundamental interactions in exploratory data visualization. In D3.js it is
delivered by `d3.brush`, which is among the library's most widely used modules.
It allows users to filter, highlight, or zoom into a sub-region of data without
writing manual hit-testing logic.

The project already has the core GPU machinery to support this. GUP-012
delivered a GPU interaction system with `query_region`, GUP-075 implemented
interactive mark selection including `SelectionToolKind::Rectangle` (a low-level
drag-to-rect building block inside `MarkSelectionSystem`), and GUP-234 added
touch lasso selection on top of those primitives. However, no high-level
developer-facing brush API exists. There is no `BrushBehavior` type, no
`BrushEvent`, and no visual overlay mark that gives the user feedback while they
drag. The `SelectionToolKind::Rectangle` mechanism in `mark_selection.rs` is an
internal detail — it handles input state but does not emit typed brush events or
render the dragged rectangle as a first-class overlay.

This story closes that gap: it delivers a D3-inspired `BrushBehavior` builder
that attaches to a chart, renders a semi-transparent `BrushMark` overlay during
a drag gesture, performs a GPU region query on drag-end, and fires a
`BrushEvent` carrying the selected element IDs. Brush coordinates must be mapped
through the chart's current viewport transform so that the selection is correct
under pan and zoom.

## User Story

> "As a visualization developer, I want to attach a rectangular brush to a chart
> so that users can drag to select a region and my application receives the IDs
> of all data points inside it."

> "As an end user, I want to see a semi-transparent rectangle track my drag so
> that I understand the current selection region before I release the mouse."

## Acceptance Criteria

### AC1: BrushBehavior API

- [ ] A public `BrushBehavior` struct exists with a builder-style API:
  ```rust
  let brush = BrushBehavior::new()
      .on("brush", |event: BrushEvent| { /* handler */ })
      .on("brushend", |event: BrushEvent| { /* handler */ });
  chart.brush(brush);
  ```
- [ ] `BrushBehavior::new()` compiles without requiring extra type parameters.
- [ ] `.on(event_name, handler)` accepts at least `"brush"` (drag in progress)
      and `"brushend"` (drag released) as event names.
- [ ] Attaching the same `BrushBehavior` twice replaces the first attachment
      rather than registering duplicates.

### AC2: BrushEvent Contents

- [ ] `BrushEvent` carries:
  - the brush rectangle in data-space coordinates (origin + extent as `[f32; 4]`
    or a named `BrushExtent` struct),
  - the brush rectangle in screen-space pixels,
  - a `Vec<u32>` of mark IDs that fall within the brush rectangle.
- [ ] `BrushEvent::selection` is empty (not `None`) when the drag produces a
      zero-area rectangle.
- [ ] `BrushEvent` implements `Clone` and `Debug`.

### AC3: Visual Overlay (BrushMark)

- [ ] A `BrushMark` type (or equivalent internal representation) renders the
      in-progress brush rectangle as a semi-transparent filled rectangle with a
      visible stroke border.
- [ ] The overlay is visible only while a drag is in progress; it disappears
      after `"brushend"` fires.
- [ ] The overlay renders above the data marks (higher z-order / rendered last
      in the frame).
- [ ] Default visual style: fill `rgba(0.4, 0.6, 1.0, 0.2)`, stroke
      `rgba(0.4, 0.6, 1.0, 0.8)`, stroke width `1 px`. Both are overridable via
      `BrushBehavior::style(BrushStyle { ... })`.

### AC4: GPU Region Query

- [ ] On `"brushend"`, the brush rectangle is submitted as a region query to the
      `MarkSelectionSystem` / GPU interaction pipeline from GUP-012/GUP-075.
- [ ] All mark IDs whose positions fall within the brush rectangle are returned
      in `BrushEvent::selection`.
- [ ] The query completes without GPU validation errors on both Vulkan and Metal
      backends.

### AC5: Viewport-Aware Coordinates

- [ ] When the chart has a non-identity viewport transform (pan offset or zoom
      scale), the brush rectangle is correctly inverse-transformed from screen
      space to data space before the GPU query and before populating
      `BrushEvent::data_extent`.
- [ ] A unit test verifies that a brush drawn at screen position `(100, 100)` to
      `(200, 200)` with a 2× zoom centred at the origin maps to the correct
      data-space rectangle.

### AC6: Example and Documentation

- [ ] An example `examples/brush_selection.rs` demonstrates attaching a brush to
      a scatter chart and printing selected IDs on `"brushend"`.
- [ ] `cargo check --examples` passes cleanly.
- [ ] Public types (`BrushBehavior`, `BrushEvent`, `BrushExtent`, `BrushStyle`)
      have `///` doc-comments.

## Technical Tasks

- [ ] Define `BrushExtent`, `BrushStyle`, `BrushEvent`, and `BrushBehavior`
      types in a new `src/brush.rs` module; re-export from `lib.rs`.
- [ ] Implement `BrushBehavior::new()`, `.on()`, `.style()`, and the internal
      `attach(&mut chart)` method.
- [ ] Add `Chart::brush(behavior: BrushBehavior)` method that registers the
      behavior and stores it as part of chart state.
- [ ] Implement drag-gesture detection inside the chart's input event loop:
      mouse-down → start rectangle, mouse-move → update rectangle + fire
      `"brush"`, mouse-up → fire `"brushend"` + clear overlay.
- [ ] Re-use `SelectionToolKind::Rectangle` / `ToolState::DraggingRect` from
      `mark_selection.rs` to track the drag rectangle; avoid duplicating
      geometry logic.
- [ ] Implement `BrushMark` (or adapt `RectangleMark` from GUP-067) as an
      overlay layer: allocate a single-instance GPU buffer, update it each frame
      while drag is active, skip rendering when idle.
- [ ] Integrate viewport inverse-transform: read `ViewportTransform` /
      `Viewport2D` from chart state and apply the inverse when converting screen
      coordinates to data-space.
- [ ] Wire `MarkSelectionSystem::handle_pointer_up` (or equivalent) to submit
      the region query via the GPU interaction pipeline and await results before
      firing `"brushend"`.
- [ ] Populate `BrushEvent` from query results and invoke registered handlers.
- [ ] Write unit tests for:
  - `BrushBehavior` builder methods,
  - viewport-transform inversion (AC5),
  - `BrushEvent` content when selection is empty vs non-empty.
- [ ] Write `examples/brush_selection.rs` demonstrating the full API.
- [ ] Add doc-comments to all public types.

## Dependencies

### Prerequisite Stories

- GUP-012: GPU Interaction System ✅ — provides `query_region` and the GPU hit
  testing pipeline that the brush region query is submitted to.
- GUP-075: Interactive Mark Selection ✅ — provides `MarkSelectionSystem`,
  `SelectionToolKind::Rectangle`, `ToolState::DraggingRect`, and `rect_select`;
  the brush reuses this infrastructure rather than reimplementing it.
- GUP-067: Rectangle and Line Mark Implementations ✅ — provides the
  `RectangleMark` / `RectangleVertex` GPU geometry used to render the
  `BrushMark` overlay.
- GUP-013: Event Handling System 📋 — provides the `.on(event, handler)`
  dispatch infrastructure that `BrushBehavior` hooks into for firing
  `BrushEvent`s; `BrushBehavior` may provide its own minimal dispatch if GUP-013
  is not yet complete, but the API shape must remain compatible.

### Enables Stories

- GUP-279: Linked View Coordination — the brush selection output (`BrushEvent`
  with selected IDs) is the primary input mechanism for coordinating state
  across linked charts; GUP-279 depends on a stable `BrushEvent` API from this
  story.

## Testing Strategy

- **Unit tests**: `BrushBehavior` builder API; `BrushEvent` construction and
  clone/debug; viewport inverse-transform for AC5; empty-selection edge case.
- **Integration tests**: Attach brush to a headless chart, simulate drag input
  events, assert that `"brushend"` fires with the correct element IDs from a
  small known dataset.
- **Visual validation**: Run `examples/brush_selection.rs`, drag a rectangle
  over a cluster of points, confirm the overlay rectangle tracks the cursor and
  disappears on release; inspect printed IDs.
- **GPU validation**: Run the integration test on Vulkan and Metal backends with
  the wgpu validation layer enabled; confirm zero validation errors.

## Success Metrics

- [ ] `cargo test -- --test-threads=1` passes with no new failures.
- [ ] The brush example compiles and runs:
      `cargo run --example brush_selection`.
- [ ] `BrushEvent::selection` contains the correct IDs for a 100-point synthetic
      dataset in the integration test.
- [ ] Viewport-transform unit test passes for 2× zoom case (AC5).
- [ ] No GPU validation errors on at least two wgpu backends.

## Risk Assessment

- **Medium**: The async GPU region query must complete before `"brushend"`
  handlers fire. If the GPU pipeline stalls, the handler invocation will block
  the event loop. _Mitigation_: Use the same async pattern already established
  in `MarkSelectionSystem`; consider a timeout or fallback CPU path for the
  headless test environment.

- **Low**: `BrushMark` overlay rendering order. If the overlay is not drawn
  after all data marks, it may be occluded. _Mitigation_: Add the brush overlay
  as the last render pass in the chart frame; document the z-order contract.

- **Low**: GUP-013 (Event Handling System) is still planned. The
  `BrushBehavior::on()` dispatch may need to be self-contained until GUP-013
  lands. _Mitigation_: Design `BrushBehavior`'s internal handler map to be
  trivially replaceable with the GUP-013 `EventManager` once available; keep the
  coupling surface small.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
