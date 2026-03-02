# GUP-253: LogScale GPU Shader Function

## Story Overview

**Initiative**: Shader Function System **Status**: 🚧 In Progress **Created**:
2025-07-22

## Context

Logarithmic scales are indispensable for visualizing data that spans multiple
orders of magnitude — stock prices, population figures, earthquake magnitudes,
scientific measurements. Without a log scale, a data set ranging from 1 to
1,000,000 renders almost all values compressed against one axis boundary, making
meaningful patterns invisible. A GPU-side log scale removes the need for any CPU
pre-processing: the raw domain values are forwarded to the GPU and mapped in the
shader.

GUP-053 (Advanced Shader Function Library) establishes the `LogarithmicScale`
low-level WGSL transform: a raw `f32 → f32` function that applies a
configurable-base logarithm. GUP-252 (LinearScale GPU Shader Function) pins the
canonical pattern for a full _scale_ — `LogScaleUniforms`, a complete domain →
range mapping, `ChartBuilder` axis integration, and the composability contract.
This story follows that pattern to deliver `LogScale`, a first-class
`ShaderFunction` that maps values from a logarithmic domain
`[domain_min, domain_max]` to a linear output range `[range_min, range_max]` on
the GPU.

Particular care is needed around numerical correctness: `log(0)` is undefined,
negative values are outside the natural domain, and choosing the right WGSL
built-in (`log`, `log2`) requires matching the configurable base. Symmetric-log
("log-sign") support allows negative values to be visualized by applying the
logarithm to the absolute value and restoring the sign — a common pattern when
data straddles zero (e.g. profit-and-loss figures).

## User Story

> "As a visualization developer, I want a composable GPU `LogScale` shader
> function so that I can map data spanning multiple orders of magnitude to pixel
> coordinates or colour ranges in a single GPU pass, without pre-processing
> values on the CPU."

## Acceptance Criteria

### AC1: LogScaleUniforms Struct

- [ ] `LogScaleUniforms` is a `bytemuck::Pod + bytemuck::Zeroable` struct with
      fields `domain_min: f32`, `domain_max: f32`, `range_min: f32`,
      `range_max: f32`, `base: f32` (default `10.0`), and `symmetric: u32`
      (boolean flag, `0` = off, `1` = symmetric-log)
- [ ] A unit test confirms the struct is 24 bytes and 4-byte aligned, matching
      WGSL `std140`/`std430` layout expectations

### AC2: WGSL Implementation — Standard Log Scale

- [ ] The generated WGSL function
      `log_scale(value: f32, uniforms:     LogScaleUniforms) -> f32` maps
      `domain_min` → `range_min` and `domain_max` → `range_max` via logarithmic
      interpolation in the domain
- [ ] Base conversion is handled correctly:
      `log_base(x, b) = log2(x) /     log2(b)`, using WGSL's built-in `log2`
- [ ] A unit test (pure Rust, no GPU) verifies that `log_scale(100.0)` with
      `domain=[1, 1000]`, `range=[0, 1]`, `base=10` returns approximately
      `0.667` (≙ `log10(100) / log10(1000)`)
- [ ] A unit test verifies that `log_scale(domain_min)` returns `range_min` and
      `log_scale(domain_max)` returns `range_max`

### AC3: Zero and Sub-Epsilon Guard

- [ ] Values ≤ 0 in a standard (non-symmetric) log scale are clamped to a small
      epsilon (`1e-10`) before the logarithm is taken, preventing `log(0) = -∞`
      from propagating to NaN or infinity in downstream functions
- [ ] A unit test verifies that `log_scale(0.0)` and `log_scale(-1.0)` return
      `range_min` (the clamped boundary value) rather than NaN or ±infinity

### AC4: Symmetric-Log (Log-Sign) Mode

- [ ] When `symmetric = 1`, the WGSL function maps negative input `x` as
      `-log_base(|x| + 1, base)`, zero as `0.0`, and positive `x` as
      `log_base(x + 1, base)`, preserving sign symmetry around zero
- [ ] A unit test confirms that `log_scale(-v)` = `-log_scale(v)` when
      `symmetric = 1` and the domain is centred on zero
- [ ] A unit test verifies that `log_scale(0.0)` returns exactly `0.0` in
      symmetric mode

### AC5: Rust Builder API

- [ ] `LogScale::new(base: f32) -> LogScale` constructs a scale with the given
      base and sensible defaults (`domain=[1, 10]`, `range=[0, 1]`,
      `symmetric=false`)
- [ ] Builder methods `.domain(min: f32, max: f32)`,
      `.range(min: f32, max:     f32)`, `.symmetric(bool)` are present and
      return `Self` for chaining
- [ ] `LogScale` implements `ShaderFunction<Input = f32, Output = f32>` and
      `create_uniforms()` returns a populated `LogScaleUniforms`

### AC6: ChartBuilder Axis Integration

