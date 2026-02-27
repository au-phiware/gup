# GUP-229: Tooltip Background Rendering

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 3  
**Status**: ✅ Complete  
**Dependencies**: GUP-200 (Interactive Clipping Reveal)

## Problem Statement

The hover reveal tooltip (GUP-200) currently renders only the text content
without a visual background. In a typical data visualization, tooltip text
overlaps with chart elements and is difficult to read without a contrasting
background box, border, and optional corner radius.

## User Story

**As a** chart user  
**I want** tooltips to have a visible background with configurable appearance  
**So that** I can clearly read tooltip content against any chart background

## Acceptance Criteria

- [x] Tooltip renders with a solid background rectangle behind the text
- [x] Background color, border color, and border width are configurable via
      `TooltipConfig`
- [x] Optional corner radius for rounded tooltip boxes
- [x] Background opacity matches the tooltip fade-in/fade-out animation
- [x] Tooltip shadow or drop-shadow effect (optional)
- [x] Background rendering uses a dedicated GPU rounded-rectangle pipeline

## Technical Tasks

1. Create a tooltip background renderer using the Rectangle mark or a dedicated
   solid-color quad shader
2. Wire background rendering into the `TooltipLayout` positioning system
3. Ensure background renders behind (before) the tooltip text
4. Add corner radius support to the background rectangle
5. Test background rendering with various tooltip configurations

## Dependencies

- GUP-200 (Interactive Clipping Reveal) — provides `TooltipConfig`,
  `TooltipLayout`, and `ActiveTooltip`

## Testing Strategy

- Unit tests for background bounds calculation
- Visual integration tests verifying background renders behind text
- Tests for rounded corner rendering

## Success Metrics

- Tooltips are clearly readable over any chart background
- No visual artifacts at tooltip edges

## Risk Assessment

- **GPU pipeline complexity**: May need a new simple-quad render pipeline if the
  mark system is too heavy for this use case.
- **Z-ordering**: Background must render before text in the same render pass.

## Definition of Done

- [x] Background rectangle renders behind tooltip text
- [x] Corner radius supported
- [x] Configurable via `TooltipConfig`
- [x] Tests passing
- [x] Updated demo showcasing the feature

## Implementation Summary

**Completed**: 2025-07-18

### Architecture

A dedicated `TooltipBackgroundRenderer` draws tooltip backgrounds using
instanced rendering of a unit quad with an SDF-based fragment shader. The shader
computes a signed distance field for rounded rectangles, producing smooth
anti-aliased edges, configurable border, and optional Gaussian-approximation
drop shadow — all in a single draw call per frame.

### Key Files Changed

- **`src/shaders/tooltip_bg.wgsl`** (new) — WGSL shader implementing SDF rounded
  rectangle with border, anti-aliasing, alpha-over compositing, and drop shadow.
- **`src/text/tooltip_bg.rs`** (new) — `TooltipBackgroundRenderer` with
  instanced rendering, buffer management, and orthographic projection.
- **`src/text/hover_reveal.rs`** — Extended `TooltipConfig` with
  `corner_radius`, `shadow_radius`, `shadow_color`, and `shadow_offset` fields.
- **`src/text.rs`** — Added `tooltip_bg` submodule.
- **`src/prelude.rs`** — Exported `TooltipBackgroundRenderer`.
- **`examples/hover_reveal_demo.rs`** — Updated to render tooltip background
  behind text with shadow enabled.
- **`tests/tooltip_bg_tests.rs`** (new) — 6 GPU integration tests.

### Test Counts

- **5 unit tests** in `text::tooltip_bg::tests` (Pod layout, projection, config)
- **6 integration tests** in `tests/tooltip_bg_tests.rs` (GPU creation, render
  pass, config variants, end-to-end flow)
- All 23 existing `hover_reveal` unit tests continue to pass
- All 7 existing `hover_reveal` integration tests continue to pass

### Design Decisions

- **Dedicated SDF shader** instead of reusing the mark system's Rectangle. The
  mark system is designed for data visualization marks with instance buffers,
  shader function pipelines, and selection mechanics. A tooltip background is a
  simple UI element — a single rounded rectangle per frame. A lightweight
  dedicated shader avoids mark system overhead and gives full control over
  corner radius, border, and shadow.
- **Instanced rendering with unit quad** — a single TriangleStrip quad (4
  vertices) is reused for all tooltip instances. Per-instance data (rect bounds,
  colors, parameters) is uploaded to a separate instance buffer.
- **SDF-based rounded rectangle** in the fragment shader provides smooth
  anti-aliased edges at any scale without tessellating corner geometry.
- **Optional drop shadow** uses a simple `smoothstep` falloff over the shadow
  radius for a Gaussian-like appearance.
- **Background renders before text** in the same render pass — the demo calls
  `tooltip_bg.render()` before `text_renderer.render_queued_text()` to ensure
  correct z-ordering.

---

**Story Created**: 2026-02-27  
**Story Completed**: 2025-07-18  
**Origin**: GUP-200 retrospective follow-up
