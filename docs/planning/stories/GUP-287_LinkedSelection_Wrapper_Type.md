# GUP-287: LinkedSelection Wrapper Type

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: ✅ Complete **Created**:
2025-07-19 **Completed**: 2025-07-20

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

- [x] A `LinkedSelection<T, M, K>` type wraps `Selection<T, M>` +
      `SharedSelectionState<K>` + key function + dim opacity
- [x] `LinkedSelection` exposes a `prepare_render` method that automatically
      checks the generation counter and only rebuilds when the selection state
      has changed
- [x] The type provides the same `render` method as `Selection` for seamless
      substitution
- [x] Builder pattern:
      `LinkedSelection::new(data, shared_state, key_fn).dim_opacity(0.2)`
- [x] All existing linked_selection tests continue to pass
- [x] New tests verify automatic rebuild on state change and skip on no change

## Technical Tasks

- [x] Define `LinkedSelection<T, M, K>` struct
- [x] Implement `prepare_render` with generation-based change detection
- [x] Implement `render` delegation to inner `Selection`
- [x] Add builder methods for configuration
- [x] Write unit and integration tests
- [x] Update prelude exports

## Dependencies

### Prerequisite Stories

- GUP-279: Linked View Coordination ✅ — provides SharedSelectionState and
  build_dimmed_instances

## Testing Strategy

- Unit tests for automatic rebuild detection
- Integration test with two LinkedSelections sharing state
- Performance test: verify skip when no change

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

1. **`Selection::prepare_render_raw`** — New public method on `Selection<T, M>`
   that accepts pre-built instance data (`&[I]`) instead of a mapper closure.
   This enables external instance construction (e.g. with dimming applied)
   without exposing the private `upload_instances` method.

2. **`LinkedSelection<T, M, K>`** struct in `src/linked_selection.rs`:
   - Wraps `Selection<T, M>` + `SharedSelectionState<K>` + boxed key function +
     dim opacity + last-seen generation counter
   - `new(data, shared_state, key_fn)` — creates from data
   - `from_selection(selection, shared_state, key_fn)` — wraps existing
     Selection
   - `dim_opacity(f32)` — builder method (default 0.2)
   - `prepare_render(device, queue, mapper, cache, pool)` — automatic
     generation-based change detection; only rebuilds when shared state changed
     or render state is missing
   - `render(render_pass)` — delegates to inner Selection
   - Accessor methods: `data()`, `selection()`, `selection_mut()`,
     `shared_state()`, `set_data()`, `is_render_ready()`, `last_generation()`

3. **Prelude export** — `LinkedSelection` added to `src/prelude.rs`

### Key Files Changed

| File                              | Change                                      |
| --------------------------------- | ------------------------------------------- |
| `src/linked_selection.rs`         | Added `LinkedSelection` struct (~200 lines) |
| `src/selection.rs`                | Added `prepare_render_raw` method           |
| `src/prelude.rs`                  | Added `LinkedSelection` to exports          |
| `tests/linked_selection_tests.rs` | 5 GPU integration tests (new file)          |

### Test Results

- 32 linked_selection unit tests pass (26 existing + 6 new)
- 5 GPU integration tests pass
- 3228 total tests pass across the project
