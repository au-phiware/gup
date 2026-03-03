# GUP-274: Map Mark Rendering

## Story Overview

**Initiative**: Mark System  
**Status**: 🚧 In Progress  
**Created**: 2025-01-27

## Context

Geographic visualizations — choropleth maps, data overlays, regional
aggregations — require a way to render country and region outlines as vector
paths. GeoJSON is the universally accepted format for geographic boundary data,
supported by every major data source (Natural Earth, OpenStreetMap exports,
national statistics bureaus). There is no existing mark type in Gup that
understands geographic geometry.

GUP-132 (GPU Path Tessellation) already provides the machinery to tessellate
arbitrary polygon outlines on the GPU. GUP-009 (Core Mark Trait) defines the
`Mark` trait that all visual primitives must implement. What is missing is the
bridge between GeoJSON input data and the tessellation pipeline, along with the
application of a geographic projection so that spherical coordinates (longitude,
latitude) are mapped to screen-space positions.

GUP-273 (Geographic Projection Shader System) will deliver a library of
projection shader functions (Mercator, Equirectangular, etc.) that can be
invoked from within a vertex shader. This story builds on that to create
`GeoPathMark`: a first-class mark type that ingests GeoJSON `Feature` and
`FeatureCollection` documents, tessellates their polygon and multi-polygon
geometries, and renders them with fill and stroke via the GPU tessellation
pipeline. A topology simplification pass reduces polygon vertex count at small
display scales, maintaining interactive frame rates for world-scale datasets.

## User Story

> "As a visualization developer, I want to supply a GeoJSON boundary dataset and
> a projection type to a `GeoPathMark` so that country and region outlines are
> rendered as GPU-accelerated filled and stroked paths ready to serve as the
> base layer of a choropleth map."

## Acceptance Criteria

### AC1: GeoJSON Parsing

- [ ] A `GeoJsonSource` type can be constructed from a raw GeoJSON `&str` or
      `serde_json::Value`.
- [ ] `Feature` objects with `Polygon` and `MultiPolygon` geometry types are
      parsed into internal path representations.
- [ ] `FeatureCollection` objects are parsed, producing one path per feature.
- [ ] Parse errors (malformed JSON, unsupported geometry types) return a typed
      `GupError` variant; they do not panic.
- [ ] `Point`, `LineString`, `MultiPoint`, `MultiLineString`, and
      `GeometryCollection` geometry types are explicitly rejected with a clear
      error message documenting that only polygon types are supported by this
      mark.

### AC2: GeoPathMark API

- [ ] A `GeoPathMark` struct implements the `Mark` trait from GUP-009.
- [ ] It can be constructed with a `GeoJsonSource` and a projection identifier
      (e.g., `Projection::Mercator`, `Projection::Equirectangular`).
- [ ] Builder methods allow setting `fill_color: Option<Color>`,
      `stroke_color: Option<Color>`, and `stroke_width: f32`.
- [ ] The mark compiles and links cleanly: `cargo check --examples` passes.

### AC3: GPU Rendering Pipeline

- [ ] Polygon rings are tessellated using the GPU tessellation infrastructure
      from GUP-132 (earcut or equivalent).
- [ ] The vertex shader invokes the selected projection function from GUP-273 to
      convert (longitude, latitude) pairs into clip-space coordinates.
- [ ] Filled regions render without visible gaps or z-fighting between adjacent
      country polygons.
- [ ] Stroke outlines render along the original boundary ring (not the
      tessellated interior triangles).

### AC4: Topology Simplification

- [ ] A `simplification_tolerance(f32)` builder method accepts a tolerance value
      in degrees (e.g., `0.5` for coarse world maps, `0.05` for regional
      detail).
- [ ] When a non-zero tolerance is set, polygon rings are simplified using
      Ramer–Douglas–Peucker before tessellation.
- [ ] At `tolerance = 0.0` (default), simplification is skipped and original
      coordinates are used verbatim.
- [ ] The triangle count of a simplified world-map render is measurably lower
      than the unsimplified baseline (verified in the integration test).

### AC5: Example — World Map

- [ ] An example `examples/geo_world_map.rs` renders country outlines from a
      bundled low-resolution GeoJSON file (Natural Earth 110m or equivalent,
      committed to `assets/`).
- [ ] The example exits cleanly on all CI platforms without GPU validation
      errors.
- [ ] A screenshot or visual validation note is added to the example's top-level
      doc comment.

## Technical Tasks

