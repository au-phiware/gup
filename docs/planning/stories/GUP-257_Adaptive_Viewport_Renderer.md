# GUP-257: Adaptive Viewport Renderer

## Story Overview

**Initiative**: Advanced Scale **Status**: 🚧 In Progress **Created**: 2025-07-14

## Context

GUP-256 builds a Level-of-Detail pyramid that pre-computes downsampled
representations of a large point dataset at multiple resolutions. Having the
pyramid in memory is necessary but not sufficient: the renderer must decide,
each frame, which tier of that pyramid to draw and which portion of it is
actually on screen. Without this selection logic, the pyramid is just a static
data structure with no effect on frame rate or visual quality.

The current rendering path treats every dataset as a flat buffer and submits the
full contents to the GPU regardless of zoom level or screen coverage. This is
manageable for datasets up to a few million points (thanks to the GPU-side
frustum culling in GUP-077), but breaks down at billion-point scale where even a
single LOD tier may exceed GPU memory bandwidth for a 30 FPS budget.

GUP-076 (GPU Occlusion Culling) and GUP-077 (Compute Shader Instance Filtering)
established the culling infrastructure — hierarchical Z-buffer generation,
prefix- sum compaction, and indirect draw dispatch. GUP-257 builds on those
compute pipelines to add a viewport-driven tier-selection stage upstream of the
existing culling pass, so that only the spatially and visually relevant subset
of the chosen LOD tier is ever submitted.

Smooth visual transitions between LOD tiers are required to avoid the
perceptible "popping" that occurs when the renderer abruptly switches from one
resolution to another during a continuous zoom gesture.

## User Story

> "As a visualization developer, I want the renderer to automatically select the
> right level of detail and cull off-screen data based on the current viewport
> so that billion-point datasets render at 30+ FPS without manual tuning."

> "As a developer debugging LOD behaviour, I want an optional overlay showing
> the active LOD tier and visible point count per frame so that I can verify the
> selection heuristic is working correctly."

## Acceptance Criteria

### AC1: LOD Tier Selection Algorithm

- [ ] `AdaptiveRenderer` accepts a `LodPyramid` (from GUP-256) and a `Viewport`
      (zoom level, pan offset, screen resolution in pixels).
- [ ] Per-frame LOD tier is determined by a **pixels-per-data-point** heuristic:
      the tier whose downsampled density yields ≥ 1 pixel per visible point at
      the current zoom is selected; if multiple tiers qualify the coarsest is
      preferred.
- [ ] The selected tier index is exposed as a public field / accessor so callers
      and tests can inspect it without the debug overlay.
- [ ] Unit tests verify tier selection for at least: maximum zoom-in (finest
      tier), maximum zoom-out (coarsest tier), mid-range zoom, and a viewport
      that covers only a sub-region of the data extents.

### AC2: Viewport Frustum Culling at Selected LOD Level

- [ ] After tier selection, a compute shader culling pass (reusing the
      `ComputeInstanceFilter` infrastructure from GUP-077) discards points whose
      positions fall outside the current viewport frustum.
- [ ] The culling pass operates on the selected LOD tier's GPU buffer only; it
      does not read from other tiers.
- [ ] Indirect draw call is issued using the compacted index buffer produced by
      the culling pass — no readback to CPU.
- [ ] No GPU validation errors or pipeline errors occur during the culling pass.

### AC3: Smooth LOD Transitions (No Popping)

- [ ] LOD tier switches are blended over a configurable number of frames
      (default: 8 frames) using an alpha cross-fade between the outgoing and
      incoming tier.
- [ ] The transition is imperceptible at the default blend duration during a
      continuous mouse-wheel zoom gesture at 60 FPS.
- [ ] Setting `blend_frames = 0` disables blending (instant switch), which is
      useful for testing and for applications that prefer crisp behaviour.
- [ ] A unit test verifies that the blend alpha progresses correctly from 0.0 to
      1.0 across the configured frame count and clamps at 1.0 thereafter.

### AC4: Performance Target — 1 Billion Points at 30+ FPS

- [ ] A benchmark (`benches/adaptive_renderer.rs`) renders a synthetic
      1-billion- point dataset (or a statistically equivalent scaled proxy)
      through `AdaptiveRenderer` and records frame time.
- [ ] At maximum zoom-out (coarsest LOD tier covering the full dataset),
      sustained frame time is ≤ 33 ms on the CI reference GPU (or headless
      software renderer at proportionally scaled point counts).
- [ ] The benchmark is registered in `perf-thresholds.toml` with an appropriate
      regression threshold.

### AC5: Debug Overlay

- [ ] `AdaptiveRenderer::set_debug_overlay(enabled: bool)` toggles a heads-up
      display drawn as a GPU text/rect pass on top of the scene.
- [ ] The overlay shows: current LOD tier index (e.g. "LOD 3/6"), visible point
      count after culling, and total points in the selected tier.
- [ ] The overlay is hidden by default and has zero CPU/GPU overhead when
      disabled (no buffer writes, no additional draw calls).
- [ ] An example (`examples/adaptive_lod_debug.rs`) demonstrates the overlay
      alongside a zoomable scatter plot, with keyboard shortcut `D` toggling it
      on and off.

## Technical Tasks

- [ ] Define `AdaptiveRenderer` struct in `src/renderer/adaptive.rs`:
  - Fields: `lod_pyramid: LodPyramid`, `viewport: Viewport`,
    `selected_tier: usize`, `blend_state: LodBlendState`,
    `cull_pipeline: ComputeInstanceFilter` (GUP-077),
    `debug_overlay: Option<DebugOverlay>`.
