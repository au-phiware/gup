# GUP-252: LinearScale GPU Shader Function

## Story Overview

**Initiative**: Shader Function System **Status**: 🚧 In Progress **Created**:
2025-07-23

## Context

The implementation strategy treats scales as first-class composable shader
functions: data transformations that run on the GPU rather than being applied
CPU-side before upload. GUP-005 established the `ShaderFunction` /
`ComposableShaderFunction` trait and GUP-007 built the pipeline builder that
executes composed functions. A skeletal `LinearScale` struct currently lives in
`src/shader_function.rs` and covers the basic interpolation formula, but it is
explicitly marked as "a basic example" and is missing several properties
required for production use.

Specifically, the current implementation lacks a clamping option (values outside
`[domain_min, domain_max]` are extrapolated indefinitely), has no inversion
support (the GPU cannot map output coordinates back to data values), and the
`LinearScaleUniforms` struct carries no `clamp` field. There is also a name
conflict: `tick_generator::LinearScale` (CPU-side tick generation) must be
aliased as `TickLinearScale` in `lib.rs` to avoid colliding with the shader
function version, and a redundant `LinearScaleTemplate` produced by the
`wgsl_function!` macro duplicates the same WGSL. Neither the `ChartBuilder` API
nor any benchmark exercises the shader-function `LinearScale` directly.

GUP-053 lays the groundwork for the broader advanced function library — the
composable infrastructure that scale functions will slot into. This story
delivers a complete, production-quality `LinearScale` that becomes the reference
implementation for GUP-253 (LogScale), GUP-254 (OrdinalScale), and GUP-255
(ColorScale), which follow the same pattern.

## User Story

> "As a visualization developer, I want a `LinearScale` shader function that
> maps a data domain to an output range on the GPU — with clamping and inversion
> options — so that I can bind it to a `Selection` attribute or axis without
> writing raw WGSL or performing data normalization on the CPU."

## Acceptance Criteria

### AC1: Production-ready `LinearScaleUniforms` struct

- [ ] `LinearScaleUniforms` is `#[repr(C)]`, `bytemuck::Pod`, and
      `bytemuck::Zeroable`
- [ ] Fields: `domain_min: f32`, `domain_max: f32`, `range_min: f32`,
      `range_max: f32`, `clamp: u32` (0 = unclamped, 1 = clamped; `u32` used for
      alignment)
- [ ] `ShaderUniform` impl generates a WGSL struct definition matching the Rust
      layout exactly (verified by a unit test that round-trips the struct
      through `bytemuck::bytes_of` and checks field offsets)
- [ ] The existing four-field struct (without `clamp`) is removed or replaced;
      `LinearScaleTemplate` and its associated `wgsl_function!` block are
      removed to eliminate duplication

### AC2: Correct WGSL code generation

- [ ] `ComposableShaderFunction::wgsl_function()` returns a WGSL snippet
      containing both `linear_scale` (forward) and `linear_scale_invert`
      (reverse) functions
- [ ] `linear_scale` normalises the input to `[0, 1]` relative to the domain,
      then maps to the range; when `uniforms.clamp == 1u` the normalised value
      is clamped to `[0, 1]` before range expansion
- [ ] `linear_scale_invert` performs the mathematical inverse (maps output range
      back to input domain), respecting the same clamping flag
- [ ] Generated WGSL compiles without errors under `naga` validation (existing
      `ShaderPipeline` validation path counts)
- [ ] Unit tests verify correct output for: in-range value, below-domain value
      (unclamped extrapolation), below-domain value (clamped to `range_min`),
      above-domain value (clamped to `range_max`), and identity mapping
      (`domain == range`)

### AC3: Rust builder API

- [ ] `LinearScale::new(domain_min, domain_max, range_min, range_max) -> Self`
      constructs an unclamped scale (existing signature preserved for
      compatibility)
- [ ] `LinearScale::with_clamp(domain_min, domain_max, range_min, range_max) -> Self`
      constructs a clamped scale
- [ ] `LinearScale::invert() -> LinearScaleInvert` returns a companion type that
      implements `ComposableShaderFunction` with `Input = f32`, `Output = f32`
      and delegates to `linear_scale_invert` in WGSL
- [ ] Both `LinearScale` and `LinearScaleInvert` implement
      `ComposableShaderFunction` and compose correctly through the pipeline
      builder (verified by an integration test that chains `LinearScale` →
      `LinearScaleInvert` and confirms round-trip identity within floating-point
      tolerance)

### AC4: ChartBuilder axis integration

- [ ] `ChartBuilder` (and concrete builders such as `ScatterPlotBuilder`,
      `LineChartBuilder`) expose an `x_scale` / `y_scale` method accepting a
      `LinearScale`
- [ ] When `x_scale` / `y_scale` is set, the builder uses the scale's domain to
      auto-configure axis tick generation (delegating to the existing
      `IntegratedLinearScale` / `TickScale` machinery)
- [ ] An integration test builds a `ScatterPlotBuilder` with an explicit
      `LinearScale`, verifies that ticks are generated from the provided domain,
      and that the resulting `ShaderPipeline` contains a `linear_scale` function
      call

### AC5: Benchmark coverage

- [ ] A Criterion benchmark exists at `benches/` that measures throughput for
      composing `LinearScale` into a pipeline (1 000 compositions)
