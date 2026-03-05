# GUP-367: Choropleth Module Refactoring

## Story Overview

**Initiative**: Chart Builders
**Status**: 💡 New
**Created**: 2025-07-18

## Context

The choropleth module (`src/chart_builder/builders/choropleth.rs`) has grown to
approximately 1 750 lines as features have been added across GUP-275 and
GUP-287. It contains the builder, the built chart type, geometry helpers
(tessellation, simplification), colour scale sampling, GPU recolouring data
structures, and 40+ unit tests — all in a single file.

This story splits the module into sub-modules for improved maintainability and
navigability as more choropleth features land (GUP-288, GUP-289).

## User Story

> "As a contributor to the Gup project, I want the choropleth code organised
> into focused sub-modules, so that I can find and modify specific functionality
> without scrolling through a 1 750-line file."

## Acceptance Criteria

- [ ] `src/chart_builder/builders/choropleth/` directory replaces the single
      `choropleth.rs` file.
- [ ] Sub-modules include at minimum: `mod.rs` (re-exports), `builder.rs`
      (ChoroplethChartBuilder), `chart.rs` (ChoroplethChart, update_colors),
      `recolor.rs` (RegionColorBuffer, IndexedChoroplethVertex, shaders),
      `geometry.rs` (tessellation, simplification helpers).
- [ ] All existing public API paths continue to work (no breaking changes).
- [ ] All 40+ existing tests pass without modification.

## Dependencies

### Prerequisite Stories

- GUP-287: GPU-Side Choropleth Recolouring ✅

## Testing Strategy

- All existing choropleth tests must pass without changes.
- Compile check that all downstream code (examples, other modules) continues
  to build.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
