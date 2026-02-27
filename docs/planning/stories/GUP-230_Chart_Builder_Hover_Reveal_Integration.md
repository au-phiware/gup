# GUP-230: Chart Builder Hover Reveal Integration

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Chart Builder Enhancements  
**Priority**: Low  
**Story Points**: 3  
**Status**: ✅ Complete  
**Dependencies**: GUP-200 (Interactive Clipping Reveal), GUP-018 (Chart
Builders)

## Problem Statement

The hover reveal system (GUP-200) provides `ClippedTextRegistry` and
`HoverRevealState` as standalone components that must be manually wired into
each chart's rendering loop. Chart builders should automatically support hover
reveal for axis labels and chart titles without requiring users to manage the
registry and state machine themselves.

## User Story

**As a** chart builder user  
**I want** hover reveal to work automatically on truncated axis labels  
**So that** I can see full label text without extra configuration

## Acceptance Criteria

- [x] `ChartBuilder` owns a `ClippedTextRegistry` and `HoverRevealState`
- [x] Axis labels and chart titles automatically register with the registry when
      clipped
- [x] `ChartBuilder` accepts mouse position updates for hover detection
- [x] Tooltip text is automatically queued for rendering during chart draw
- [x] Hover reveal can be enabled/disabled via a chart builder configuration
      option
- [x] `TooltipConfig` can be customized through the chart builder API

## Technical Tasks

1. Add `ClippedTextRegistry` and `HoverRevealState` fields to the chart builder
   state
2. Wire axis label rendering to register clipped text
3. Add `with_hover_reveal(bool)` and `with_tooltip_config(TooltipConfig)`
   builder methods
4. Queue tooltip rendering during chart draw phase
5. Pass mouse position updates from the application to the chart builder

## Dependencies

- GUP-200 (Interactive Clipping Reveal) — provides core hover reveal types
- GUP-018 (Chart Builders) — chart builder infrastructure

## Testing Strategy

- Integration tests: chart builder with clipped axis labels shows tooltip
- Configuration tests: enable/disable, custom tooltip config
- Visual test with a chart that has long axis labels

## Success Metrics

- Zero additional code required by the user beyond `with_hover_reveal(true)`
- Tooltip appears within 0.3s of hovering over truncated axis labels

## Risk Assessment

- **Mouse position forwarding**: The chart builder doesn't currently receive
  mouse events; an input mechanism needs to be designed.
- **Rendering order**: Tooltip must render last (on top of all chart elements).

## Definition of Done

- [x] Chart builder automatically supports hover reveal
- [x] Configuration via builder API
- [x] Tests passing
- [x] Example demonstrating automatic hover reveal in a chart

## Implementation Summary

### What Was Implemented

- **ChartConfig extensions**: Added `hover_reveal: bool` and
  `tooltip_config: TooltipConfig` fields with `with_hover_reveal()` and
  `with_tooltip_config()` builder methods.
- **ConfigurableBuilder trait**: Extended with `hover_reveal()` and
  `tooltip_config()` methods, implemented across all 6 chart builders (scatter,
  line, bar, area, heatmap, boxplot).
- **ComposedChart state**: Added `ClippedTextRegistry` and `HoverRevealState`
  fields, initialised from `ChartConfig` in `ComposedChart::new()`.
- **Hover update**: `update_hover(mouse_x, mouse_y, dt)` method forwards to the
  internal `HoverRevealState`.
- **Automatic clipping registration**: `queue_chart_text()` and
  `queue_chart_text_resolved()` now pass viewport bounds and clipping config
  when hover reveal is enabled, and register clipped text entries in the
  registry per frame.
- **Tooltip queuing**: `queue_tooltip_text()` method checks for active tooltip
  and queues text for rendering.
- **TextRenderer extension**: `queue_text_with_fonts_layout()` method returns
  full `LayoutResult` (including `original_text` for clipping detection).
- **Example**: `chart_hover_reveal_demo` demonstrates the complete integration.

