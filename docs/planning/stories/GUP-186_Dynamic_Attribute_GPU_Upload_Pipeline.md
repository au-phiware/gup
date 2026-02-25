# GUP-186: Dynamic Attribute GPU Upload Pipeline

**Status**: 🚧 In Progress **Priority**: Medium **Category**: Feature
Enhancement **Estimated Effort**: 2 days **Dependencies**: GUP-069 (Advanced
Mark Rendering Features)

## Overview

Build the complete GPU upload pipeline for `DynamicAttributeMap`, including
automatic buffer management, dirty-only uploads, and integration with the
rendering loop. GUP-069 provides the data structures but stops at the
`collect_static_values()` level.

## Context

GUP-069 introduced `DynamicAttributeMap` with dirty tracking, generation
counters, and `DynamicAttributeValue` variants (Static, PerInstance,
ShaderDriven). The current implementation collects values on the CPU side but
does not manage GPU buffer allocation, dirty-only partial uploads, or automatic
integration with the `MarkRenderer` rendering loop.

## User Story

**As a** visualization developer **I want** dynamic attributes to automatically
upload to the GPU when changed **So that** I can update mark properties at
runtime without manual buffer management

## Acceptance Criteria

- [ ] Automatic GPU buffer creation when attributes are first set
- [ ] Dirty-only upload: only changed attributes are re-uploaded to GPU
- [ ] Per-instance data uploaded to storage buffers
- [ ] Static data uploaded to uniform buffers
- [ ] Integration with `MarkRenderer` so dynamic attributes are bound during
      rendering
- [ ] Performance: attribute updates + GPU upload < 1ms for typical cases

## Technical Tasks

1. Create `DynamicAttributeBufferManager` that allocates and manages GPU buffers
2. Implement dirty-only upload logic using
   `DynamicAttributeMap::dirty_attributes()`
3. Integrate with `MarkRenderer::render_marks()` to bind dynamic attribute
   buffers
4. Add performance benchmarks for attribute update + upload cycle

## Testing Strategy

- GPU integration tests for buffer allocation and upload
- Performance tests validating <1ms update cycle
- Integration test with `MarkRenderer` end-to-end

## Success Metrics

- Dirty-only uploads reduce GPU bandwidth by >50% vs full re-upload
- Attribute update + upload cycle < 1ms for 100 attributes
- Zero regression in existing mark rendering performance

## Risk Assessment

- **Medium risk**: buffer management requires careful alignment and sizing
- Must handle buffer resizing when per-instance data grows

## Definition of Done

- [ ] Automatic buffer management for dynamic attributes
- [ ] Dirty-only upload implemented and tested
- [ ] Integration with MarkRenderer rendering loop
- [ ] Performance benchmarks pass
- [ ] All existing tests continue to pass
