# GUP-254: OrdinalScale GPU Shader Function

## Story Overview

**Initiative**: Shader Function System **Status**: 🚧 In Progress **Created**:
2025-07-24

## Context

GUP-005 established the `ShaderFunction` trait and the composition machinery
that underpins all of Gup's GPU data transforms. GUP-052 refined the shader
pipeline builder into its production-ready form, and GUP-053 is expanding the
library of pre-built composable functions. GUP-252 (LinearScale GPU Shader
Function) establishes the pattern for scale `ShaderFunction` implementations — a
uniform struct carrying scale parameters, a WGSL function body, and a CPU-side
builder — that this story follows for the ordinal case.

Ordinal (categorical) scales are fundamentally different from continuous scales:
they map a finite, discrete set of categories to positions within a range rather
than interpolating across a domain. Bar charts, grouped comparisons, box plots,
and choropleth maps all depend on an ordinal scale to convert a category
identifier into a screen coordinate. Without a GPU-native ordinal scale, each
category's position must be computed on the CPU and uploaded per frame, breaking
Gup's GPU-first architecture and eliminating the performance advantage for
charts with large numbers of categories.

On the GPU, categories are represented as unsigned integer indices (the CPU is
responsible for maintaining the string-to-index mapping). Two variants of the
ordinal scale are needed: a **BandScale**, which divides the range into equal
bands and maps each index to the center of its band while exposing the bandwidth
for downstream sizing; and a **PointScale**, which distributes points evenly
across the range with configurable outer padding. Both variants share a common
uniform layout (`range_start`, `step_size`, `padding`) and differ only in how
the final position and, for BandScale, the bandwidth are derived.

GUP-245 (Bar Chart Builder) explicitly references GUP-254 as a prerequisite: the
`BarChartBuilder` delegates categorical x-axis position mapping to this scale's
GPU shader function. GUP-275 (Choropleth Chart Builder) similarly needs an
ordinal scale for region-to-position mapping. Delivering this story unblocks
both chart builders from having to fall back to CPU-side position computation.

## User Story

> "As a visualization developer, I want a composable `OrdinalScale`
> `ShaderFunction` that maps integer category indices to pixel positions on the
> GPU, so that bar charts, box plots, and categorical comparisons with large
> datasets render without CPU bottlenecks."

> "As a visualization developer, I want a CPU-side `OrdinalScale` builder that
> accepts a slice of category labels and produces the correct GPU uniforms and a
> `category_index()` lookup function, so that I can work with string category
> names in Rust while the GPU handles all positional arithmetic."

## Acceptance Criteria

### AC1: OrdinalScaleUniforms GPU Struct

- [ ] `OrdinalScaleUniforms` is a `#[repr(C)]` struct that derives
      `bytemuck::Pod`, `bytemuck::Zeroable`, and `Debug`
- [ ] The struct carries at minimum: `range_start: f32`, `step_size: f32`,
      `padding: f32`, and `category_count: u32`
- [ ] The struct is correctly sized and aligned for WGSL uniform binding
      (verified by a `bytemuck::cast_slice` round-trip test)

### AC2: BandScale Shader Function

- [ ] `BandScale` implements `ShaderFunction` with `Input = u32` (category
      index) and `Output = f32` (band centre position)
- [ ] The generated WGSL function maps index `i` to
      `range_start + (f32(i) + 0.5) * step_size * (1.0 - padding)` (or
      equivalent formulation that passes correctness tests)
- [ ] `BandScale::bandwidth()` returns `step_size * (1.0 - padding)` as a CPU
      `f32` value matching the GPU calculation
- [ ] A second WGSL helper function or output variant exposes the bandwidth as a
      `f32` uniform derivable from `OrdinalScaleUniforms` so that downstream
      sizing marks can consume it without additional CPU involvement
- [ ] Unit tests confirm correct positions for a three-category band scale over
      range `[0.0, 300.0]` with `padding = 0.1`

### AC3: PointScale Shader Function

- [ ] `PointScale` implements `ShaderFunction` with `Input = u32` and
      `Output = f32`
- [ ] The generated WGSL function maps index `i` to
      `range_start + f32(i) * step_size` with outer padding applied to
      `range_start` and `step_size` at construction time
- [ ] Unit tests confirm correct positions for a four-category point scale over
      range `[0.0, 400.0]` with `padding = 0.5` (matching D3's `scalePoint`
      behaviour for the same inputs)

### AC4: CPU-Side Category-to-Index Mapping

- [ ] `OrdinalScale::from_categories(labels: &[&str]) -> OrdinalScale` builds a
      scale from a string slice, assigning indices in the order provided
- [ ] `OrdinalScale::category_index(label: &str) -> Option<u32>` performs an
      O(1) lookup (hash map backed)
- [ ] `OrdinalScale::uniforms(range: (f32, f32), padding: f32) -> OrdinalScaleUniforms`
      produces the correctly computed uniform struct ready for GPU upload
- [ ] Attempting to look up a label not present in the original slice returns
      `None` and does not panic
- [ ] Round-trip test: `from_categories` → `category_index` → uniform → WGSL
      function produces positions equivalent to manual calculation for a
      five-category example

### AC5: ChartBuilder Integration

- [ ] `OrdinalScale` can be passed to a chart builder's `.x_scale()` method (or
      equivalent integration point established by GUP-252/GUP-245)
- [ ] At least one runnable example (`examples/ordinal_scale.rs` or similar)
      demonstrates constructing an `OrdinalScale` from string categories and
      composing it in a shader pipeline
