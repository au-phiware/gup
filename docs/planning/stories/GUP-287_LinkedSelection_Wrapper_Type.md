# GUP-287: LinkedSelection Wrapper Type

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 📋 Planned **Created**:
2025-07-19

## Context

GUP-279 introduced `SharedSelectionState<K>` and the `build_dimmed_instances`
free function for linked-view coordination. While functional, the current API
requires manual orchestration in the render loop: the user must call
`has_changed_since`, `build_dimmed_instances`, and `prepare_render` explicitly
for each chart. This story wraps that pattern into a `LinkedSelection<T, M, K>`
type that automatically detects state changes and rebuilds instances.

## User Story

> "As a visualization developer, I want a single type that combines a Selection
> with linked-view state so that I don't need to manually track generation
> counters and rebuild instance buffers on every frame."

## Acceptance Criteria

- [ ] A `LinkedSelection<T, M, K>` type wraps `Selection<T, M>` +
      `SharedSelectionState<K>` + key function + dim opacity
- [ ] `LinkedSelection` exposes a `prepare_render` method that automatically
      checks the generation counter and only rebuilds when the selection state
      has changed
- [ ] The type provides the same `render` method as `Selection` for seamless
      substitution
- [ ] Builder pattern: `LinkedSelection::new(data, shared_state, key_fn)
      .dim_opacity(0.2)`
- [ ] All existing linked_selection tests continue to pass
- [ ] New tests verify automatic rebuild on state change and skip on no change

## Technical Tasks

- [ ] Define `LinkedSelection<T, M, K>` struct
- [ ] Implement `prepare_render` with generation-based change detection
- [ ] Implement `render` delegation to inner `Selection`
- [ ] Add builder methods for configuration
- [ ] Write unit and integration tests
- [ ] Update prelude exports

## Dependencies

### Prerequisite Stories

- GUP-279: Linked View Coordination ✅ — provides SharedSelectionState and
  build_dimmed_instances

## Testing Strategy

- Unit tests for automatic rebuild detection
- Integration test with two LinkedSelections sharing state
- Performance test: verify skip when no change

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