- [ ] Add `geojson` and `serde_json` crate dependencies (feature-gated if
      appropriate to avoid bloating builds that don't need geo support).
- [ ] Implement `GeoJsonSource`: parse GeoJSON text into `Vec<Ring>` where
      `Ring = Vec<[f64; 2]>` (longitude, latitude pairs).
- [ ] Implement `GeoPathMark` struct with `Mark` trait impl: `mark_type_id()`,
      `prepare()`, `render()`.
- [ ] In `prepare()`, apply optional Ramer–Douglas–Peucker simplification to
      each ring, then enqueue tessellation jobs via the GUP-132 tessellation
      API.
- [ ] Write a vertex shader stage (WGSL) that reads pre-tessellated (lon, lat)
      vertices, calls the GUP-273 projection function, and outputs clip-space
      `vec4<f32>` positions.
- [ ] Write a fragment shader stage that applies `fill_color` to interior
      fragments and `stroke_color` to boundary fragments (or use separate draw
      calls for fill and stroke).
- [ ] Add unit tests for GeoJSON parsing: valid `Polygon`, valid `MultiPolygon`,
      valid `FeatureCollection`, malformed JSON, unsupported geometry type.
- [ ] Add integration test: load the bundled world GeoJSON, create a
      `GeoPathMark`, call `prepare()` and `render()` against a headless wgpu
      device, assert no errors and non-zero triangle count.
- [ ] Add simplification test: same world GeoJSON at `tolerance = 0.5` produces
      fewer triangles than at `tolerance = 0.0`.
- [ ] Commit the Natural Earth 110m countries GeoJSON to
      `assets/ne_110m_countries.geojson`.
- [ ] Write `examples/geo_world_map.rs` using `GeoPathMark` with Mercator
      projection.
- [ ] Update `docs/planning/stories/INDEX.md` entry to ✅ on completion.

## Dependencies

### Prerequisite Stories

- GUP-009: Core Mark Trait ✅ — provides the `Mark` trait that `GeoPathMark`
  implements
- GUP-132: GPU Path Tessellation ✅ — provides the tessellation pipeline used to
  triangulate polygon rings on the GPU
- GUP-273: Geographic Projection Shader System 📋 — provides the WGSL projection
  functions (Mercator, Equirectangular, etc.) invoked in the vertex shader

### Enables Stories

- GUP-275: Choropleth Chart Builder — `GeoPathMark` is the rendering primitive
  that the higher-level choropleth builder will use to colour regions by data
  value

## Testing Strategy

- **Unit tests**: GeoJSON parser correctness — all supported geometry types,
  error paths for unsupported types, malformed JSON.
- **Integration tests**: headless wgpu device render of the bundled world
  GeoJSON; assert zero GPU validation errors, non-zero vertex and index buffer
  sizes, and that simplification reduces triangle count.
- **Visual validation**: `examples/geo_world_map.rs` runs to completion;
  developer visually confirms country outlines appear correctly projected on
  screen.
- **Performance**: informal check that a full world render (110m resolution) at
  `tolerance = 0.5` sustains 60 fps on the CI GPU; no formal benchmark required
  at this stage.

## Success Metrics

- [ ] `cargo test -- --test-threads=1` passes with all new tests green.
- [ ] `examples/geo_world_map.rs` renders a recognisable world map with country
      outlines in a single `GeoPathMark`.
- [ ] Simplification at `tolerance = 0.5` reduces total triangle count for the
      110m world dataset by at least 30% compared to the unsimplified baseline.
- [ ] No GPU validation errors reported during the integration test render pass.

## Risk Assessment

- **Medium**: The GUP-273 projection shader API is not yet finalised (📋
  Planned). If the projection function signature changes during GUP-273
  implementation, the vertex shader in this story will need updating.  
  _Mitigation_: Keep the projection call-site isolated in a single WGSL include
  or function wrapper so that only one file needs editing if the API changes.

- **Medium**: GeoJSON files for high-resolution boundaries can be very large
  (10–100 MB). Parsing and tessellating these in a single frame could stall the
  render thread.  
  _Mitigation_: `prepare()` is an async-friendly or background operation in the
  Mark trait; document that large GeoJSON sources should be loaded and prepared
  off the render thread. This story only validates correctness with the 110m
  low-resolution dataset; high-resolution streaming is out of scope.

- **Low**: Ramer–Douglas–Peucker operates in planar (lon, lat) space, which
  introduces slight geometric inaccuracies near the poles.  
  _Mitigation_: Document the known limitation. Spherical-aware simplification
  can be added in a follow-up story if required for polar projections.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
