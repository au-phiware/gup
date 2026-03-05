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

## Retrospective

**Completed**: 2025-07-19

### Key Technical Learnings

#### Hybrid Enum + Trait Object Pattern

- **Challenge**: The project strongly prefers enums over trait objects for
  known variant sets. However, per-layer data support inherently requires
  type erasure — the composite builder cannot know the foreign data type `T2`
  at compile time. Pure enums cannot express "any T2".
- **Solution**: Combined both approaches: `AnyCompositeLayer<T>` and
  `AnyBuiltLayer<T>` are two-variant enums (`Typed | Erased`) where the
  `Typed` variant uses the existing `LayerKind<T>` enum and the `Erased`
  variant wraps `Box<dyn ErasedLayerSpec>` / `Box<dyn ErasedBuiltLayer>`.
  This keeps compile-time type safety for same-type layers while enabling
  type erasure only where strictly necessary.
- **Pattern**: When you have a known set of variants _plus_ an open-ended
  extension point, use an enum with a `Erased(Box<dyn Trait>)` catch-all
  variant. This preserves exhaustiveness checks for the known variants and
  limits trait-object overhead to the extension case.

#### Deferred Build with Cached Domains

- **Challenge**: Domain unification must happen across all layers _before_
  any layer is built (since scales depend on the unified domain). But erased
  layers carry their own data, so we can't compute their domains at
  `build_with_data` time using the shared `Vec<T>`.
- **Solution**: Pre-compute each erased layer's x/y domain at
  `layer_with_data()` time (when `Vec<T2>` and the builder are still
  available) and cache the results in `TypedLayerSpec`. At `build_with_data`
  time, the cached domains participate in unification alongside typed layers'
  domains computed from the shared data.
- **Pattern**: When type erasure prevents late computation, eagerly compute
  type-dependent values _before_ erasure and cache them in the wrapper.

#### Object-Safe Lifetime Parameters

- **Challenge**: `BuiltLayer<T>::draw` has signature
  `fn draw<'a>(&'a self, render_pass: &mut RenderPass<'a>)` — tying `self`
  and the render pass to the same lifetime. This could potentially break
  object safety since it looks like a generic parameter.
- **Solution**: Rust lifetime parameters on methods are late-bound and do
  not affect object safety. Only type parameters make a trait non-object-safe.
  The `ErasedBuiltLayer` trait with `fn erased_draw<'a>(&'a self, ...)` works
  correctly as `dyn ErasedBuiltLayer`.
- **Pattern**: Lifetime parameters on trait methods are always fine for
  `dyn Trait`. Only generic _type_ parameters violate object safety.

### Architectural Decisions

#### Drop Clone from CompositeChartBuilder

- **Decision**: Removed the `Clone` derive from `CompositeChartBuilder<T>`
  since `Box<dyn ErasedLayerSpec>` cannot implement `Clone`.
- **Reasoning**: The builder is consumed by `build_with_data()`, so Clone
  is never needed externally. Grep confirmed no external `.clone()` calls.
- **Trade-off**: If someone later needs to clone a builder (e.g. for forking
  a composition), they'll need a manual Clone implementation that deep-clones
  the erased layers. This is unlikely given the consuming builder pattern.
- **Future**: Could add a `CloneableErasedLayerSpec` supertrait if clone
  becomes needed, but YAGNI applies.

#### Two-Method API (layer_with_data + layer_with_data_y2)

- **Decision**: Provided `.layer_with_data()` for primary y-axis and
  `.layer_with_data_y2()` for secondary y-axis, mirroring the existing
  `.layer()` / `.layer_with_y2()` pair.
- **Reasoning**: Consistent with the existing API surface. A builder-style
  chaining approach (`.layer_with_data(builder, data).y_axis(Secondary)`)
  would require a temporary intermediate type.
- **Trade-off**: Two methods instead of one with a configuration parameter.
  But this matches the established convention.
- **Future**: If a third axis or more complex assignment schemes emerge, a
  `.layer_with_data_config(builder, data, LayerConfig)` variant could unify
  the API.

#### All-Erased Composite Support

- **Decision**: Allowed composites where _all_ layers are erased (no typed
  layers at all) by passing empty `Vec<T>` as data. The primary type `T`
  becomes phantom in this case.
- **Reasoning**: Enables maximum flexibility — users can compose layers
  from entirely different data sources. The anchor selection for axis
  rendering is created with empty data, which is harmless.
- **Trade-off**: The empty-data check now only triggers when there are typed
  layers without data, which is slightly more complex than the original
  "always reject empty data" approach.
- **Future**: Could add `composite_from_layers()` that doesn't require
  specifying a primary `T` at all.

### Development Workflow Insights

- **Fast iteration from prior stories**: GUP-251 and GUP-303 established
  the composite infrastructure so thoroughly that this story required
  _no changes_ to any files outside the composite module, integration tests,
  and the new example. The enum-based architecture was designed with exactly
  this extension in mind.
- **Test-first domain logic**: The unit tests for `TypedLayerSpec` domain
  caching caught no bugs because the logic was trivially correct. However,
  they serve as regression guards for future refactors.
- **GPU test single-threading**: `--test-threads=1` remains essential.
  Parallel GPU context creation segfaults reliably. Using `--test <name>`
  to target the integration test file is much faster than filtering all
  test binaries.
- **Build times**: The project has 100+ test binaries. Running
  `cargo test -p gup --lib composite` took ~5 minutes just to compile even
  when no code changed, because the lib test binary is huge. Targeting
  `--test composite_chart_integration` is 10x faster for iteration.

### Follow-up Stories

1. **GUP-362: Accessor-to-GPU Position Pipeline** — Already planned. The
   `apply_accessors_to_selection` placeholder closures and the "override by
   append" workaround in `build_layer()` should be replaced by proper GPU-side
   scale transformations. This becomes more important with per-layer data since
   each erased layer independently generates NDC positions via CPU-side
   `scale_value()`.

2. **GUP-365: Composite Layer Ordering Control** — Currently layers render
   strictly in declaration order. For complex mixed-type composites, users
   may want to re-order layers after construction (e.g. move the regression
   line behind scatter points). A `.layer_order()` or z-index mechanism would
   help.
