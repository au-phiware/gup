# GUP-365: Composite Layer Ordering Control

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2025-07-19

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

- [ ] A method `.layer_order(order: &[usize])` or equivalent API on
      `CompositeChartBuilder` allows reordering layers by index.
- [ ] Alternatively, layers accept a z-index value that determines rendering
      order (lower z-index = rendered first / behind).
- [ ] The default behaviour (declaration order) is unchanged when no explicit
      ordering is specified.
- [ ] An example demonstrates reordering layers.

## Technical Tasks

- [ ] Design the layer ordering API (z-index vs explicit ordering vs both).
- [ ] Update `CompositeChart::draw()` to respect the ordering.
- [ ] Write unit tests for ordering logic.
- [ ] Add an example demonstrating layer reordering.

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

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
