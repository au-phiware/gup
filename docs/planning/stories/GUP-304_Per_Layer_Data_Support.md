# GUP-304: Per-Layer Data Support for Composite Charts

## Story Overview

**Initiative**: Chart Builders **Status**: 📋 Planned **Created**: 2026-03-03

## Context

GUP-251's `CompositeChartBuilder<T>` requires all layers to share a single data
type `T`. This works well when every layer visualises different facets of the
same dataset, but falls short for heterogeneous compositions — for example, a
scatter plot of raw observations (`Observation { x, y }`) overlaid with a
regression line from a fitted model (`FitPoint { x, y_hat }`).

This story extends the composite API to allow each layer to carry its own data
type, enabling truly independent layer composition.

## User Story

> "As a visualization developer, I want each layer in a composite chart to
> accept its own data set so that I can overlay visualisations from different
> data schemas without converting everything to a single type."

## Acceptance Criteria

- [ ] A new method `.layer_with_data(builder, data)` accepts a builder and its
      own `Vec<T2>` where `T2` may differ from the composite's primary `T`.
- [ ] Domain introspection still works: the composite computes the union of all
      layers' x and y domains regardless of their data type.
- [ ] An example demonstrates compositing a scatter plot and a line plot that
      use different data types.
- [ ] The existing single-type `.layer(builder)` API continues to work
      unchanged.

## Technical Tasks

- [ ] Design a type-erased `ErasedLayer` trait or `AnyLayer` enum variant that
      hides the concrete data type behind domain-computation and render
      interfaces.
- [ ] Implement `layer_with_data(builder, data)` on `CompositeChartBuilder`.
- [ ] Update domain-unification logic to iterate both same-type and erased
      layers.
- [ ] Write unit and integration tests.

## Dependencies

### Prerequisite Stories

- GUP-251: Custom Composite Chart Support ✅
- GUP-303: Composite Chart GPU Render Pipeline 📋

## Testing Strategy

- Unit tests for domain computation with mixed-type layers.
- Integration test building a composite with two different data types.

## Risk Assessment

- **High**: Full type erasure is fundamentally at odds with the project's
  preference for enums over trait objects. Careful design is needed.
  _Mitigation_: Consider a limited approach using `Box<dyn ErasedDomain +
  ErasedRender>` only for the "foreign" layers, keeping the enum path for
  same-type layers.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
