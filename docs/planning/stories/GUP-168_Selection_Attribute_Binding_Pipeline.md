# GUP-168: Selection Attribute Binding Pipeline

**Status**: ✅ Complete (2025-07-22)

## Story Overview

**Title**: Implement attr() / attr_parallel() for automatic instance generation
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping **Priority**: Medium
**Story Points**: 8

## Context

GUP-165 added `Selection::prepare_render(device, queue, mapper)` which requires
the caller to provide a manual mapper closure (e.g.,
`|a| CircleInstance::from(a)`). The Selection already has placeholder `attr()`
and `attr_parallel()` methods that could automatically compose shader functions
into GPU instance data.

This story implements the attribute binding pipeline so users can write:

```rust
selection
    .attr("position", linear_scale)
    .attr("color", color_map)
    .prepare_render(&device, &queue)?;
```

instead of manually converting data to instances.

## User Story

**As a** library user building visualisations **I want** to bind data attributes
to mark properties declaratively **So that** I don't need to manually construct
GPU instance structs

## Acceptance Criteria

- [x] `Selection::attr(name, value)` stores attribute bindings
- [x] `Selection::attr_parallel(composition, names)` stores multi-attribute
      bindings
- [x] `prepare_render()` can be called without a mapper when attributes are
      bound
- [x] Attribute bindings compose with the shader function system
- [x] Type safety: incompatible attribute types produce compile-time errors
- [x] Example demonstrating declarative attribute binding

## Dependencies

- **Requires**: GUP-165 (Selection API Render Integration) ✅
- **Requires**: GUP-005 (Shader Function System) — for shader function
  composition

## Testing Strategy

- Unit tests for attribute storage and retrieval
- GPU integration test: render circles with bound position and color attributes
- Compile-time type safety tests (should_not_compile patterns)

## Definition of Done

- [x] All acceptance criteria met
- [x] Existing Selection tests still pass
- [x] `mask all-fix` clean
- [x] Example included

## Implementation Summary

### What was implemented

- **`AttrValue` enum** — type-erased GPU-compatible attribute value
  (`Float`/`Vec2`/`Vec4`)
- **`IntoAttrValue` trait** — compile-time type safety for attribute binding
  values (`f32`, `[f32;2]`, `[f32;4]`, `Vec2`, `Vec4`)
- **`IntoAttrValues<T, N>` trait** — parallel tuple extraction for 2 and 3
  element tuples
- **`MarkInstanceBuilder` trait** — marks implement this to build GPU instances
  from named attribute bindings
- **`Selection::attr(name, closure)`** — stores named binding closures, replaces
  placeholder
- **`Selection::attr_parallel(closure, names)`** — stores multi-attribute
  bindings from tuple closures
- **`Selection::prepare_render_bound(device, queue)`** — mapper-free GPU upload
  using stored bindings
- **`Selection::bound_attributes()`** / **`has_attr_bindings()`** — query
  methods
- **`MarkInstanceBuilder` for `Circle`** — supports center/position,
  radius/size, fill_color/color, stroke_width, stroke_color
- **`MarkInstanceBuilder` for `Rectangle`** — supports center/position, size,
  fill_color/color, stroke_width, stroke_color, corner_radius
- Removed `PositionShaderFunction` / `ColorShaderFunction` placeholder types
- Updated chart_builder and parallel_composition_demo example to use closure API

### Key files changed

| File                                            | Change                                      |
| ----------------------------------------------- | ------------------------------------------- |
| `src/selection.rs`                              | Core attr binding types and Selection impls |
| `src/mark/circle.rs`                            | MarkInstanceBuilder for Circle              |
| `src/mark/rectangle.rs`                         | MarkInstanceBuilder for Rectangle           |
| `src/lib.rs`                                    | Updated public API exports                  |
| `src/prelude.rs`                                | Updated prelude re-exports                  |
| `src/chart_builder/builders.rs`                 | Migrated to closure-based attr()            |
| `examples/attr_binding_demo.rs`                 | New example (4 scenarios)                   |
| `examples/parallel_composition_demo.rs`         | Updated to closure-based API                |
| `tests/selection_parallel_integration_tests.rs` | Rewritten for new API (7 tests)             |

### Test counts

- **31 selection tests** (26 original + 5 new GPU binding tests)
- **7 mark instance builder tests** (4 Circle + 3 Rectangle)
- **7 integration tests** (rewritten parallel binding tests)
