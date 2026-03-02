# GUP-255: ColorScale GPU Shader Function

## Story Overview

**Initiative**: Shader Function System  
**Status**: 🚧 In Progress  
**Created**: 2026-03-02

## Context

Color scales are one of the most fundamental data-mapping primitives in
visualization: they translate a numeric value — temperature, density, count,
ratio — into a color. Every heatmap, choropleth, and value-encoded scatter plot
depends on this mapping. Without a first-class GPU-side `ColorScale`, chart
builders must perform the mapping on the CPU and upload pre-colored vertices,
breaking the composable shader-function model and creating unnecessary
round-trips between CPU and GPU.

GUP-134 (Storage Buffer ColorGradient) delivered the gradient evaluation
building block: a storage-buffer-backed WGSL function that performs binary
search over up to hundreds of color stops and interpolates between them.
However, `ColorGradientStorage` is a raw gradient evaluator — it maps a
normalised `[0, 1]` float to `vec4<f32>`. It is not yet a `ShaderFunction`, has
no domain normalisation, and cannot be composed in a pipeline with scale
functions like `LinearScale`.

GUP-252 (LinearScale GPU Shader Function) will provide a `LinearScale`
`ShaderFunction` (f32 → f32) that normalises an input value from an arbitrary
domain onto `[0, 1]` (or any target range). This story builds on both: it wraps
`ColorGradientStorage` as a proper `ShaderFunction` (f32 → vec4\<f32\>), exposes
built-in palettes (Viridis, Plasma, Inferno, Magma, RdBu, and others), adds
diverging-scale support with explicit midpoints, a discrete (quantize/quantile)
variant, and integration with the `ChartBuilder` `.color_scale()` API. The
natural composition `LinearScale.compose(ColorScale)` then gives chart builders
a complete, GPU-resident domain → color pipeline.

## User Story

> "As a visualization developer, I want a composable `ColorScale` shader
> function that maps numeric domain values to colors on the GPU, so that I can
> build value-encoded charts (heatmaps, choropleths, density plots) without
> performing color mapping on the CPU."

> "As a chart builder author, I want `.color_scale(ColorScale::viridis())`
> available on `ChartBuilder`, so that I can wire up color encoding with a
> single ergonomic call."

## Acceptance Criteria

### AC1: ColorScale as a ShaderFunction

- [ ] `ColorScale` implements `ShaderFunction` with `Input = f32` and
      `Output = vec4<f32>`
- [ ] `ColorScale::wgsl_function()` emits valid WGSL that evaluates a gradient
      lookup (using the binary-search approach from GUP-134) and returns a
      `vec4<f32>` colour
- [ ] `ColorScale::create_uniforms()` produces a `ColorScaleUniforms` struct
      carrying the normalised domain clamp range (`[domain_min, domain_max]`)
      and discrete-mode metadata
- [ ] Generated WGSL compiles without errors via `naga` validation (same gate
      used by existing shader function tests)
- [ ] The gradient colour data is provided through a storage buffer (following
      the GUP-134 pattern); `ColorScale` exposes `create_colors_buffer_data()`
      and `create_stops_buffer_data()` helpers

### AC2: Built-in Palettes

- [ ] `ColorScale::viridis()` — perceptually uniform, colorblind-friendly
- [ ] `ColorScale::plasma()` — bright, vibrant, perceptually uniform
- [ ] `ColorScale::inferno()` — dark-to-bright warm ramp
- [ ] `ColorScale::magma()` — dark-to-bright muted ramp
- [ ] `ColorScale::rd_bu()` — diverging red–white–blue palette
- [ ] Each palette constructor accepts a `domain: (f32, f32)` parameter
      specifying the input range
- [ ] All palette constructors are pure Rust (no GPU resources created at
      construction time), consistent with the GUP-134 "lightweight CPU struct"
      pattern

### AC3: Diverging Scale Support