- [ ] `ChartBuilder` accepts `.y_scale(LogScale::new(10.0))` and
      `.x_scale(LogScale::new(10.0))` (or equivalent axis configuration API
      consistent with GUP-252's pattern)
- [ ] An integration test or example demonstrates a chart rendered with a log
      Y-axis mapping data values `[1, 10, 100, 1000]` to correct pixel positions

### AC7: Composition with ColorScale

- [ ] `LogScale` composes with a downstream `f32 → vec4<f32>` `ColorScale` (or
      the `HslToRgb`-based chain from GUP-053) using the existing
      `ShaderPipeline` builder without type errors
- [ ] A unit or integration test exercises the `LogScale → ColorScale` chain and
      confirms the GPU validation layer reports no errors

## Technical Tasks

- [ ] Create `src/scale/log_scale.rs` (or extend the scales module established
      by GUP-252) with:
  - `LogScaleUniforms` struct with `bytemuck` derives
  - `LogScale` Rust builder struct with `new`, `domain`, `range`, `symmetric`
    methods
  - `ShaderFunction` impl for `LogScale`
  - WGSL snippet string (standard and symmetric branches, epsilon guard,
    base-conversion helper)
- [ ] Register `LogScale` and `LogScaleUniforms` in `src/scale/mod.rs` and in
      the crate prelude (consistent with GUP-252's registration pattern)
- [ ] Write pure-Rust unit tests covering:
  - Uniform struct size and alignment (AC1)
  - Boundary value correctness (AC2)
  - Epsilon/zero guard (AC3)
  - Symmetric-log sign symmetry and zero mapping (AC4)
  - Builder API chaining (AC5)
- [ ] Write an integration test (GPU test harness) for the
      `LogScale →     ColorScale` composition chain (AC7)
- [ ] Add or extend a `log_scale_example` (or adapt an existing scale example
      from GUP-252) that renders a scatter plot or colour gradient using the log
      scale, confirming visual correctness for data spanning 3+ orders of
      magnitude
- [ ] Update `ChartBuilder` to accept `LogScale` via the axis scale API
      introduced in GUP-252 (AC6)

## Dependencies

### Prerequisite Stories

- GUP-005: ShaderFunction Trait ✅ — provides the `ShaderFunction` trait,
  `ShaderType` bounds, and uniform management that `LogScale` implements
- GUP-053: Advanced Shader Function Library 📋 — delivers `LogarithmicScale` as
  a low-level `f32 → f32` transform and `ClampFunction` that the log scale uses
  internally; also provides the `HslToRgb` chain needed for the AC7 composition
  test
- GUP-252: LinearScale GPU Shader Function 📋 — establishes the scale module
  layout, `ChartBuilder` axis API, and the exact pattern (`Uniforms` struct,
  builder, `ShaderFunction` impl) that this story follows

### Enables Stories

- GUP-254: OrdinalScale GPU Shader Function — may reuse the scale module
  structure and `ChartBuilder` integration pattern set by this story
- GUP-255: ColorScale GPU Shader Function — directly composes with `LogScale` to
  produce log-scaled colour mappings; the AC7 test here validates that path

## Testing Strategy

- **Unit tests**: Pure-Rust tests (no GPU required) for `LogScaleUniforms`
  layout, boundary values, zero/epsilon guard, symmetric-log sign symmetry, and
  builder API chaining. Located in `src/scale/log_scale.rs` under
  `#[cfg(test)]`.
- **Integration tests**: Use the existing GPU test harness to run a
  `LogScale → ColorScale` composed pipeline; assert no GPU validation errors and
  that a round-trip through the shader produces expected `range_min`/`range_max`
  boundary outputs.
- **Visual validation**: Run the log-scale example and inspect the rendered
  output to confirm that equal-ratio data intervals (e.g. ×10 steps) produce
  equal pixel intervals on the log axis.
- **Compile-fail tests**: Confirm that composing `LogScale` with a
  `vec2<f32>`-input function fails to compile (type mismatch caught at compile
  time by the existing `ShaderFunction` type system).

## Success Metrics

- [ ] All unit tests pass: `cargo test -- --test-threads=1`
- [ ] `log_scale(0.0)` never produces NaN or infinity in any test case
- [ ] The GPU integration test for `LogScale → ColorScale` completes without GPU
      validation errors
- [ ] The log-scale example compiles and renders without panics:
      `cargo check --examples`
- [ ] A data set spanning three orders of magnitude (1 → 1000) visually maps
      equal-ratio increments to equal pixel distances in the rendered example

## Risk Assessment

- **Medium**: WGSL base-conversion arithmetic (`log2(x) / log2(base)`) can
  amplify floating-point rounding errors for bases close to 1 or for very large
  domain extents. _Mitigation_: Use `log2` (native WGSL built-in) throughout and
  validate edge cases (base = 2, base = 10, base = ℯ ≈ 2.718) in unit tests.

- **Medium**: The symmetric-log branch adds conditional logic to the shader.
  WGSL does not short-circuit branches on non-uniform values, so the `select`
  built-in should be preferred over `if`/`else` for sub-group divergence
  avoidance. _Mitigation_: Use `select(neg_branch, pos_branch, value >= 0.0)` in
  the WGSL snippet; note this in a code comment.

- **Low**: If GUP-252 (LinearScale) is not yet complete when work on this story
  begins, the `ChartBuilder` axis API and scale module layout may not exist.
  _Mitigation_: Track GUP-252's status; this story is blocked until GUP-252 is
  merged. The WGSL implementation and unit tests can be written in parallel
  while waiting on the axis API.

- **Low**: The `LogarithmicScale` provided by GUP-053 and the `LogScale` defined
  here overlap in purpose. Care should be taken to avoid duplicating WGSL
  snippets: `LogScale` should delegate its core log computation to the GUP-053
  WGSL helper or replicate it with an explicit attribution comment, not silently
  diverge. _Mitigation_: Review GUP-053's WGSL output during implementation and
  decide (and document) whether to import, delegate, or inline.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