- [ ] The example compiles without errors: `cargo check --examples`

### AC6: Composition Compatibility

- [ ] Both `BandScale` and `PointScale` compose with downstream `ShaderFunction`
      implementations (e.g., a color map or position transform) using the
      existing pipeline builder API from GUP-052
- [ ] No GPU validation layer errors are produced when the composed pipeline
      runs in the test harness

## Technical Tasks

- [ ] Define `OrdinalScaleUniforms` struct in the scale module (alongside or
      following the location established by GUP-252's `LinearScaleUniforms`)
- [ ] Implement `BandScale` struct with `ShaderFunction` impl; write WGSL
      function body using the uniform fields
- [ ] Implement `PointScale` struct with `ShaderFunction` impl; write WGSL
      function body
- [ ] Add `bandwidth()` helper on `BandScale` that mirrors the GPU formula
- [ ] Implement `OrdinalScale` CPU builder: -
      `from_categories(&[&str]) -> Self` -
      `category_index(&str) -> Option<u32>` -
      `band_scale(range, padding) -> BandScale` -
      `point_scale(range, padding) -> PointScale` -
      `uniforms(range, padding) -> OrdinalScaleUniforms`
- [ ] Write unit tests for `BandScale` position calculation against expected
      values for known inputs
- [ ] Write unit tests for `PointScale` position calculation against expected
      values
- [ ] Write unit test for `category_index` lookup including missing-label case
- [ ] Write unit test for `OrdinalScaleUniforms` bytemuck round-trip
- [ ] Write unit test for composition of `BandScale` with a mock downstream
      `ShaderFunction`
- [ ] Create `examples/ordinal_scale.rs` demonstrating CPU category mapping and
      GPU pipeline composition
- [ ] Verify `cargo check --examples` passes
- [ ] Update public API exports in `lib.rs` (or the scale module's `mod.rs`)

## Dependencies

### Prerequisite Stories

- GUP-005: ShaderFunction Trait ✅ — provides the `ShaderFunction` trait,
  uniform binding infrastructure, and composition mechanism that all scale
  functions build on
- GUP-052: Shader Pipeline Builder ✅ — provides the `ComposableShaderPipeline`
  builder used to compose `OrdinalScale` variants with downstream functions
- GUP-252: LinearScale GPU Shader Function 📋 — establishes the scale
  `ShaderFunction` pattern (uniform struct, WGSL body, CPU builder) that
  `OrdinalScale` follows; module layout and naming conventions are set here
- GUP-053: Advanced Shader Function Library 📋 — provides mathematical building
  blocks (clamp, etc.) that the WGSL bodies may rely on

### Enables Stories

- GUP-245: Bar Chart Builder — `BarChartBuilder` delegates categorical x-axis
  position computation to `OrdinalScale`; completing this story unblocks
  GPU-native bar positioning
- GUP-275: Choropleth Chart Builder — needs `OrdinalScale` for region-to-band
  mapping along categorical axes

## Testing Strategy

- **Unit tests**: Test `BandScale` and `PointScale` position calculations for
  known inputs (2-, 3-, and 5-category examples) against manually computed
  expected values; test `category_index` lookup with hits and misses; test
  `OrdinalScaleUniforms` for correct field values after `uniforms()` call; test
  bytemuck `Pod` round-trip
- **Integration tests**: Compose `BandScale` or `PointScale` with a downstream
  `ShaderFunction` via `ComposableShaderPipeline` and verify no WGSL compilation
  or GPU validation errors
- **Visual validation**: Run `examples/ordinal_scale.rs` and visually confirm
  that category labels map to evenly spaced bands or points across the
  configured pixel range
- **Performance**: Uniform upload and pipeline compilation should not regress
  relative to the `LinearScale` baseline established in GUP-252; no explicit new
  benchmark is required unless a regression is detected

## Success Metrics

- [ ] All unit tests pass with `cargo test -- --test-threads=1`
- [ ] `BandScale` and `PointScale` produce positions within floating-point
      epsilon of manually computed expected values for all test cases
- [ ] `category_index()` performs a hash-map lookup without linear scan
      (verified by implementation inspection)
- [ ] The ordinal scale example compiles and runs without GPU validation errors
- [ ] Both scale variants compose cleanly in the pipeline builder: no type
      errors, no WGSL compilation errors

## Risk Assessment

- **Low**: The mathematical formulas for band and point scales are
  well-established (D3's `scaleBand` and `scalePoint` provide the canonical
  reference). The primary risk is correctly mirroring the GPU arithmetic in both
  the WGSL function and the CPU `bandwidth()` helper. _Mitigation_: Drive the
  implementation with unit tests that cross-check CPU and GPU formulas against
  known reference values before writing the WGSL body.

- **Medium**: The `ShaderFunction` pattern established by GUP-252 may introduce
  API decisions (module location, trait bounds, uniform binding conventions)
  that this story must follow. If GUP-252 is still in progress when this story
  begins, there may be rework if conventions change. _Mitigation_: If GUP-252 is
  not yet complete, draft `OrdinalScaleUniforms` and the WGSL bodies in a
  feature branch and merge once the pattern is stable. Coordinate with GUP-252's
  implementer on naming conventions early.

- **Low**: The CPU `OrdinalScale` struct introduces a string-to-index hash map
  that must be kept in sync with the GPU uniform's `category_count`. An off-by-
  one in `step_size` calculation would produce misaligned bars. _Mitigation_:
  Test `uniforms()` output for correctness before the GPU integration test;
  confirm `category_count` equals the length of the input slice.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
