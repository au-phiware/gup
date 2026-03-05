# GUP-304: Per-Layer Data Support for Composite Charts

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**:
2026-03-03 **Completed**: 2025-07-19

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

- [x] A new method `.layer_with_data(builder, data)` accepts a builder and its
      own `Vec<T2>` where `T2` may differ from the composite's primary `T`.
- [x] Domain introspection still works: the composite computes the union of all
      layers' x and y domains regardless of their data type.
- [x] An example demonstrates compositing a scatter plot and a line plot that
      use different data types.
- [x] The existing single-type `.layer(builder)` API continues to work
      unchanged.

## Technical Tasks

- [x] Design a type-erased `ErasedLayer` trait or `AnyLayer` enum variant that
      hides the concrete data type behind domain-computation and render
      interfaces.
- [x] Implement `layer_with_data(builder, data)` on `CompositeChartBuilder`.
- [x] Update domain-unification logic to iterate both same-type and erased
      layers.
- [x] Write unit and integration tests.

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
  _Mitigation_: Consider a limited approach using
  `Box<dyn ErasedDomain + ErasedRender>` only for the "foreign" layers, keeping
  the enum path for same-type layers.

## Definition of Done

- [x] All Acceptance Criteria are satisfied.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

- **`ErasedBuiltLayer` trait** — Type-erased interface for built layers enabling
  prepare/draw/render-ready lifecycle for layers with any data type.
- **`ErasedLayerSpec` trait** — Type-erased interface for deferred layer
  specifications, capturing pre-computed domain info and deferring the build
  until unified scales are known.
- **`TypedLayerSpec<T2>` struct** — Concrete implementation of `ErasedLayerSpec`
  that stores a builder (`LayerKind<T2>`), its data (`Vec<T2>`), and cached
  domain ranges.
- **`AnyCompositeLayer<T>` enum** — Maintains declaration order between
  same-type (`Typed`) and foreign-type (`Erased`) layers in the builder.
- **`AnyBuiltLayer<T>` enum** — Maintains declaration order between typed and
  erased built layers in the output `CompositeChart`, enabling correct rendering
  order.
- **`.layer_with_data(builder, data)`** — New method on `CompositeChartBuilder`
  that accepts a builder and data with a foreign type `T2`.
- **`.layer_with_data_y2(builder, data)`** — Same as above for the secondary
  y-axis.
- **Domain unification** — Updated to iterate both `Typed` and `Erased` layer
  variants, computing the union of all layers' x and y domains regardless of
  their concrete data type.

### Key Files Changed

| File                                      | Change                                     |
| ----------------------------------------- | ------------------------------------------ |
| `src/chart_builder/builders/composite.rs` | Core: erased traits, enums, new methods    |
| `tests/composite_chart_integration.rs`    | 5 new GPU integration tests                |
| `examples/composite_mixed_data.rs`        | New: scatter(Observation) + line(FitPoint) |

### Test Counts

- **7 new unit tests** in `composite.rs` (erased layer construction, domain
  caching, mixed-type domain unification, API unchanged)
- **5 new integration tests** in `composite_chart_integration.rs` (mixed-data
  build, domain unification, dual-axis, prepare+render, all-erased layers)
- **All 15 composite integration tests** pass
- **All 35 composite unit+integration tests** pass
