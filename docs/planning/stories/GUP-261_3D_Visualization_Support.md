# GUP-261: 3D Visualization Support

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-01-27

## Context

Gup's implementation strategy explicitly calls for "3D visualization support
with lighting and materials" as a key advanced-scale feature. While all existing
marks operate in 2D clip-space coordinates, wgpu already exposes the full 3D
rendering stack — depth buffers, perspective transforms, and programmable vertex
and fragment shaders — so the infrastructure is in place and ready to be
surfaced through Gup's mark system.

The current render pipeline (GUP-004) creates a `wgpu::RenderPipeline` with
depth/stencil state set to `None` and all vertices carrying only `[f32; 2]`
position components. Extending this to 3D requires three co-ordinated additions:
a depth attachment attached to each render pass; a camera abstraction that
generates the view/projection matrices uploaded as a uniform; and a new family
of 3D mark types whose vertex types carry `[f32; 3]` positions plus surface
normals. GUP-131 already provides the `Vec3` and `Mat4` shader types needed to
express these constructs without boilerplate.

The motivating use-case is an interactive 3D scatter plot — the kind of
visualisation that appears routinely in scientific data exploration (genomics
PCA plots, molecular visualisation, geospatial point clouds). Once the depth
buffer and camera uniform are in place, any number of 3D mark types can be added
on top without revisiting the pipeline plumbing.

## User Story

> "As a visualization developer, I want to place marks in 3D space with
> perspective projection and Phong lighting so that I can build interactive 3D
> scatter plots and spatial data visualizations that run at GPU speed."

## Acceptance Criteria

### AC1: Depth Buffer Integration

- [ ] `RenderContext` creates a `wgpu::Texture` depth attachment
      (`Depth32Float`) matching the surface dimensions
- [ ] The depth attachment is re-created automatically when the surface is
      resized
- [ ] `RenderPassDescriptor` includes a `depth_stencil_attachment` that clears
      to `1.0` each frame
- [ ] 3D render pipelines are created with
      `depth_stencil: Some(DepthStencilState { depth_write_enabled: true, depth_compare: Less, … })`
- [ ] 2D mark pipelines remain unaffected (depth testing stays opt-in per
      pipeline)

### AC2: Camera Abstraction

- [ ] A `Camera` struct exists with:
  - `perspective(fov_y_radians, aspect, near, far) -> Camera`
  - `orthographic(left, right, bottom, top, near, far) -> Camera`
  - `look_at(eye: Vec3, center: Vec3, up: Vec3) -> Camera` (sets view matrix)
- [ ] `CameraUniform` is a `bytemuck::Pod + bytemuck::Zeroable` struct
      containing `view: Mat4`, `projection: Mat4`, and `model: Mat4`
- [ ] `Camera::to_uniform() -> CameraUniform` computes the combined matrices
- [ ] A `wgpu::Buffer` (uniform, size = `size_of::<CameraUniform>()`) can be
      created from a `Camera` and uploaded via `queue.write_buffer`
- [ ] The camera uniform is bound to `@group(0) @binding(0)` in all 3D shaders

### AC3: 3D Mark Types

- [ ] `Sphere3D` mark renders a billboard sprite with depth-correct radius; its
      `Vertex` type carries `position: [f32; 3]` and `radius: f32`
- [ ] `Box3D` mark renders an axis-aligned box; its `Vertex` type carries
      `center: [f32; 3]`, `half_extents: [f32; 3]`
- [ ] `Line3D` mark renders a 3D line segment between two `[f32; 3]` endpoints
- [ ] All three marks implement the existing `Mark` trait from GUP-009
- [ ] Instance counts of 100 K render without GPU validation errors on both
      native and headless test environments

### AC4: Phong Lighting Model

- [ ] A reusable WGSL function
      `phong_lighting(normal, view_dir, light_dir, material, light) -> vec4<f32>`
      is available in a shared shader module
- [ ] `Material` struct (Rust side) contains `albedo: [f32; 3]`, `ambient: f32`,
      `diffuse: f32`, `specular: f32`, `shininess: f32` and is `bytemuck::Pod`
- [ ] `LightUniform` struct contains `direction: [f32; 3]`, `_pad: f32`,
      `color: [f32; 3]`, `intensity: f32` and is `bytemuck::Pod`
- [ ] `Sphere3D` and `Box3D` fragment shaders call `phong_lighting` to compute
      final colour; `Line3D` is unlit (constant colour)
- [ ] Changing `Material` values produces a visually correct change in specular
      highlight position and intensity

### AC5: 3D Scatter Plot Example

- [ ] `examples/scatter_3d.rs` compiles and runs headlessly without panics
- [ ] The example renders ≥ 1 000 data points as `Sphere3D` marks positioned by
      three independent data dimensions mapped to `(x, y, z)`
- [ ] Camera orbits around the scene using a simple time-based rotation
      (demonstrates live `CameraUniform` updates each frame)
