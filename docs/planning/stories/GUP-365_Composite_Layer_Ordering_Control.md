# GUP-365: Composite Layer Ordering Control

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-19
**Completed**: 2026-03-06

## Context

GUP-251 established declaration-order rendering for composite chart layers:
layers added first are drawn at the bottom, layers added last are drawn on top.
GUP-304 extended this to type-erased layers. This ordering is implicit — it
depends entirely on the order of `.layer()` / `.layer_with_data()` calls in the
builder chain.

For complex composites with many layers, users may want to adjust the rendering
order after construction — for example, moving a reference line behind scatter
points, or promoting a highlight layer to the top. A z-index or explicit
ordering API would provide this control without requiring the user to
restructure their builder chain.

## User Story

> "As a visualization developer, I want to control the rendering order of layers
> in a composite chart so that I can ensure visual priority without
> restructuring my builder calls."

## Acceptance Criteria

- [x] A method `.layer_order(order: &[usize])` or equivalent API on
      `CompositeChartBuilder` allows reordering layers by index.
- [x] Alternatively, layers accept a z-index value that determines rendering
      order (lower z-index = rendered first / behind).
- [x] The default behaviour (declaration order) is unchanged when no explicit
      ordering is specified.
- [x] An example demonstrates reordering layers.

## Technical Tasks

- [x] Design the layer ordering API (z-index vs explicit ordering vs both).
- [x] Update `CompositeChart::draw()` to respect the ordering.
- [x] Write unit tests for ordering logic.
- [x] Add an example demonstrating layer reordering.

## Dependencies

### Prerequisite Stories

- GUP-304: Per-Layer Data Support ✅

## Testing Strategy

- Unit tests verifying draw order with various ordering configurations.
- Integration test confirming render succeeds with reordered layers.

## Risk Assessment

- **Low**: The rendering pipeline already iterates layers in order; changing the
  iteration order is straightforward. _Mitigation_: Use a simple sorted index
  vector rather than physically reordering the layer storage.

## Definition of Done

- [x] All Acceptance Criteria are satisfied.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

Both a **z-index API** and an **explicit layer-order API** for controlling
composite chart layer rendering order, with full backward compatibility.

### API Design

Two complementary ordering mechanisms:

1. **`z_index(z: i32)`** — chainable method on `CompositeChartBuilder` that sets
   the render priority on the most recently added layer. Lower z-index values
   are drawn first (behind), higher values on top. Works with both typed and
   type-erased (foreign data) layers.

2. **`layer_order(order: &[usize])`** — explicit index-based reordering. The
   slice must be a permutation of `0..layer_count`. When both z-index and
   layer_order are specified, layer_order takes precedence.

3. **`render_order() -> &[usize]`** — accessor on `CompositeChart` to inspect
   the computed render order.

### Key Files Changed

- `src/chart_builder/builders/composite.rs` — Core implementation:
  - Added `z_index` field to `CompositeLayer<T>` and `TypedLayerSpec<T2>`
  - Added `erased_z_index()` / `set_erased_z_index()` to `ErasedLayerSpec` trait
  - Added `layer_order` field and `z_index()` / `layer_order()` methods to
    `CompositeChartBuilder`
  - Added `render_order` field to `CompositeChart`, used by `draw()`
  - Added `compute_render_order()` helper with validation
  - 13 new unit tests for ordering logic

- `examples/composite_layer_order.rs` — New example demonstrating z-index
  reordering with three layers (bar, scatter, area) rendered in non-declaration
  order.

### Test Count

- 13 new unit tests for layer ordering
- All 3095 lib tests pass
- 5 doctests pass (including new z_index and layer_order examples)

## Retrospective

**Completed**: 2026-03-06

### Key Technical Learnings

#### Dual Ordering API Design (z-index + explicit order)

- **Challenge**: The story left the API design open — z-index vs explicit
  ordering vs both. Need to decide which approach gives the best user experience
  without over-engineering.
- **Solution**: Implemented both as complementary mechanisms. `z_index()` is
  natural for per-layer priority (set-and-forget), while `layer_order()` is
  precise for cases where the user wants exact control. `layer_order` takes
  precedence when both are used.
- **Pattern**: When two approaches serve different use cases, implement both with
  a clear precedence rule rather than forcing users into one paradigm.

#### Chainable z-index on Last-Added Layer

- **Challenge**: Adding z-index per-layer without combinatorial explosion of
  methods (e.g., `layer_with_z()`, `layer_with_z_y2()`, `layer_with_data_z()`,
  etc.).
- **Solution**: `z_index()` operates on the most recently added layer, so it
  chains naturally after any `layer*()` call:
  `.layer(scatter).z_index(10).layer(line).z_index(5)`.
- **Pattern**: When extending a builder with cross-cutting concerns, a "modify
  last item" method avoids method combinatorics. The trade-off is that calling
  `z_index()` before any layer panics, which is documented.

#### Index Vector vs Physical Reordering

- **Challenge**: Whether to physically reorder the `additional_layers` vec or
  use an indirection vector for rendering.
- **Solution**: Used a `render_order: Vec<usize>` indirection vector as
  suggested in the risk assessment. Layers stay in declaration order for
  consistent indexing; only the draw loop uses the sorted order.
- **Pattern**: Indirection vectors are preferable when the original order has
  semantic meaning (e.g., indices used in `layer_order()` refer to declaration
  positions).

### Architectural Decisions

#### Both z-index and layer_order Rather Than One

- **Decision**: Implement both ordering mechanisms.
- **Reasoning**: z-index is intuitive for CSS/game-dev users; explicit ordering
  is more direct and verifiable. Both are cheap to implement given the
  indirection vector approach.
- **Trade-off**: Two ways to do the same thing; documented that layer_order wins.
- **Future**: If animation or dynamic layer visibility is added, z-index
  is more natural for runtime changes.

#### Type-Erased z-index via Trait Methods

- **Decision**: Added `erased_z_index()` and `set_erased_z_index()` to the
  `ErasedLayerSpec` trait so z-index works on foreign-data layers.
- **Reasoning**: Without this, `z_index()` would silently not work on layers
  added via `layer_with_data()`.
- **Trade-off**: Slightly larger trait surface, but consistent behavior.
- **Future**: Any new per-layer metadata can follow this pattern.

### Development Workflow Insights

- The implementation was straightforward as predicted by the risk assessment —
  adding an indirection vector was a minimal change.
- Disk space constraints required using `CARGO_TARGET_DIR=/tmp/gup-target`
  to avoid filling the home partition during compilation.
- The `compute_render_order` function was extracted as a pure helper, making it
  trivially testable without GPU resources — 10 of the 13 new tests run
  without any GPU involvement.
- Doc-tests for the new API methods compile-check the examples in the
  docstrings, catching the API surface early.
