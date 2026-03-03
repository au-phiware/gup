# GUP-275: Choropleth Chart Builder

## Story Overview

**Initiative**: Chart Builders  
**Status**: 🚧 In Progress  
**Created**: 2025-07-14

## Context

A choropleth map is the most widely recognised form of geographic data
visualisation: each region in a map is filled with a colour derived from an
associated numeric value. Election results, population density, unemployment
rates, and disease incidence are all communicated in this way. Without
first-class choropleth support, users who need geographic visualisation must
assemble projection, geometry tessellation, colour mapping, and legend rendering
by hand — a multi-day task that defeats the purpose of a high-level chart
builder API.

GUP-018 established the `ChartBuilder` fluent API pattern that all chart types
follow. GUP-273 (Geographic Projection Shader System) delivers GPU-side map
projections (Mercator, Equal-Earth, etc.) and the WGSL infrastructure for
transforming geographic coordinates into screen space. GUP-274 (Map Mark
Rendering) provides the `GeoMark` that tessellates GeoJSON polygons into GPU
vertex buffers and renders them using a projection shader. GUP-255 (ColorScale
GPU Shader Function) provides composable, GPU-resident colour scale primitives
(Viridis, Plasma, diverging scales, etc.) that map a normalised domain value to
a `vec4<f32>` colour.

This story assembles these building blocks into a single, ergonomic
`ChoroplethChartBuilder` that a developer can use in a handful of lines: supply
GeoJSON boundaries, a data table, and a colour scale and receive a fully
rendered, interactive choropleth map with a colour legend.

## User Story

> "As a visualization developer, I want a `ChoroplethChartBuilder` that accepts
> GeoJSON boundaries, a dataset, and a colour scale, so that I can render a
> publication-quality choropleth map with a colour legend in a few lines of
> code."

## Acceptance Criteria

### AC1: Fluent Builder API

- [ ] `ChoroplethChartBuilder` is constructable via `gup::choropleth()` or
      `ChartBuilder::choropleth()`.
- [ ] `.boundaries(geojson: &GeoJson)` accepts a GeoJSON `FeatureCollection` (or
      `Geometry`) and stores the boundary data.
- [ ] `.data(values: impl IntoIterator<Item = (impl Into<String>, f64)>)` (or
      equivalent) loads a mapping of region identifier → numeric value.
- [ ] `.region_id(accessor)` configures how a GeoJSON feature's ID or property
      is resolved (e.g., `.region_id(|f| f.property("iso_a3"))`).
- [ ] `.value(accessor)` configures how a value is extracted from the data
      record when the data source is a struct slice rather than a pre-keyed map.
- [ ] `.color_scale(scale: ColorScale)` accepts any `ColorScale` (e.g.,
      `ColorScale::viridis()`, `ColorScale::plasma()`,
      `ColorScale::diverging(…)`).
- [ ] `.projection(Projection::Mercator)` (and other projections from GUP-273)
      selects the geographic projection applied to all boundaries.
- [ ] The builder compiles and renders without errors using the test dataset
      (see AC4).

### AC2: Rendering Correctness

- [ ] Each GeoJSON feature polygon is filled with the colour corresponding to
      its associated value under the chosen `ColorScale`.
- [ ] Features with no associated data value are rendered in a configurable
      "no-data" colour (default: mid-grey).
- [ ] Polygon boundaries (strokes) are rendered as a separate, configurable
      layer (default: thin white stroke, opacity 0.4).
- [ ] The rendered map produces no wgpu validation errors or warnings.
- [ ] The map fills the chart area respecting the configured margins/padding.

### AC3: Colour Legend

- [ ] A continuous colour bar (gradient rectangle) is rendered as a chart axis,
      oriented horizontally by default and positionable via
      `.legend_position(…)`.
- [ ] The colour bar displays the domain min/max values as tick labels using the
      standard axis label formatter.
- [ ] `.legend(false)` suppresses the colour bar entirely.
- [ ] The legend is GPU-rendered (not a CPU-composited overlay) and uses the
      same `ColorScale` shader function applied to the regions.

### AC4: World-Population Example

- [ ] An example `examples/choropleth_world_population.rs` exists that: - Loads
      a bundled or fetched simplified world GeoJSON (country boundaries). - Maps
      country ISO codes to population values from an inline data table. -
      Renders using `ColorScale::viridis()` and `Projection::Mercator`. -
      Displays a colour legend beneath the map.
- [ ] The example compiles with `cargo check --examples` and runs without
      panicking in headless mode.

### AC5: Zoom and Pan

- [ ] The choropleth map supports pointer-driven zoom and pan via the
      interaction layer introduced in GUP-277 (if complete), or provides a no-op
      stub that is replaced when GUP-277 lands.
- [ ] `.zoom(true)` / `.zoom(false)` enables or disables zoom and pan (default:
      enabled).

## Technical Tasks

- [ ] Create `src/chart_builders/choropleth.rs` with `ChoroplethChartBuilder`
      struct and builder methods.
- [ ] Implement `boundaries()`, `data()`, `region_id()`, `value()`,
      `color_scale()`, `projection()`, `zoom()`, `legend()`, and
      `legend_position()` builder methods.
