# GUP-168: Selection Attribute Binding Pipeline

**Status**: 📋 Planned

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

- [ ] `Selection::attr(name, value)` stores attribute bindings
- [ ] `Selection::attr_parallel(composition, names)` stores multi-attribute
      bindings
- [ ] `prepare_render()` can be called without a mapper when attributes are
      bound
- [ ] Attribute bindings compose with the shader function system
- [ ] Type safety: incompatible attribute types produce compile-time errors
- [ ] Example demonstrating declarative attribute binding

## Dependencies

- **Requires**: GUP-165 (Selection API Render Integration) ✅
- **Requires**: GUP-005 (Shader Function System) — for shader function
  composition

## Testing Strategy

- Unit tests for attribute storage and retrieval
- GPU integration test: render circles with bound position and color attributes
- Compile-time type safety tests (should_not_compile patterns)

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Existing Selection tests still pass
- [ ] `mask all-fix` clean
- [ ] Example included
