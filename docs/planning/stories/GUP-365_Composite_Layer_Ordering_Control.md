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