- [ ] Implement `build() -> Result<ChoroplethChart, GupError>` that resolves the
      builder into a renderable chart, joining the GeoJSON features with the
      data table to produce per-region colour uniform values.
- [ ] Delegate geometry tessellation and GPU upload to `GeoMark` (GUP-274); pass
      the per-region `ColorScale` lookup to the fragment shader.
- [ ] Implement the "no-data" fallback colour path: features with no match in
      the data map receive the configured fallback colour.
- [ ] Implement the colour legend using `ColorBarAxis` (or equivalent): a thin
      horizontal gradient rect drawn from the same `ColorScale` function, with
      domain ticks.
- [ ] Wire the zoom/pan interaction stub: if GUP-277 interaction types are
      available, use them; otherwise expose a `ZoomPanState` placeholder.
- [ ] Add `gup::choropleth()` top-level constructor function in `src/lib.rs`.
- [ ] Write unit tests for the data-join logic (region_id lookup, no-data
      fallback, domain normalisation).
- [ ] Write integration test that constructs a `ChoroplethChartBuilder` with
      minimal synthetic GeoJSON and a two-entry data table and asserts that the
      resulting `ChoroplethChart` renders without GPU errors.
- [ ] Create `examples/choropleth_world_population.rs` with bundled simplified
      world boundary data and inline population dataset.
- [ ] Update `docs/planning/stories/INDEX.md` to add GUP-275.

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot-Style Chart Builders ✅ — provides the `ChartBuilder`
  fluent API pattern, rendering pipeline, and axis/legend infrastructure that
  `ChoroplethChartBuilder` extends.
- GUP-273: Geographic Projection Shader System 📋 — provides the `Projection`
  enum and GPU-side coordinate-transform shader functions required to project
  geographic coordinates into screen space.
- GUP-274: Map Mark Rendering 📋 — provides `GeoMark`, which tessellates GeoJSON
  polygon geometries into GPU vertex buffers and handles rendering.
- GUP-255: ColorScale GPU Shader Function 📋 — provides the composable
  `ColorScale` shader function (Viridis, Plasma, diverging, etc.) used to colour
  each region and to render the colour legend.

### Enables Stories

- GUP-277: Zoom and Pan Interactions — the choropleth map is the primary
  consumer of geographic interaction (zoom, pan) once that story lands.

## Testing Strategy

- **Unit tests**: Data-join logic — given a synthetic `FeatureCollection` with
  three features (IDs "A", "B", "C") and a data map containing values for "A"
  and "C" only, assert that "B" receives the no-data colour, and that "A" and
  "C" are normalised correctly against the domain.
- **Integration tests**: Construct a minimal `ChoroplethChartBuilder` with a
  three-polygon GeoJSON and a matching data table; call `build()` and `render()`
  in a headless wgpu context; assert no GPU validation errors and that the
  output framebuffer is non-uniform (i.e. colour variation is present).
- **Visual validation**: Run `examples/choropleth_world_population.rs` and
  inspect the rendered PNG for correct colour gradation across countries.
- **Compile check**: `cargo check --examples` must pass for the new example.

## Success Metrics

- [ ] `gup::choropleth()` API renders a world choropleth from GeoJSON + data
      table in ≤ 15 lines of user code.
- [ ] No GPU validation errors or Rust panics in the world-population example
      run.
- [ ] Colour legend gradient matches the chosen `ColorScale` visually (verified
      by screenshot inspection).
- [ ] Unit and integration tests pass under `cargo test -- --test-threads=1`.
- [ ] No-data regions are visually distinguishable from minimum-value regions.

## Risk Assessment

- **Medium**: GUP-273 and GUP-274 are both `📋 Planned` and must land first. If
  either is delayed or their APIs differ from what is assumed here, the builder
  method signatures (`.projection()`, `.boundaries()`) may need adjustment.  
  _Mitigation_: Draft the `ChoroplethChartBuilder` API as a thin facade and keep
  the GeoMark and Projection integration behind an internal adapter trait so
  that API surface changes are localised.

- **Medium**: GeoJSON feature ID conventions vary widely in real-world datasets
  (some use `feature.id`, others use a property like `"iso_a3"` or `"FIPS"`). A
  single `.region_id(accessor)` closure should handle all cases, but testing
  against real datasets is needed.  
  _Mitigation_: The example uses ISO-A3 codes (a well-known standard); document
  the accessor pattern clearly and test with both `feature.id` and
  property-based lookups.

- **Low**: Rendering the colour legend as a GPU-resident gradient requires the
  `ColorBarAxis` primitive (expected from GUP-255 or the axis system). If this
  is not yet implemented, a CPU-composited fallback legend can stand in
  temporarily.  
  _Mitigation_: Implement a minimal `ColorBarAxis` within this story if it is
  not already available; it is a small rectangle rendered with the `ColorScale`
  shader function as its fill.

- **Low**: Zoom and pan (GUP-277) may not be complete when this story is
  implemented. The dependency is listed as optional — the chart renders
  correctly without interaction.  
  _Mitigation_: Guard the interaction wiring behind a feature flag or a
  conditional compile path so the chart is fully usable without GUP-277.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
