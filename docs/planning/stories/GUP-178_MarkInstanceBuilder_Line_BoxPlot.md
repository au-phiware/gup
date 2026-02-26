# GUP-178: MarkInstanceBuilder for Line and BoxPlot

**Status**: ✅ Complete (2025-07-24)

## Story Overview

**Title**: Extend MarkInstanceBuilder to Line and BoxPlot marks **Epic**: Phase
1 Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**: 3

## Context

GUP-168 implemented `MarkInstanceBuilder` for `Circle` and `Rectangle` marks,
enabling declarative attribute binding via `attr()` and
`prepare_render_bound()`. The `Line` and `BoxPlot` marks still require the
manual `prepare_render(mapper)` path.

This story extends `MarkInstanceBuilder` to the remaining mark types for
complete coverage.

## User Story

**As a** library user **I want** to use declarative `attr()` bindings with Line
and BoxPlot marks **So that** I have a consistent API across all mark types

## Acceptance Criteria

- [x] `MarkInstanceBuilder` implemented for `Line` mark
- [x] `MarkInstanceBuilder` implemented for `BoxPlot` mark
- [x] Attribute name aliases consistent with Circle and Rectangle
- [x] BoxPlot-specific attributes (min, q1, median, q3, max, etc.) supported
- [x] GPU integration tests for Line and BoxPlot with `prepare_render_bound()`
- [x] Documentation updated

## Dependencies

- **Requires**: GUP-168 (Selection Attribute Binding Pipeline) ✅
- **Requires**: GUP-067 (Rectangle and Line Mark Types) ✅
- **Requires**: GUP-166 (Unified BoxPlot Mark Renderer) ✅

## Testing Strategy

- Unit tests for Line and BoxPlot instance builders
- GPU integration tests for prepare_render_bound with both marks
- Test attribute aliases consistency

## Definition of Done

- [x] All acceptance criteria met
- [x] Existing tests still pass
- [x] `mask all-fix` clean

## Implementation Summary

### What Was Implemented

- **`LineInstance` GPU-ready struct** (48 bytes) — Matches the WGSL
  `LineInstance` layout in `line.vert.wgsl` with start, end, color, width,
  style, and padding fields.
- **`From<&LineAttributes>` and `From<LineAttributes>`** for `LineInstance` —
  Conversion from high-level attributes to GPU-ready data.
- **`MarkInstanceBuilder` for `Line`** — Supports attributes: `start`/`from`,
  `end`/`to`, `color`/`stroke_color`, `width`/`stroke_width`/`size`.
- **`MarkInstanceBuilder` for `BoxPlot`** — Supports attributes:
  `position`/`center`, `min`/`whisker_min`, `q1`, `median`, `q3`,
  `max`/`whisker_max`, `width`, `box_fill_color`/`fill_color`/`color`,
  `box_stroke_color`, `median_color`, `whisker_color`, `outlier_color`,
  `stroke_width`, `outlier_radius`.
- **GPU integration tests** — `gpu_prepare_render_bound_line` and
  `gpu_prepare_render_bound_boxplot` in `selection.rs`.

### Key Files Changed

| File                      | Change                                         |
| ------------------------- | ---------------------------------------------- |
| `src/mark/line.rs`        | LineInstance struct + MarkInstanceBuilder + 7 tests |
| `src/mark/boxplot.rs`     | MarkInstanceBuilder + 5 tests                  |
| `src/selection.rs`        | 2 GPU integration tests + updated imports      |
| `src/lib.rs`              | Export LineInstance                             |

### Test Counts

- 7 new Line unit tests (instance struct, builder, aliases)
- 5 new BoxPlot unit tests (builder, colors, aliases)
- 2 new GPU integration tests (Line + BoxPlot prepare_render_bound)
- All 1588 existing tests continue to pass