- [ ] `ColorScale::diverging(palette, domain_min, midpoint, domain_max)` maps
      values below the midpoint to the first half of the gradient and values
      above to the second half
- [ ] The midpoint does not need to be the arithmetic mean of the domain; the
      WGSL function correctly handles asymmetric domains
- [ ] A unit test verifies that the midpoint value maps to the exact centre of
      the gradient (normalised 0.5)

### AC4: Discrete (Quantize) Variant

- [ ] `ColorScale::quantize(palette, domain, n_bins: u32)` divides the domain
      into `n_bins` equal-width buckets and snaps each input to the
      corresponding bucket colour
- [ ] The WGSL implementation uses integer truncation (no binary search) to
      select the colour bin for efficiency
- [ ] A unit test verifies correct bin assignment at boundary values

### AC5: Composition with LinearScale

- [ ] `LinearScale::new(domain_min, domain_max, 0.0, 1.0).compose(ColorScale)`
      produces a `FunctionChain` with `Input = f32` and `Output = vec4<f32>`
      that compiles as a combined WGSL function (relies on GUP-252 and the
      `FunctionChain` infrastructure from GUP-005)
- [ ] A compilation test confirms the composed chain type-checks and the
      generated WGSL passes `naga` validation
- [ ] The composed chain correctly normalises an out-of-domain value (clamped by
      the LinearScale) and returns a boundary palette colour

### AC6: ChartBuilder Integration

- [ ] `ChartBuilder` gains a `.color_scale(impl Into<ColorScale>)` method that
      stores the scale and wires it into the chart's shader pipeline
- [ ] `ColorScale::viridis()`, `ColorScale::plasma()`, and the other built-ins
      are accepted without additional conversion
- [ ] An example `examples/color_scale_heatmap.rs` compiles
      (`cargo check --examples`) and demonstrates end-to-end use of
      `.color_scale(ColorScale::viridis())` on a 2D dataset

## Technical Tasks

- [ ] Add `ColorScale` struct to `src/shader_function.rs` (or a new
      `src/color_scale.rs` module) with `domain_min`, `domain_max`, `scale_kind`
      (continuous / diverging / quantize), `midpoint: Option<f32>`, and
      `n_bins: Option<u32>` fields
- [ ] Define `ColorScaleUniforms` (`#[repr(C)]`, `Pod + Zeroable`) carrying the
      fields needed at GPU evaluation time
- [ ] Implement `ShaderFunction for ColorScale` — `wgsl_function()` returns the
      WGSL snippet; `create_uniforms()` serialises the CPU-side configuration
- [ ] Write the continuous-mode WGSL function body (delegate to the GUP-134
      binary-search gradient evaluator after domain-clamping and normalisation)
- [ ] Write the diverging-mode WGSL function body (piecewise normalisation
      around midpoint before gradient lookup)
- [ ] Write the quantize-mode WGSL function body (integer bin selection, no
      binary search)
- [ ] Add palette constructors (`viridis`, `plasma`, `inferno`, `magma`,
      `rd_bu`) reusing stop data already defined in GUP-134; add `magma` and
      `rd_bu` stop data if not yet present
- [ ] Implement `ColorScale::diverging()` and `ColorScale::quantize()`
      convenience constructors
- [ ] Add `compose` integration test: `LinearScale.compose(ColorScale)` type
      checks and WGSL validates (depends on GUP-252 being complete)
- [ ] Extend `ChartBuilder` with `.color_scale()` method; update the internal
      pipeline builder to bind the `ColorScale` uniforms and storage buffers at
      the correct bind-group slots
- [ ] Create `examples/color_scale_heatmap.rs` demonstrating viridis encoding on
      a synthetic 2D grid dataset
- [ ] Export `ColorScale` and `ColorScaleUniforms` from `src/prelude.rs`
- [ ] Update rustdoc on all public items with usage examples

## Dependencies

### Prerequisite Stories

