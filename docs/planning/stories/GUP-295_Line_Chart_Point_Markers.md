# GUP-295: Line Chart Point Markers

## Story Overview

**Initiative**: Chart Builders **Status**: 💡 New **Created**: 2025-07-28

## Context

GUP-246 delivered the `LineChartBuilder` with polyline construction,
multi-series support, and four interpolation modes. During implementation, the
"Add optional point markers" task was deferred because overlaying a
`Selection<T, Circle>` on top of a `Selection<LineSegment<T>, Line>` requires
composing two selections of different mark types within a single chart, which
the current `ComposedChart<T, M>` generic design does not support (it is
single-mark).

Point markers are a common feature in line charts — they show the exact data
point locations as small circles or dots, making it easier to read precise
values, especially on sparse datasets.

## User Story

> "As a visualization developer, I want to optionally enable point markers on
> line charts so that individual data points are visible as small circles at
> each polyline vertex."

## Acceptance Criteria

- [ ] `LineChartBuilder` gains a `.show_markers(bool)` method (default `false`)
- [ ] When enabled, small circles are rendered at each original data point
      position
- [ ] Marker color matches the series color by default
- [ ] Marker size is configurable via `.marker_size(f32)`
- [ ] Works with all interpolation modes (markers appear at original data
      points, not at interpolated intermediate points)
- [ ] A unit test verifies marker count equals original data point count

## Technical Tasks

- [ ] Investigate whether `CompositeChart` (GUP-251) is needed, or if a simpler
      approach (e.g., a second render pass or mark-type-erased layer) suffices
- [ ] Implement the marker overlay rendering
- [ ] Add `.show_markers()` and `.marker_size()` fluent methods
- [ ] Update `line_chart_demo.rs` with a marker example

## Dependencies

### Prerequisite Stories

- GUP-246: Line Chart Builder ✅ — provides the line chart that markers overlay
- GUP-251: Custom Composite Chart Support 📋 — may be needed for multi-mark
  composition within a single chart

## Testing Strategy

- Unit tests for marker count and position correctness
- Visual validation via example

## Risk Assessment

- **Medium**: Requires composing two Selection types of different marks. If
  GUP-251 is not available, a simpler single-pass approach using the Line mark's
  existing shader to render dots at endpoints may suffice.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] Tests pass: `cargo test -- --test-threads=1`
- [ ] Lint clean: `mask all-fix`
- [ ] Example updated
