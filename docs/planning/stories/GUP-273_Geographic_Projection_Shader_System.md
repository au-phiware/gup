# GUP-273: Geographic Projection Shader System

## Story Overview

**Initiative**: Shader Function System **Status**: 🚧 In Progress **Created**:
2026-03-02

## Context

The Gup implementation strategy uses geographic projection as the motivating
example of composable shader functions:

```rust
chart.select_all::<Circle>()
    .data(weather_data)
    .attr("position",
        geographic_projection::new(viewport)
            .mix(screen_transform::new(dimensions))
    );
```

This pattern — transforming `(longitude, latitude)` pairs through a map
projection and then into pixel-space screen coordinates — represents exactly the
kind of multi-stage GPU pipeline that the `ShaderFunction` composition system
was designed to enable. The pipeline builder (GUP-052) and the foundation trait
(GUP-005) are complete; what is missing is the domain-specific layer that knows
how to perform cartographic projections in WGSL.

Geographic projections are mathematically non-trivial: each projection defines a
unique, often transcendental mapping from the sphere to the plane, with
configurable centre, scale, and translation parameters. They also have natural
validity boundaries (e.g. the Mercator projection is undefined at the poles),
which means a projection shader function must also be able to signal that a
given point should be culled from the render. This clipping behaviour integrates
with the existing shader pipeline rather than requiring bespoke geometry
processing.

GUP-053 (Advanced Shader Function Library) establishes the organisation pattern
for domain-specific shader function modules (`src/shader_functions/`). This
story follows the same pattern, adding a `src/shader_functions/geo.rs` module
that houses all projection types and the `GeoPoint` coordinate type they operate
on.

## User Story

> "As a visualization developer, I want composable GPU shader functions for
> common geographic projections so that I can map `(longitude, latitude)`
> coordinates to screen pixels by composing a projection with a screen
> transform, without writing raw WGSL."

> "As a visualization developer building map-based charts, I want
> out-of-boundary coordinates to be automatically culled by the projection
> shader so that points outside the visible projection region are discarded on
> the GPU rather than producing visual artefacts."

## Acceptance Criteria

### AC1: GeoPoint Coordinate Type

- [ ] A `GeoPoint` struct with `longitude: f32` and `latitude: f32` fields is
      defined and implements `ShaderType` so it can be used as
      `ShaderFunction::Input`
- [ ] `GeoPoint` implements `bytemuck::Pod` and `bytemuck::Zeroable`, allowing
      it to be written directly into GPU vertex or instance buffers
- [ ] The WGSL representation of `GeoPoint` is a
      `struct gup_GeoPoint { longitude: f32, latitude: f32 }` and the generated
      binding code is verified by a unit test that inspects the emitted WGSL
      string

### AC2: Equirectangular Projection

- [ ] `EquirectangularProjection` implements `ShaderFunction` with
      `Input = GeoPoint` and `Output = vec2<f32>`
- [ ] The projection maps `(lon, lat)` to `(lon * cos(lat_0), lat)` in radians,
      where `lat_0` is the configurable central parallel (default `0.0`)
- [ ] Uniforms struct
      `EquirectangularUniforms { center_lon: f32, center_lat: f32, scale: f32, translate_x: f32, translate_y: f32 }`
      is `bytemuck::Pod`
- [ ] A unit test verifies that `(0°, 0°)` maps to `(0.0, 0.0)` before scale and
      translation are applied, and that scale and translation are applied
      correctly

### AC3: Mercator Projection

- [ ] `MercatorProjection` implements `ShaderFunction` with `Input = GeoPoint`
      and `Output = vec2<f32>`
- [ ] The WGSL function applies the standard Mercator formula:
      `x = lon - center_lon`, `y = ln(tan(π/4 + lat/2))`
- [ ] Uniforms struct
      `MercatorUniforms { center_lon: f32, scale: f32, translate_x: f32, translate_y: f32, clip_lat: f32 }`
      is `bytemuck::Pod`, where `clip_lat` controls the maximum absolute
      latitude rendered (default `85.051129°` in radians, the Web Mercator
      standard limit)
- [ ] A unit test verifies that `(0°, 0°)` maps to `(0.0, 0.0)` (before scale
      and translate) and that the reference point `(180°, 0°)` maps to a
      positive x-axis displacement equal to `π * scale`

### AC4: Stereographic Projection

- [ ] `StereographicProjection` implements `ShaderFunction` with
      `Input = GeoPoint` and `Output = vec2<f32>`
- [ ] The WGSL function applies the azimuthal stereographic formula centred on a
      configurable `(center_lon, center_lat)` pole point
