# GUP-215: Chart Builder Multi-Font Integration

**Status**: ✅ Complete **Priority**: Medium **Complexity**: Low **Created**:
2025-08-21 **Completed**: 2025-07-22

## Overview

Update the chart builder layer (axes, titles, labels) to use `FontAtlasManager`
so that `TextStyle.font_family` works automatically when building charts,
without users managing font atlases manually.

## Context

GUP-202 added `FontAtlasManager` and multi-font rendering support to
`TextRenderer`, but the higher-level chart builders (from GUP-018, GUP-100,
GUP-092) still use a single `FontAtlas`. Users of the chart API cannot yet
benefit from multi-font rendering without dropping down to the low-level
`TextRenderer` API.

## User Story

As a developer using the chart builder API, I want to set `font_family` on axis
labels, titles, and annotations and have the correct fonts render automatically,
so I can create typographically rich charts without managing font atlases.

## Acceptance Criteria

- [x] Chart builders accept or internally create a `FontAtlasManager`
- [x] Axis label `TextStyle.font_family` is respected during chart rendering
- [x] Chart title `TextStyle.font_family` is respected during chart rendering
- [x] Existing chart examples continue to work without changes

## Technical Tasks

1. Integrate `FontAtlasManager` into the chart rendering pipeline
2. Update axis renderers to use `queue_text_with_fonts`
3. Ensure backward compatibility with existing single-font chart API

## Dependencies

- GUP-202 ✅ (Font-Aware Text Rendering Pipeline)
- GUP-100 ✅ (Visual Chart Axis Integration)

## Testing Strategy

- Integration tests with charts using multiple fonts
- Verify existing chart examples still compile and render

## Risk Assessment

- **Low**: Straightforward integration; the font manager API is designed to be a
  drop-in alongside existing `FontAtlas` usage.

## Definition of Done

- [x] Chart builder API supports multi-font rendering via `font_family`
- [x] All existing chart tests pass
- [x] Documentation updated with chart font customisation examples
- [x] At least one chart example uses multiple fonts

## Implementation Summary

### What was implemented

- **`ChartConfig` text style fields**: Added `label_style` and `title_style`
  (`TextStyle`) to `ChartConfig` with builder methods `with_label_style()`,
  `with_title_style()`, and `with_title()`.
- **`ComposedChart::queue_chart_text()`**: Queues axis labels and optional chart
  title through `FontAtlasManager` using `queue_text_with_fonts()` so that
  `TextStyle.font_family` is automatically resolved.
- **`ComposedChart::queue_chart_text_resolved()`**: Same as above but with label
  collision detection via `LabelPositioner`.
- **`ComposedChart::queue_title_text()`**: Private helper that positions the
  title centred at the top of the chart area.

### Key files changed

| File                                  | Change                                                   |
| ------------------------------------- | -------------------------------------------------------- |
| `src/chart_builder.rs`                | Added text style fields, queue methods, 11 tests         |
| `examples/multi_font_chart_demo.rs`   | New example: chart with DejaVu Serif title + Sans labels |
| `docs/text-rendering-architecture.md` | Added "Chart Builder Multi-Font Integration" section     |

### Tests

- 11 new tests (6 unit, 5 GPU integration) in `chart_builder::tests_multi_font`
- All 1435 existing lib tests pass with 0 failures

---

**Estimated Effort**: 3-5 days **Prerequisites**: GUP-202 ✅ **Blockers**: None

## Retrospective

**Completed**: 2025-07-22

### Key Technical Learnings

#### FontAtlasManager as the bridge between chart builders and text rendering

- **Challenge**: Chart builders generate axis geometry (vertices + labels) but
  don't own the text rendering pipeline. The text rendering happens externally
  in the example/app code, making it hard to inject font family resolution.
- **Solution**: Added `queue_chart_text()` and `queue_chart_text_resolved()`
  methods to `ComposedChart` that accept `TextRenderer` + `FontAtlasManager`
  references. These methods bridge the chart geometry layer and the text
  rendering layer, using `queue_text_with_fonts` under the hood.
- **Pattern**: The chart builder doesn't _own_ the text renderer — it borrows
  it. This keeps the architecture composable: the caller controls the render
  lifecycle while the chart builder controls the text content and styling.

#### TextStyle propagation from config to labels

- **Challenge**: `AxisLabel` has its own `anchor` field from axis geometry, but
  the font family and other style properties should come from `ChartConfig`.
- **Solution**: Clone the config's `label_style`, then override the anchor with
  the label-specific anchor. This preserves the user's font, color, and size
  choices while respecting the per-label anchor from the axis system.
- **Pattern**: Config-level styles as base, with per-element overrides applied
  on top.

### Architectural Decisions

#### External text renderer ownership

- **Decision**: `ComposedChart` borrows `TextRenderer`, `FontAtlasManager`, and
  `TextLayoutEngine` instead of owning them.
- **Reasoning**: Text rendering resources (GPU pipelines, atlases) should be
  shared across the application, not duplicated per chart. Owning them would
  force a specific lifecycle that may conflict with the app's render loop.
- **Trade-off**: The API requires passing more arguments to `queue_chart_text`.
- **Future**: If charts are made more self-contained (e.g., for a
  `Chart::render_to_texture()` API), the text renderer could be internalised
  behind an optional owned mode.

#### Title positioning

- **Decision**: Title is centred horizontally at `width / 2` and vertically at
  `top_margin / 2`, using `TextAnchor::TopCenter`.
- **Reasoning**: Simple and predictable for the common case. The top margin
  already reserves space for the title.
- **Trade-off**: Doesn't support arbitrary title positions or multi-line titles.
- **Future**: A dedicated `TitleConfig` struct could add alignment, offset, and
  multi-line support.

### Development Workflow Insights

- The headless `GupContext` + `begin_frame()` pattern made GPU integration tests
  straightforward — no window or surface needed.
- The two-phase text rendering API (queue before pass, render during pass) is
  clean but requires discipline: the example must call `begin_frame()`, queue
  all text, then create the render pass.
- Pre-existing doctest failures (GUP-207) are noisy but don't affect
  lib/integration test results.

### Follow-up Stories

1. **GUP-216: Chart Title Layout Configuration** — Add `TitleConfig` to support
   title alignment (left/center/right), vertical offset, subtitle, and
   multi-line titles.
2. **GUP-217: Per-Axis Label Style Override** — Allow individual axes to
   override the chart-level `label_style`, e.g., different font sizes or colors
   for the X-axis vs Y-axis.
