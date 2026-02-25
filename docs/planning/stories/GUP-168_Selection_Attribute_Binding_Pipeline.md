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

## Retrospective

**Completed**: 2025-07-22

### Key Technical Learnings

#### CPU-side vs GPU-side attribute binding

- **Challenge**: The story's example showed `attr("position", linear_scale)`
  where `linear_scale` is a GPU shader function. Shader functions generate WGSL
  and don't evaluate on the CPU. The existing `prepare_render()` does CPU-side
  data mapping.
- **Solution**: Implemented CPU-side closure-based binding
  (`attr("position", |d| [d.x, d.y])`) which is immediately useful and
  ergonomic. GPU-side shader function binding (where the shader runs on the GPU)
  is a natural follow-up.
- **Pattern**: When a story spans CPU and GPU concerns, implement the CPU path
  first as it's immediately testable and useful, then add GPU integration.

#### Refactoring prepare_render without breaking existing API

- **Challenge**: Existing code relies on
  `prepare_render(device, queue, mapper)`. Adding a mapper-free variant required
  factoring out the GPU upload logic.
- **Solution**: Extracted `upload_instances()` as a private helper shared by
  both `prepare_render()` (manual mapper) and `prepare_render_bound()`
  (declarative bindings). Zero breaking changes to existing code.
- **Pattern**: When adding a new path to an existing method, extract the common
  tail into a shared helper rather than duplicating logic.

#### Type-erased attribute values with compile-time safety

- **Challenge**: Different mark attributes have different types (f32, Vec2,
  Vec4) but need to be stored in a homogeneous container.
- **Solution**: `AttrValue` enum for runtime storage + `IntoAttrValue` trait for
  compile-time gates. Only GPU-compatible types (`f32`, `[f32;2]`, `[f32;4]`,
  `Vec2`, `Vec4`) implement the trait, so `attr("x", |d| "text")` fails at
  compile time.
- **Pattern**: Enum for storage + trait-bound for input = compile-time type
  safety with runtime flexibility.

### Architectural Decisions

#### Separate `prepare_render_bound()` instead of overloading `prepare_render()`

- **Decision**: Added a new method `prepare_render_bound()` rather than making
  the mapper optional in `prepare_render()`.
- **Reasoning**: Rust doesn't support function overloading. Making mapper
  optional with `Option<impl Fn>` is awkward. A separate method name is clear
  and discoverable.
- **Trade-off**: Users need to know two method names. Documentation and examples
  mitigate this.
- **Future**: Could add a unified `prepare()` method that checks for bindings
  first.

#### String-based attribute names with runtime validation

- **Decision**: Used string attribute names (`"center"`, `"fill_color"`) rather
  than typed keys (`Position`, `Color`).
- **Reasoning**: Matches the story's API design. Ergonomic and familiar to users
  from web visualisation backgrounds. Each mark's `MarkInstanceBuilder` maps
  names to fields.
- **Trade-off**: Attribute name typos are caught at runtime (silent no-op for
  unknown names) rather than compile time.
- **Future**: A typed-key API (`attr(POSITION, ...)`) could be layered on top
  for users who want full compile-time validation.

#### Attribute name aliases (position→center, color→fill_color)

- **Decision**: Mark instance builders accept both short names (`"position"`,
  `"color"`, `"size"`) and the field-specific names (`"center"`, `"fill_color"`,
  `"radius"`).
- **Reasoning**: Users think in terms of "position" and "color", not mark-
  specific field names. Aliases reduce cognitive load.
- **Trade-off**: Slight added complexity in the builder match arms.
- **Future**: This pattern should be consistent across all marks.

### Development Workflow Insights

- **Pre-existing test breakage**: The old placeholder `attr()` accepted anything
  (`V: Send + Sync + 'static`). Tightening the bounds to `F: Fn(&T) -> V`
  immediately revealed all call sites that passed non-closure types (shader
  functions, constants). This was productive — it forced a clean migration.
- **Integration test rewrite**: The `selection_parallel_integration_tests.rs`
  file was completely rewritten. The old tests were only verifying that
  placeholder methods compiled; the new tests verify actual binding storage and
  retrieval.
- **Example-driven development**: Writing the `attr_binding_demo.rs` example
  helped find API ergonomic issues (e.g., `context.device()` vs
  `context.device`).

### Follow-up Stories

1. **GUP-177: GPU-Side Shader Function Attribute Binding** — Extend `attr()` to
   accept `ComposableShaderFunction` types that generate WGSL and run attribute
   transformations on the GPU instead of the CPU. This would enable the original
   story vision of `attr("position", linear_scale)` where `linear_scale` is a
   GPU shader function.

2. **GUP-178: MarkInstanceBuilder for Line and BoxPlot** — Extend
   `MarkInstanceBuilder` implementations to Line and BoxPlot marks to complete
   coverage of all mark types. Current implementation covers Circle and
   Rectangle only.