- [ ] Uniforms struct
      `StereographicUniforms { center_lon: f32, center_lat: f32, scale: f32, translate_x: f32, translate_y: f32 }`
      is `bytemuck::Pod`
- [ ] A unit test verifies the antipodal point (diametrically opposite the
      projection centre) returns a radius that exceeds a large sentinel value (≥
      `1e6`), confirming correct divergence behaviour at the antipode

### AC5: Orthographic Projection

- [ ] `OrthographicProjection` implements `ShaderFunction` with
      `Input = GeoPoint` and `Output = vec2<f32>`
- [ ] The WGSL function applies the azimuthal orthographic formula, projecting
      only the hemisphere facing the viewer (points on the far hemisphere are
      clipped)
- [ ] Uniforms struct
      `OrthographicUniforms { center_lon: f32, center_lat: f32, scale: f32, translate_x: f32, translate_y: f32 }`
      is `bytemuck::Pod`
- [ ] A unit test verifies that the projection centre itself maps to
      `(0.0, 0.0)` (before scale and translate) and that a point 90° away on the
      great circle maps to a point on the unit circle boundary (radius ≈ 1.0
      within `1e-5`)

### AC6: Projection Boundary Clipping

- [ ] Each projection that has a natural validity boundary (Mercator latitude
      clamp; Orthographic far-hemisphere) encodes the clip test inside its WGSL
      function
- [ ] When a point is outside the valid projection region, the WGSL function
      returns a sentinel value `vec2(CLIP_SENTINEL, CLIP_SENTINEL)` where
      `CLIP_SENTINEL` is a named constant (`1e9`) exported from the module
- [ ] The Rust side exposes a `CLIP_SENTINEL: f32` constant so downstream code
      (e.g. a fragment discard or geometry filter) can test for clipped points
      without magic numbers
- [ ] A unit test for each clipping projection verifies that an out-of-bounds
      input produces a sentinel output

### AC7: Composition with Screen Transform

- [ ] At least one projection can be composed with `screen_transform` (or an
      equivalent `ShaderFunction` that maps `vec2<f32>` to clip-space
      `vec2<f32>`) using the `ShaderPipeline` builder from GUP-052
- [ ] An integration test exercises the full
      `GeoPoint → projected vec2 →     screen vec2` pipeline, feeding a known
      `(lon, lat)` through the composed pipeline and asserting the output pixel
      position is within `1.0` pixel of the expected value
- [ ] The integration test runs without GPU validation errors

### AC8: Module Organisation and Public API

- [ ] All projection types are defined in `src/shader_functions/geo.rs` and
      re-exported from `src/shader_functions/mod.rs`
- [ ] The public API surface is
      `pub use shader_functions::geo::{GeoPoint, EquirectangularProjection, MercatorProjection, StereographicProjection, OrthographicProjection, CLIP_SENTINEL}`
- [ ] A `geographic_projection` example (or extended existing map example)
      demonstrates composing a projection with a screen transform and rendering
      a set of world-city coordinates as `Circle` marks

## Technical Tasks

- [ ] Define `GeoPoint` struct in `src/shader_functions/geo.rs`; derive /
      implement `bytemuck::Pod`, `bytemuck::Zeroable`, and `ShaderType`; write
      the WGSL struct snippet and unit test for it
- [ ] Implement `EquirectangularProjection` with `EquirectangularUniforms`;
      write the WGSL function; write unit tests for the coordinate mapping and
      the scale/translate application
- [ ] Implement `MercatorProjection` with `MercatorUniforms`; write the WGSL
      function including the latitude clamp; write unit tests for the coordinate
      mapping, the `(180°, 0°)` reference point, and the clip sentinel for a
      beyond-`clip_lat` input
- [ ] Implement `StereographicProjection` with `StereographicUniforms`; write
      the WGSL function; write unit tests for the centre identity and the
      antipodal divergence
- [ ] Implement `OrthographicProjection` with `OrthographicUniforms`; write the
      WGSL function including the far-hemisphere clip test; write unit tests for
      the centre identity, the 90°-away boundary point, and the clip sentinel
- [ ] Define and export `CLIP_SENTINEL: f32 = 1e9` as a module-level constant;
      ensure the same value appears as a WGSL constant in each clipping
      projection's shader snippet
- [ ] Re-export all types from `src/shader_functions/mod.rs`
- [ ] Write an integration test (`tests/geo_projection.rs` or inline in the
      module) that composes `MercatorProjection` with a `ScreenTransform` and
      validates a `(lon, lat)` → pixel round-trip
