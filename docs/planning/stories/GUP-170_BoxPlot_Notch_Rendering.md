# GUP-170: BoxPlot Notch Rendering

**Status**: ✅ Complete (2025-07-19)

## Story Overview

**Title**: Notched Box Plot Rendering in SDF Shader **Epic**: Phase 1 Initiative
4 - Advanced Data Mapping **Priority**: Low **Story Points**: 2

## Context

GUP-166 implemented a unified BoxPlot mark with SDF-based rendering. The
`BoxPlotAttributes` struct already includes `notched: bool` and
`notch_width: f32` fields, but the fragment shader does not render notches. A
notched box plot shows a narrowing at the median to indicate the confidence
interval, which is a common statistical visualisation technique.

## User Story

**As a** data analyst **I want** to render notched box plots **So that** I can
visually compare medians with confidence interval indicators

## Acceptance Criteria

- [x] When `notched` is `true`, the box SDF narrows symmetrically at the median
      position by `notch_width` fraction of the box width
- [x] Notch shape is smooth (trapezoidal or curved)
- [x] Existing non-notched rendering is unaffected (notched defaults to false)
- [x] Demo or test exercises both notched and non-notched box plots

## Dependencies

- **Requires**: GUP-166 (Unified BoxPlot Mark Renderer) ✅

## Testing Strategy

- Unit test: verify `BoxPlotInstance` packs notch fields correctly
- GPU test: render notched box plot to headless texture without errors

## Definition of Done

- [x] All acceptance criteria met
- [x] All tests pass (`cargo test -- --test-threads=1`)
- [x] `mask all-fix` clean

## Implementation Summary

### What Was Implemented

Notched box plot rendering via the existing SDF-based fragment shader. When
`notched == true`, the box narrows symmetrically at the median using a linear
interpolation from full width at Q1/Q3 to `(1 - notch_width) * width` at the
median, producing a smooth trapezoidal notch shape.

### Key Files Changed

- **`src/mark/boxplot.rs`**: Added `notched: u32`, `notch_width: f32`, and
  `_pad_notch: [f32; 2]` fields to `BoxPlotInstance` (256 → 272 bytes). Updated
  `From<BoxPlotAttributes>`, `build_instance`, and generated shader strings.
- **`src/mark/shaders/boxplot.frag.wgsl`**: Added notch SDF logic — computes
  `effective_hw` that varies with position along the value axis.
- **`src/mark/shaders/boxplot.vert.wgsl`**: Updated WGSL struct to match.
- **`src/mark/shaders/boxplot_pattern.frag.wgsl`**: Same notch logic for
  pattern-enabled rendering.
- **`src/selection.rs`**: Added `gpu_render_notched_boxplot` GPU test.
- **`examples/boxplot_rendering_demo.rs`**: Updated demo to alternate notched
  and non-notched box plots.

### Test Counts

- 4 new unit tests (notch field packing and attribute builder)
- 1 new GPU integration test (notched + non-notched rendering)
- All 1589 existing tests continue to pass

---

_Identified during GUP-166 retrospective (2025-07-17)._

## Retrospective

**Completed**: 2025-07-19

### Key Technical Learnings

#### WGSL Storage Buffer Alignment for Struct Extensions

- **Challenge**: Adding two new fields (`notched: u32` and `notch_width: f32`)
  before the `outliers: array<vec4<f32>, 8>` array required careful alignment
  handling. WGSL arrays of `vec4<f32>` require 16-byte alignment, and adding
  8 bytes of data would misalign the array.
- **Solution**: Added explicit `_pad_notch: [f32; 2]` padding (8 bytes) on the
  Rust side to match the implicit padding WGSL inserts between `f32` fields
  and a 16-byte-aligned array. Struct size went from 256 → 272 bytes.
- **Pattern**: When extending GPU structs with fixed-size arrays, always check
  the alignment requirement of the next field. With `#[repr(C)]` in Rust, no
  automatic padding is inserted for WGSL alignment — it must be explicit.

#### SDF Notch Shape via Variable Half-Width

- **Challenge**: Making the box SDF notch-aware without fundamentally
  restructuring the existing rendering logic.
- **Solution**: Introduced `effective_hw` — a variable half-width that equals
  the full `hw` outside the box or when `notched == 0`, and linearly
  interpolates to `hw * (1 - notch_width)` at the median when inside the box
  with `notched == 1`. This required only replacing `hw` with `effective_hw`
  in two lines of the existing SDF check.
- **Pattern**: SDF shape modifications are cleanly expressible as parameterised
  half-width or radius functions, keeping the core SDF logic untouched.

### Architectural Decisions

#### Trapezoidal (Linear) Notch Shape

- **Decision**: Used linear interpolation for the notch (V-shaped / bowtie)
  rather than a curved or stepped shape.
- **Reasoning**: Linear interpolation is the simplest SDF modification, produces
  a visually clear narrowing, and matches the most common notched box plot
  convention (e.g. R's `notch = TRUE` in `boxplot()`).
- **Trade-off**: A curved notch (e.g. parabolic) might look slightly smoother
  but would add complexity. The current approach can be extended later if
  needed.
- **Future**: Could add a `notch_shape` enum (Linear, Curved) if users request
  alternative shapes.

#### Bool-as-u32 for GPU Transfer

- **Decision**: Stored `notched` as `u32` (0 or 1) in `BoxPlotInstance` rather
  than trying to use a `bool` type.
- **Reasoning**: WGSL has no `bool` in storage buffers. Using `u32` matches WGSL
  semantics directly and avoids bytemuck complications.
- **Trade-off**: Slightly less self-documenting than a Rust `bool` field, but
  the doc comment makes the intent clear.

### Development Workflow Insights

- The existing SDF architecture from GUP-166 was extremely well-suited for this
  extension. Adding notch support required touching only the `effective_hw`
  computation — the rest of the box SDF (stroke, median line, anti-aliasing)
  adapted automatically.
- Having three copies of the WGSL struct (vert, frag, pattern) plus two
  generated-shader copies in Rust is a maintenance burden. A shared include or
  code generation approach would reduce the risk of struct drift.
- The visual verification step (screenshot of the running demo) was valuable —
  it confirmed the notch shape, anti-aliased edges, and stroke behaviour were
  all correct at a glance.

### Follow-up Stories

No new follow-up stories identified. The existing GUP-171 (BoxPlot Pixel-Space
Stroke Widths) remains the natural next enhancement for the box plot subsystem.