- GUP-005: Shader Function Trait ✅ — provides `ShaderFunction` trait,
  `FunctionChain`, and `compose()` infrastructure
- GUP-134: Storage Buffer ColorGradient ✅ — provides `ColorGradientStorage`,
  the gradient evaluation WGSL, and preset palette stop data that `ColorScale`
  builds on
- GUP-252: LinearScale GPU Shader Function 📋 — required for the
  `LinearScale.compose(ColorScale)` composition test and the `.color_scale()`
  domain normalisation path

### Enables Stories

- GUP-248: Heatmap Chart Builder — color encoding via `ColorScale` is the
  primary visual channel in any heatmap
- GUP-250: Density Plot Builder — density values must be mapped to colours for
  the visual representation
- GUP-275: Choropleth Chart Builder — geographic value encoding relies on a
  `ColorScale` for region fill colours

## Testing Strategy

- **Unit tests**: verify `create_uniforms()` fields are correct for domain/range
  configurations; verify boundary-value colour outputs for each scale kind
  (continuous, diverging, quantize); verify midpoint maps to gradient centre
  (0.5) in diverging mode
- **WGSL validation**: call `naga`'s `parse_str` (or equivalent inline
  validation helper used elsewhere in the test suite) on the WGSL returned by
  `wgsl_function()` and on the composed `LinearScale → ColorScale` chain
- **Composition type test**: confirm
  `LinearScale::new(...).compose(ColorScale::viridis(...))` produces
  `FunctionChain<LinearScale, ColorScale>` — compile-time check that types align
- **Integration test**: construct a `ColorScale`, produce buffer data via
  `create_colors_buffer_data()` / `create_stops_buffer_data()`, and verify
  buffer lengths match the expected stop counts for each preset
- **Visual validation**: run `examples/color_scale_heatmap.rs` and confirm the
  gradient colours render without GPU validation errors (headless where CI
  supports it)

## Success Metrics

- [ ] All new tests pass: `cargo test -- --test-threads=1`
- [ ] `ColorScale::wgsl_function()` output passes `naga` validation for all
      three scale kinds (continuous, diverging, quantize)
- [ ] `LinearScale.compose(ColorScale)` compiles and its WGSL validates
- [ ] `examples/color_scale_heatmap.rs` compiles: `cargo check --examples`
- [ ] Lint and format clean: `mask all-fix && cargo clippy --all-targets`

## Risk Assessment

- **Medium**: The WGSL for the diverging and quantize variants must share a
  single `wgsl_function()` entry point but differ in logic. A flag in
  `ColorScaleUniforms` (or a separate `wgsl_function_name()`) may be needed to
  select the correct code path. _Mitigation_: emit a single WGSL function that
  branches on a `scale_kind: u32` uniform field — this keeps the
  `ShaderFunction` interface clean while supporting all variants from one
  bind-group slot.
- **Medium**: The WGSL binary-search code from GUP-134 is defined as a static
  string on `ColorGradientStorage`. `ColorScale` will need to either re-emit
  that helper or compose the two functions so the helper is not duplicated when
  both appear in the same shader pipeline. _Mitigation_: extract the
  gradient-lookup helper into a shared constant (e.g.,
  `COLOR_GRADIENT_WGSL_HELPER`) in `src/shader_function.rs` that both
  `ColorGradientStorage` and `ColorScale` reference.
- **Low**: `magma` and `rd_bu` palette stop data may not yet exist in the
  codebase (GUP-134 delivered viridis, plasma, inferno, cool*warm, rainbow,
  grayscale). \_Mitigation*: add the missing stop tables as additional constants
  alongside the GUP-134 presets; the task list already accounts for this.
- **Low**: GUP-252 (LinearScale) is 📋 Planned. The composition test in AC5
  cannot be written until GUP-252 is complete. _Mitigation_: all other
  acceptance criteria are independent of GUP-252 and can be implemented and
  reviewed first; AC5 and the related task are gated on GUP-252 and should be
  the last items completed.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