- [ ] Implement
      `AdaptiveRenderer::select_tier(&self, viewport: &Viewport) -> usize` using
      the pixels-per-data-point heuristic. Keep the function pure (no GPU side
      effects) for testability.
- [ ] Implement `LodBlendState`: tracks `(from_tier, to_tier, progress: f32)`;
      advances `progress` by `1.0 / blend_frames` each call to
      `LodBlendState::tick()`; exposes `alpha()` for fragment shader uniform.
- [ ] Wire `AdaptiveRenderer::render(&mut self, encoder, viewport)`:
  1. Call `select_tier` to determine target tier.
  2. If tier changed, start a blend transition via `LodBlendState`.
  3. Run `ComputeInstanceFilter` culling pass on the selected tier's buffer.
  4. Issue indirect draw for the compacted buffer, setting the blend alpha
     uniform.
  5. If blending, also draw the outgoing tier with `1.0 - alpha`.
  6. Optionally render debug overlay.
- [ ] Extend the `Viewport` type (or create one if absent) with: `zoom: f32`,
      `pan: Vec2`, `screen_size: UVec2`, and a method
      `pixels_per_world_unit() -> f32`.
- [ ] Write unit tests in `src/renderer/adaptive.rs` for `select_tier` and
      `LodBlendState::tick`.
- [ ] Write integration test in `tests/adaptive_renderer_integration.rs` that
      constructs a small `LodPyramid` (synthetic data, no real GPU required via
      mock or headless wgpu) and exercises the full `render` call.
- [ ] Add benchmark `benches/adaptive_renderer.rs` with a scaled point count
      appropriate for headless CI and a scaling comment explaining the
      extrapolation to 1B points.
- [ ] Implement `DebugOverlay` in `src/renderer/debug_overlay.rs`: GPU text/rect
      pipeline, zero-cost when disabled, activated via
      `set_debug_overlay(true)`.
- [ ] Add `examples/adaptive_lod_debug.rs` with winit window, zoomable viewport
      controls, and `D` key toggle for the debug overlay.
- [ ] Register benchmark threshold in `perf-thresholds.toml`.
- [ ] Update `docs/mark-system/README.md` or a new `docs/LOD_SYSTEM.md` with a
      brief description of the adaptive renderer's role in the LOD pipeline.

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides `GupContext` (wgpu device/queue)
  used by all GPU pipelines
- GUP-076: GPU Occlusion Culling ✅ — provides hierarchical Z-buffer
  infrastructure reused for depth-aware LOD culling
- GUP-077: Compute Shader Instance Filtering ✅ — provides
  `ComputeInstanceFilter` (frustum culling, prefix-sum, compaction, indirect
  draw) that this story wires into the LOD selection stage
- GUP-256: Level-of-Detail Pyramid 📋 — provides the `LodPyramid` struct and its
  per-tier GPU buffers that `AdaptiveRenderer` selects from

### Enables Stories

- GUP-258: Streaming Data Manager for LOD — the streaming manager needs a live
  `AdaptiveRenderer` to know which tiers are currently in use and should be
  prioritised for streaming/eviction

## Testing Strategy

- **Unit tests**: `select_tier` is pure and fast; test the heuristic at boundary
  zoom values without GPU. `LodBlendState::tick` tested for correct alpha
  progression and clamping.
- **Integration tests**: Headless wgpu (or a mock pyramid) exercises the full
  render path including the compute culling pass; asserts no GPU validation
  errors and that the indirect draw buffer is non-empty for an in-frustum
  viewport.
- **Visual validation**: Run `examples/adaptive_lod_debug.rs`, zoom in/out,
  verify overlay numbers change and no visual popping occurs during zoom.
- **Performance**: `benches/adaptive_renderer.rs` measures frame time for
  increasing point counts; regression threshold set in `perf-thresholds.toml`.

## Success Metrics

- [ ] `select_tier` unit tests pass for all four specified viewport scenarios
- [ ] Integration test completes with zero GPU validation errors
- [ ] Benchmark frame time for the scaled proxy dataset meets the ≤ 33 ms target
- [ ] Debug overlay renders correctly and introduces zero overhead when disabled
- [ ] `cargo test -- --test-threads=1` passes, including the new integration
      test
- [ ] `cargo bench --bench adaptive_renderer` completes and is tracked by CI

## Risk Assessment

- **Medium**: The pixels-per-data-point heuristic may require tuning for
  non-uniform data distributions (e.g. highly clustered datasets where one
  screen region is much denser than the global average). _Mitigation_: Expose
  `heuristic_scale: f32` as a configurable multiplier so callers can adjust
  without recompiling; document the default and its rationale.

- **Medium**: Blending two LOD tiers simultaneously doubles the fragment
  workload during a transition, potentially exceeding the 33 ms budget on low-
  end GPUs during fast zoom gestures. _Mitigation_: Make `blend_frames`
  configurable and default it to 0 (instant) in benchmark runs; document the
  trade-off.

- **Low**: `ComputeInstanceFilter` (GUP-077) was designed for flat instance
  buffers; adapting it to operate on a selected tier's sub-buffer may require a
  small API extension. _Mitigation_: GUP-077 exposes buffer slices via wgpu
  `BufferSlice`; verify the compute shader dispatch uses the correct byte offset
  before committing to this approach.

- **Low**: The 1-billion-point performance target cannot be directly verified in
  CI due to GPU memory constraints. _Mitigation_: Use a statistically equivalent
  scaled proxy (e.g. 10M points) and document the linear extrapolation
  assumption; note if the assumption breaks due to memory bandwidth saturation.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