- [ ] The example uses `Phong` lighting with at least one directional light and
      a non-trivial material (i.e., visible specular highlight)

## Technical Tasks

- [ ] Add `DepthBuffer` helper to `src/render/depth.rs` that creates and owns
      the `wgpu::Texture` / `wgpu::TextureView` and exposes a `resize` method
- [ ] Integrate `DepthBuffer` into `GupContext`; wire it into the render pass
      builder so that 3D pipelines can request the depth attachment
- [ ] Add `src/camera.rs` with `Camera`, `CameraUniform`, and the projection /
      view-matrix helpers (use column-major `[[f32; 4]; 4]` representation
      compatible with `bytemuck`)
- [ ] Add `src/lighting.rs` with `Material` and `LightUniform`
- [ ] Add shared WGSL include `src/shaders/phong.wgsl` containing
      `phong_lighting` and `blinn_phong_lighting` functions
- [ ] Add `src/marks/sphere3d.rs` implementing `Sphere3D` mark with billboard
      vertex shader and SDF-based depth reconstruction in the fragment shader
- [ ] Add `src/marks/box3d.rs` implementing `Box3D` mark with six-face geometry
      generated in the vertex shader via instance index
- [ ] Add `src/marks/line3d.rs` implementing `Line3D` mark as a camera-facing
      quad between two 3D endpoints
- [ ] Register new mark types in the mark registry
- [ ] Add `examples/scatter_3d.rs` demonstrating orbit camera + Phong lighting
- [ ] Write unit tests for `Camera` matrix construction
      (orthographic/perspective round-trips, `look_at` basis vectors)
- [ ] Write integration test asserting a `Sphere3D` selection of 1 000 points
      renders without GPU validation errors
- [ ] Update `docs/mark-system/README.md` to reference the new 3D mark types

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides `GupContext`, `wgpu::Device`, and
  `wgpu::Queue`; depth buffer attaches here
- GUP-009: Core Mark Trait ✅ — `Mark` trait that `Sphere3D`, `Box3D`, and
  `Line3D` must implement
- GUP-010: Basic Mark Implementations ✅ — established the pattern for per-mark
  vertex types and WGSL shader files that 3D marks follow
- GUP-131: Shader Type Constructors ✅ — supplies `Vec3::new`, `Mat4`
  constructors used in `Camera` and WGSL lighting code

### Enables Stories

- A future "GPU Frustum Culling for 3D Marks" story — depth buffer and camera
  uniform are prerequisites for view-frustum-based culling in compute shaders
- A future "3D Axis & Grid" story — camera uniform needed to project axis lines
  into 3D space

## Testing Strategy

- **Unit tests**: `Camera` matrix correctness — verify that
  `perspective(π/2, 1.0, 0.1, 100.0)` maps the near and far planes correctly;
  `look_at` produces orthonormal basis vectors
- **Integration tests**: headless render of 1 000 `Sphere3D` instances; assert
  no wgpu validation layer errors and that the colour attachment is non-zero
  (i.e., something was actually drawn)
- **Visual validation**: run `examples/scatter_3d` and capture a screenshot;
  verify that spheres appear at different depths and that the specular highlight
  rotates with the camera orbit
- **Performance**: 100 K `Sphere3D` instances should render in < 16 ms per frame
  on a discrete GPU (assert via `wgpu::QuerySet` timestamp or
  `std::time::Instant` wall-clock guard in the integration test)

## Success Metrics

- [ ] 100 K `Sphere3D` marks render at ≥ 60 FPS on native discrete GPU
- [ ] Zero wgpu validation layer errors in the integration test suite
- [ ] `Camera::perspective` and `Camera::look_at` pass all unit tests
- [ ] `examples/scatter_3d` compiles, runs, and produces a visible 3D scatter
      plot with working orbit animation

## Risk Assessment

- **Medium**: Billboard sphere depth reconstruction requires writing a depth
  value from the fragment shader (`@builtin(frag_depth)`). Not all wgpu backends
  enable this by default; the Vulkan and Metal backends support it, but the
  OpenGL compatibility backend may behave differently. _Mitigation_: Gate the
  billboard depth-write technique behind a runtime capability check; fall back
  to simple disc (no depth correction) when `frag_depth` write is unavailable.

- **Low**: Column-major vs row-major matrix layout differences between Rust math
  conventions and WGSL. Transposing matrices at the boundary is a common source
  of subtle bugs. _Mitigation_: Add dedicated unit tests that verify each matrix
  transform component independently against known values (e.g., `perspective`
  near-plane clip).

- **Low**: Depth buffer format compatibility — `Depth32Float` is universally
  supported by wgpu backends, but `Depth24PlusStencil8` may be preferred on some
  hardware. Using `Depth32Float` throughout avoids format-negotiation
  complexity. _Mitigation_: Hardcode `Depth32Float` for now; add a
  format-selection API in a follow-up story if stencil support is needed.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
