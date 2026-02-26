# GUP-205: SDF Text Rendering Performance Tuning

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Text Rendering
**Priority**: Low **Story Points**: 3 **Status**: ✅ Complete

## Overview

Tune the SDF (Signed Distance Field) shader parameters for optimal
quality/performance balance at different text sizes used by axis labels. The
existing TextRenderer has excellent batching (single draw call) but the SDF
rendering parameters haven't been profiled against the range of font sizes
typically used for axis tick labels, axis titles, and chart annotations.

## Context

GUP-094 identified that while the TextRenderer batching is efficient, the SDF
shader parameters (smoothing threshold, edge softness, sub-pixel sampling)
haven't been tuned for the specific text sizes used by the axis system. Small
labels may appear blurry or aliased without proper SDF parameter adjustment.

## User Story

> "As a chart developer, I want axis labels to render crisply at all sizes so
> that my visualizations look professional regardless of chart dimensions."

## Acceptance Criteria

- [x] SDF parameters profiled at 8px, 12px, 16px, 24px, and 32px text sizes
- [x] Optimal smoothing values documented for each size range
- [x] Auto-tuning logic selects appropriate SDF params based on text size
- [x] No visual regression for existing text rendering
- [x] Performance impact of tuning measured (should be negligible)

## Dependencies

- **GUP-099**: GPU Text Rendering Pipeline ✅
- **GUP-108**: Correct SDF Font Atlas Generation ✅
- **GUP-094**: Axis Performance Optimization ✅

## Testing Strategy

- Visual comparison: text at various sizes with tuned vs default parameters
- Performance benchmark: measure rendering time with auto-tuning enabled
- Regression: existing text tests continue to pass

## Definition of Done

- [x] SDF parameters tuned and documented
- [x] Auto-tuning logic implemented
- [x] Visual quality maintained or improved
- [x] Performance unchanged or improved

## Implementation Summary

### What was implemented

1. **`SdfTuningParams` struct** (`src/text/sdf_tuning.rs`): Encapsulates
   `edge_threshold` and `smoothing_factor` per font size.
2. **`SdfTuningProfile`**: Breakpoint-based tuning profiles with piecewise
   linear interpolation for smooth transitions across all sizes.
3. **Default tuning profile** with profiled values at 8, 12, 16, 24, 32 px:
   - 8px: `edge_threshold=-0.06`, `smoothing_factor=1.0` (bold compensation)
   - 12px: `edge_threshold=-0.03`, `smoothing_factor=1.2`
   - 16px: `edge_threshold=0.0`, `smoothing_factor=1.5` (matches legacy default)
   - 24px: `edge_threshold=0.0`, `smoothing_factor=1.4`
   - 32px: `edge_threshold=0.0`, `smoothing_factor=1.2` (crisp edges)
4. **Updated WGSL shader** (`src/shaders/text.wgsl`): reads smoothing_factor
   from `sdf_params[2]`, packs combination_mode with debug_mode in
   `sdf_params[3]`. Backward compatible — `sdf_params[2]=0.0` falls back to 1.5.
5. **Wired auto-tuning** into `TextRenderer::create_vertices`: font_size flows
   from `TextStyle` → `create_vertices` → per-vertex SDF params.

### Key files changed

- `src/text/sdf_tuning.rs` (new, 262 lines) — SDF tuning module
- `src/text.rs` — Module registration, doc updates
- `src/shaders/text.wgsl` — Configurable smoothing_factor, packed params
- `src/text/renderer.rs` — Auto-tuning wired into vertex generation

### Tests

- 12 unit tests in `sdf_tuning::tests` covering interpolation, clamping,
  backward compatibility, monotonicity, and custom profiles
- All 1656 existing tests pass (3 pre-existing GPU bind-group failures unrelated)
- Examples compile and run without errors
