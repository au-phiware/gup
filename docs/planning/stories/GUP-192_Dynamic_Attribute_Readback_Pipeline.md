# GUP-192: Dynamic Attribute Readback Pipeline

**Status**: 🚧 In Progress **Priority**: Low **Category**: Feature Enhancement
**Estimated Effort**: 1 day **Dependencies**: GUP-186 (Dynamic Attribute GPU
Upload Pipeline)

## Overview

Add async GPU-to-CPU readback for dynamic attribute buffers managed by
`DynamicAttributeBufferManager`. Currently the pipeline only supports upload
(CPU→GPU); this story adds download (GPU→CPU) for debugging, validation, and
CPU-side post-processing of GPU-computed attribute values.

## Context

GUP-186 introduced `DynamicAttributeBufferManager` with automatic buffer
lifecycle, dirty-only uploads, and MarkRenderer integration. However, there is
no way to read attribute data back from the GPU. This is needed for:

- Debugging GPU-computed attributes (e.g., shader-driven values)
- Validating that uploads wrote correct data
- CPU-side analytics on GPU-computed results

## User Story

**As a** visualization developer **I want** to read dynamic attribute values
back from the GPU **So that** I can debug, validate, and post-process
GPU-computed attribute data

## Acceptance Criteria

- [ ] Async readback of uniform buffer contents (static attributes)
- [ ] Async readback of individual storage buffers (per-instance attributes)
- [ ] Staging buffer management for efficient readback
- [ ] Integration with existing `GpuBuffer::download()` patterns

## Technical Tasks

1. Add `download_static_values()` to `DynamicAttributeBufferManager`
2. Add `download_per_instance(name)` to `DynamicAttributeBufferManager`
3. Manage staging buffers for GPU→CPU copy operations
4. Write GPU integration tests for readback correctness

## Testing Strategy

- GPU integration tests: upload → readback → verify values match
- Roundtrip test: upload static, modify on GPU (compute shader), readback
- Performance test: readback should not block rendering pipeline

## Success Metrics

- Readback values match uploaded values exactly
- Staging buffer reuse minimizes allocation overhead

## Risk Assessment

- **Low risk**: follows existing `GpuBuffer::download()` patterns
- Async buffer mapping requires careful lifetime management

## Definition of Done

- [ ] Async readback methods implemented
- [ ] Staging buffer management
- [ ] GPU integration tests pass
- [ ] Documentation with usage examples
