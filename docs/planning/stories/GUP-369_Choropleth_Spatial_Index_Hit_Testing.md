# GUP-369: Choropleth Spatial Index for Hit-Testing

## Story Overview

**Initiative**: Chart Builders **Status**: 💡 New **Created**: 2025-07-22

## Context

GUP-288 implemented CPU-side point-in-polygon hit-testing for choropleth
regions using a linear scan over all region polygons. For typical world maps
(~200 regions) this is fast, but highly granular datasets (county-level, zip
codes, census tracts with 500–3000+ regions) may see noticeable latency on
hover. A spatial index — such as a bounding-box pre-filter, grid index, or
R-tree — would reduce hit-testing time from O(n × edges) to approximately
O(log n).

## User Story

> "As a visualization developer, I want choropleth hover hit-testing to remain
> fast even with thousands of regions, so that users experience responsive
> interactions on detailed geographic datasets."

## Acceptance Criteria

- [ ] `region_at_point()` uses a spatial index for candidate filtering.
- [ ] Hit-testing performance is sub-millisecond for 3000+ region choropleths.
- [ ] The spatial index is built automatically during `ChoroplethChartBuilder::build()`.
- [ ] Correctness is identical to the current linear-scan implementation.

## Technical Tasks

1. Compute axis-aligned bounding boxes (AABBs) for each region during build.
2. Implement a grid-based or R-tree spatial index over the AABBs.
3. Filter candidate regions via the spatial index before running ray-casting.
4. Add benchmarks comparing linear scan vs indexed hit-testing.

## Dependencies

### Prerequisite Stories

- GUP-288: Choropleth Tooltip and Hover Interaction ✅

## Testing Strategy

- Unit tests verifying spatial index produces same results as linear scan.
- Benchmark with synthetic 3000-region choropleths.

## Success Metrics

- `region_at_point()` completes in < 0.5 ms for 3000-region choropleths.

## Risk Assessment

- **Low**: The spatial index is an additive optimisation; the existing API and
  correctness guarantees are unchanged.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
