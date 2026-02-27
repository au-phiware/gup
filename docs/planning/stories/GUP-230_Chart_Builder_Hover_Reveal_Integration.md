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