- [ ] Add a `geographic_projection` example under `examples/` that plots
      world-city coordinates as `Circle` marks, demonstrating the composed
      pipeline

## Dependencies

### Prerequisite Stories

- GUP-005: ShaderFunction Trait ✅ — defines the `ShaderFunction` trait,
  `ShaderType` bounds, and uniform management that all projection types
  implement
- GUP-007: Shader Pipeline Builder ✅ — provides the foundational pipeline
  infrastructure that the composition integration test relies on
- GUP-052: Shader Pipeline Builder v2 ✅ — the production-ready pipeline builder
  used to compose projection functions with screen transforms
- GUP-053: Advanced Shader Function Library 📋 — establishes the
  `src/shader_functions/` module structure and organisation conventions that
  this story follows; the geo module should be consistent with the
  math/color/geometry modules introduced there

### Enables Stories

- GUP-274: Map Mark Rendering — depends on the projection shader functions and
  `GeoPoint` type defined here to drive the GPU vertex stage for map marks
- GUP-275: Choropleth Chart Builder — uses the projection pipeline from this
  story to position and shade geographic regions by data value

## Testing Strategy

- **Unit tests**: Each projection and the `GeoPoint` type has pure-Rust unit
  tests that verify: WGSL snippet non-empty and contains the function name;
  uniform struct satisfies `bytemuck::Pod`; specific `(lon, lat)` inputs produce
  expected `(x, y)` outputs (computed analytically or against reference
  implementations); clip conditions produce the sentinel value
- **Integration tests**: A GPU integration test runs the composed
  `MercatorProjection → ScreenTransform` pipeline using the existing GPU test
  harness, feeds a small dataset of known coordinates, reads back the output
  buffer, and asserts pixel positions within ±1.0 px of expected values and the
  absence of GPU validation errors
- **Visual validation**: The `geographic_projection` example renders world-city
  coordinates as circles on a Mercator background; a manual screenshot review
  confirms correct geographic placement
- **Composition tests**: Verify at compile time that `GeoPoint → vec2<f32>`
  pipelines type-check correctly and that attempting to compose a
  `f32 → vec2<f32>` function directly with a projection (type mismatch) is
  rejected

## Success Metrics

- [ ] All four projection types (`Equirectangular`, `Mercator`, `Stereographic`,
      `Orthographic`) are implemented and pass their unit tests
- [ ] The clip sentinel mechanism is tested for every projection that applies
      clipping
- [ ] The GPU integration test passes the `(lon, lat)` → pixel round-trip within
      ±1.0 px without GPU validation errors
- [ ] `cargo test -- --test-threads=1` passes in full
- [ ] `cargo check --examples` passes with the new `geographic_projection`
      example

## Risk Assessment

- **Medium**: The Stereographic and Orthographic WGSL implementations involve
  trigonometric functions (`sin`, `cos`, `atan2`) that behave correctly in WGSL
  but whose precision may differ slightly from double-precision reference
  values. _Mitigation_: Use a generous tolerance (≥ `1e-4`) in numeric
  assertions and cross-check against a known reference implementation (e.g.
  D3-geo formulas) before writing the test vectors.

- **Medium**: GUP-053 (Advanced Shader Function Library) is a prerequisite for
  the module structure conventions but is itself still 📋 Planned. If GUP-053 is
  not yet merged when this story is picked up, the `geo.rs` module may need to
  be introduced before the other shader function modules exist. _Mitigation_:
  The `geo.rs` module is self-contained; it can be created in
  `src/shader_functions/geo.rs` independently and merged with the module
  organisation established by GUP-053 in a single follow-up tidy-up if needed.
  Do not block on GUP-053 if the composition infrastructure (GUP-052) is
  available.

- **Low**: The `clip_lat` / far-hemisphere sentinel approach for clipping
  requires downstream consumers (fragment shaders, geometry filters) to know
  about and test for the sentinel value. If a consumer silently renders sentinel
  points, artefacts will appear at `(1e9, 1e9)` in screen space rather than
  being discarded. _Mitigation_: Export `CLIP_SENTINEL` as a public constant and
  document the discard contract clearly; the integration test and example should
  both demonstrate the discard pattern.

- **Low**: The Mercator `ln(tan(π/4 + lat/2))` formula is numerically unstable
  as `lat → ±90°`. The default `clip_lat` of 85.051129° is chosen to avoid this,
  but a developer passing a custom `clip_lat` beyond that bound could produce
  `inf` or `nan` on the GPU. _Mitigation_: Clamp the latitude inside the WGSL
  function before applying the logarithm, regardless of the `clip_lat` uniform
  value, and document this behaviour.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