- [ ] The benchmark result for scale composition is ≤ 1 % of the typical
      per-frame render budget (target: < 100 µs for 1 000 compositions,
      consistent with the existing GUP-005 composition benchmark)
- [ ] Results are captured in the story retrospective

## Technical Tasks

- [ ] Add `clamp: u32` to `LinearScaleUniforms` in `src/shader_function.rs`;
      update `ShaderUniform::wgsl_struct_definition()` to include the field
- [ ] Remove `LinearScaleTemplate` and its `wgsl_function!` block to eliminate
      the duplicate
- [ ] Rewrite `ComposableShaderFunction::wgsl_function()` for `LinearScale` to
      include clamping logic and the `linear_scale_invert` companion function
- [ ] Add `LinearScale::with_clamp` constructor and update `create_uniforms` to
      propagate the `clamp` flag
- [ ] Implement `LinearScaleInvert` struct + `ComposableShaderFunction` impl
      that delegates to `linear_scale_invert`
- [ ] Add `LinearScale::invert() -> LinearScaleInvert` method
- [ ] Write unit tests in `src/shader_function.rs` for `LinearScaleUniforms`
      layout, WGSL output correctness, and round-trip inversion
- [ ] Extend `ChartBuilder` trait (or base struct) with `x_scale(LinearScale)`
      and `y_scale(LinearScale)` methods; wire through to axis tick
      configuration
- [ ] Write integration test in `tests/` that exercises ChartBuilder +
      LinearScale axis wiring
- [ ] Add Criterion benchmark `benches/linear_scale_composition.rs`
- [ ] Confirm the `TickLinearScale` alias in `lib.rs` remains valid; add a
      comment explaining the distinction between the shader-function
      `LinearScale` and the CPU-side `tick_generator::LinearScale`
- [ ] Update public documentation (`///` doc comments) on `LinearScale`,
      `LinearScaleInvert`, and `LinearScaleUniforms`
- [ ] Run `mask all-fix` and `cargo test -- --test-threads=1` to confirm clean
      build

## Dependencies

### Prerequisite Stories

- GUP-005: Shader Function Trait ✅ — provides `ComposableShaderFunction`,
  `ShaderUniform`, and `ShaderType` traits that `LinearScale` implements
- GUP-007: Shader Pipeline Builder ✅ — provides the `ShaderPipeline` and
  composition validation that `LinearScale` is wired into
- GUP-053: Advanced Shader Function Library 📋 — establishes the composable
  infrastructure conventions (function organisation, naming, test patterns) that
  this story follows

### Enables Stories

- GUP-253: LogScale GPU Shader Function — reuses the same uniform/builder/test
  pattern introduced here
- GUP-254: OrdinalScale GPU Shader Function — same pattern; depends on
  LinearScale for the per-band normalization step
- GUP-255: ColorScale GPU Shader Function — composes a `LinearScale` (domain →
  `[0,1]`) with a color-interpolation function

## Testing Strategy

- **Unit tests** (`src/shader_function.rs`):
  - Struct layout: `bytemuck::bytes_of(&LinearScaleUniforms { … })` confirms
    correct size (5 × `f32` = 20 bytes) and field order
  - WGSL string: `wgsl_function()` contains both `linear_scale` and
    `linear_scale_invert`
  - Numeric correctness: parameterised tests over in-range, below-domain
    (unclamped/clamped), above-domain (unclamped/clamped), and identity cases
  - Round-trip: `linear_scale_invert(linear_scale(x)) ≈ x` within `1e-5`

- **Integration tests** (`tests/`):
  - Chain `LinearScale` → `LinearScaleInvert` through `ShaderPipeline`; verify
    the generated WGSL is valid and the composition type-checks at runtime
  - `ScatterPlotBuilder` with `x_scale(LinearScale::new(0.0, 100.0, 0.0, 1.0))`
    produces axis ticks drawn from `[0, 100]`

- **Visual validation**: not required for this story (no new rendering path,
  only shader function generation and API)

- **Performance**: Criterion benchmark; target ≤ 100 µs for 1 000 `LinearScale`
  compositions

## Success Metrics

- [ ] All five acceptance criteria are satisfied and checked
- [ ] `LinearScaleUniforms` is exactly 20 bytes with no padding surprises
- [ ] Composition benchmark result documented: ≤ 100 µs / 1 000 compositions
- [ ] Zero duplicate `LinearScale*` WGSL definitions in the codebase after the
      `LinearScaleTemplate` removal

## Risk Assessment

- **Low**: The core WGSL formula is already present and correct in the existing
  implementation; this story extends rather than replaces it. _Mitigation_: Keep
  the existing `new()` constructor signature identical to avoid breaking the
  many call-sites in `selection.rs` tests.

- **Medium**: Adding `clamp: u32` to `LinearScaleUniforms` changes the struct
  size from 16 bytes to 20 bytes. Any GPU buffer that was sized to the old
  struct will need updating; the test suite will surface these breakages
  immediately. _Mitigation_: Do the struct change first, then let `cargo test`
  identify all affected sites before touching anything else.

- **Low**: The `ChartBuilder` integration touches multiple concrete builder
  types (`ScatterPlotBuilder`, `LineChartBuilder`, etc.). Scope may grow if the
  trait hierarchy requires changes at multiple layers. _Mitigation_: Introduce
  `x_scale` / `y_scale` on the base `ChartBuilder` struct (not the trait) and
  default to a passthrough `LinearScale` when not explicitly set, avoiding trait
  breakage.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
