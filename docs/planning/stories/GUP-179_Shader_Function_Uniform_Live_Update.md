# GUP-179: Shader Function Uniform Live Update

**Status**: 🚧 In Progress

## Story Overview

**Title**: Live update of shader function parameters without data re-upload
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping **Priority**: Medium
**Story Points**: 5

## Context

GUP-177 implemented GPU-side shader function attribute bindings where raw data
values are uploaded to the GPU and shader functions transform them in the vertex
shader. Currently, changing shader function parameters (e.g., adjusting a
`LinearScale` domain) requires calling `prepare_render_bound()` again, which
re-runs all CPU extractors and re-uploads instance data.

The key performance advantage of GPU shader function bindings is that re-mapping
— changing how data maps to visual properties — should only require updating the
shader function's uniform buffer, not re-uploading all instance data. This story
adds that capability.

## User Story

**As a** library user exploring large datasets interactively **I want** to
change shader function parameters (e.g., scale domain/range) without
re-uploading instance data **So that** re-mapping is near-instantaneous for
100K+ point datasets

## Acceptance Criteria

- [ ] New method `update_shader_uniforms(name, new_shader_fn)` on Selection
- [ ] Method only re-uploads the uniform buffer for the named attribute
- [ ] Instance data is NOT re-uploaded (only uniform buffer writes)
- [ ] Performance: uniform update completes in <1ms for any dataset size
- [ ] Error handling: returns error if attribute name not found or not GPU-bound

## Dependencies

- **Requires**: GUP-177 (GPU Shader Function Attribute Binding) ✅

## Testing Strategy

- Unit test: verify only uniform buffer is updated (no instance re-upload)
- GPU integration test: update shader params and render in same frame
- Performance test: compare uniform update time vs full prepare_render time

## Risk Assessment

- **Low risk**: The uniform buffers are already stored in SelectionRenderState;
  this is primarily a convenience API over `queue.write_buffer()`.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Existing Selection tests still pass
- [ ] `mask all-fix` clean
- [ ] Performance comparison included