### Key Files Changed

- `src/chart_builder.rs` — ChartConfig, ComposedChart, hover reveal wiring
- `src/chart_builder/builders.rs` — ConfigurableBuilder trait extension
- `src/chart_builder/builders/{scatter,line,bar,area,heatmap,boxplot}.rs` —
  trait implementations
- `src/text/renderer.rs` — `queue_text_with_fonts_layout()` method
- `examples/chart_hover_reveal_demo.rs` — full demo example
- `examples/multi_font_chart_demo.rs` — updated for `&mut self` signature

### Test Counts

- 18 new tests in `tests_hover_reveal` module
- All 1827+ existing tests continue to pass

---

**Story Created**: 2026-02-27  
**Story Completed**: 2025-07-15  
**Origin**: GUP-200 retrospective follow-up

## Retrospective

**Completed**: 2025-07-15

### Key Technical Learnings

#### Mutable Borrow Propagation Through API Boundaries

- **Challenge**: Changing `queue_chart_text()` from `&self` to `&mut self` to
  allow the clipped text registry to be mutated required updating all callers —
  including 11 test functions and 1 example.
- **Solution**: Used scripted line-number-based sed edits to add `mut` to all
  affected bindings efficiently.
- **Pattern**: When adding mutable state to a previously-immutable public
  method, plan for cascading `mut` requirements across the call chain. Factor
  state that needs mutation into separate methods with narrow `&mut self` scopes
  where possible.

#### Parallel Return Type Design for Layout Results

- **Challenge**: `queue_text_with_fonts` returns `TextBounds` but hover reveal
  needs the full `LayoutResult` (which includes `original_text` for clipping
  detection). Changing the return type would break existing callers.
- **Solution**: Added a parallel method `queue_text_with_fonts_layout` that
  returns the full `LayoutResult`. Both methods share the same internal logic.
- **Pattern**: When extending a public API, prefer adding a new method with a
  richer return type over modifying the existing signature. Name it with a
  suffix that describes what extra information it provides.

### Architectural Decisions

#### Hover State Owned by ComposedChart, Not ChartConfig

- **Decision**: `ClippedTextRegistry` and `HoverRevealState` are fields on
  `ComposedChart`, not on `ChartConfig`.
- **Reasoning**: `ChartConfig` is a lightweight, cloneable configuration struct.
  `ClippedTextRegistry` and `HoverRevealState` are mutable runtime state that
  shouldn't be cloned between frames.
- **Trade-off**: Users must call `update_hover()` on the `ComposedChart`
  instance, not on the config. This is consistent with how `render()` works.
- **Future**: If charts gain an event loop integration, `update_hover()` could
  be called automatically.

#### Viewport Bounds Per Axis Position

- **Decision**: `label_viewport_bounds()` computes a separate viewport region
  for each axis position (bottom, left, top, right) based on the chart area.
- **Reasoning**: Labels on different axes have different available space. Bottom
  axis labels are constrained to the bottom margin area, left axis labels to the
  left margin area, etc.
- **Trade-off**: Simple box-based clipping may not perfectly match the actual
  label spacing. For very dense labels, the collision resolution system
  (LabelPositioner) is a better solution.
- **Future**: Could be enhanced with per-tick-interval viewport bounds for more
  precise clipping detection.

### Development Workflow Insights

- The existing `hover_reveal_demo.rs` example was a helpful reference for
  understanding the standalone hover reveal API, making the integration path
  clear.
- The `ConfigurableBuilder` trait required adding methods to all 6 builder
  implementations. The implementations are identical across builders since they
  all delegate to `self.config`. A macro or blanket implementation could reduce
  this boilerplate in future.
- Pre-commit hooks catching pre-existing clippy warnings in the macros crate
  occasionally caused false-positive commit failures. The `mask all-fix` command
  resolved these consistently.

### Follow-up Stories

No new follow-up stories identified. The integration is self-contained and
builds on the stable foundations from GUP-200 and GUP-018.
