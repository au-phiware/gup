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

## Retrospective

**Completed**: 2025-07-24

### Key Technical Learnings

#### LineInstance GPU Struct

- **Challenge**: Line mark had no Rust-side `LineInstance` struct — the WGSL
  shader defined the layout but there was no corresponding bytemuck-compatible
  Rust struct for CPU-side instance construction.
- **Solution**: Created `LineInstance` (48 bytes) matching the WGSL layout
  exactly, including the `_padding: [f32; 2]` field to align to a 16-byte
  boundary.
- **Pattern**: When adding `MarkInstanceBuilder` to a mark type, always verify
  the Rust struct matches the WGSL struct layout byte-for-byte. Add a
  `size_of` assertion to catch mismatches.

#### AttrValue Limitations for Complex Types

- **Challenge**: `AttrValue` only supports `Float`, `Vec2`, and `Vec4` variants.
  BoxPlot has attributes like `orientation` (u32), `notched` (bool), and
  `outliers` (array) that cannot be set via `attr()`.
- **Solution**: Omitted `orientation`, `notched`, `notch_width`, and `outliers`
  from the builder — they use default values. Users requiring these can use the
  manual `prepare_render(mapper)` path or set them on `BoxPlotAttributes`
  directly.
- **Pattern**: `MarkInstanceBuilder` is best suited for the common numeric
  attributes (positions, sizes, colours). Complex or typed attributes may need
  a richer `AttrValue` enum or a different binding mechanism.

### Architectural Decisions

#### Alias Consistency Across Mark Types

- **Decision**: Used `position`/`center` as position aliases on BoxPlot (matching
  Circle and Rectangle), and `color`/`fill_color` as color aliases (matching
  Circle). Line uses `start`/`from` and `end`/`to` since it has two position
  endpoints rather than one center.
- **Reasoning**: Users switching between mark types should find familiar
  attribute names. The alias pattern (`"color"` → fill color) is consistent.
- **Trade-off**: More aliases means more match arms, but the cost is negligible.
- **Future**: If mark types grow further, a formal attribute name registry or
  derive macro could reduce boilerplate.

### Development Workflow Insights

- The implementation was straightforward because GUP-168 established clear
  patterns. Both impls followed the same structure: `default_instance()` from
  `Default` attrs, `build_instance()` iterating over name-value pairs.
- The existing GPU test pattern (headless context, attr binding, prepare, render,
  frame finish) copied cleanly for both Line and BoxPlot.
- `cargo test --lib mark::line` provided fast feedback loops during development
  without running the full 1588-test suite each time.

### Follow-up Stories

No new stories identified. The `AttrValue` enum limitation (no u32/bool/array
variants) is a known trade-off documented in GUP-168's retrospective.
