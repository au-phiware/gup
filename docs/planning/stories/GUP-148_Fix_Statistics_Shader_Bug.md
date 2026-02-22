# GUP-148: Fix Statistics Compute Shader Reduction Bug

**Status**: 💡 New

## Story Overview

**Title**: Fix Workgroup Reduction Bug in Statistics Compute Shader  
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping  
**Priority**: High  
**Story Points**: 3

## Context

GUP-145 discovered a critical bug in the statistics compute shader from GUP-139. The workgroup reduction algorithm returns incorrect count values (returns `workgroup_size` instead of actual data count), which causes all statistical calculations to be wrong.

## User Story

**As a** library maintainer  
**I want** the statistics compute shader to correctly aggregate values  
**So that** GPU statistical computations are accurate and reliable

## Acceptance Criteria

### AC1: Fix Reduction Logic

- [ ] Debug and identify root cause of count aggregation bug
- [ ] Fix workgroup reduction to correctly aggregate only valid data
- [ ] Verify shared memory initialization is correct
- [ ] Test workgroup barriers are properly placed

### AC2: Validate Fix

- [ ] All 14 GPU integration tests from GUP-145 pass
- [ ] Test with various dataset sizes (5, 100, 10K, 1M elements)
- [ ] Verify correct behavior at workgroup boundaries (256, 257, 512 elements)
- [ ] CPU and GPU results match within floating-point epsilon

### AC3: Multi-Workgroup Support

- [ ] Extend shader to support multiple workgroups (for large datasets)
- [ ] Implement proper atomic operations for cross-workgroup aggregation
- [ ] Test with datasets requiring 2, 10, 100+ workgroups

## Technical Investigation Needed

### Known Symptoms

1. `shared_count[0]` after reduction contains 256 instead of 5 (for 5-element dataset)
2. Single-thread writes work correctly (hardcoded 42 writes as 42)
3. Thread initialization appears correct (valid threads count=1, invalid count=0)
4. Parallel reduction tree logic appears sound
5. Bug persists after clean rebuild

### Debugging Approaches

1. **Shared Memory Inspection**: Use GPU debugging tools to inspect shared memory contents
2. **Reduction Step Tracing**: Verify each stride of the reduction loop
3. **Workgroup Barrier Validation**: Ensure barriers are correct and not optimized away
4. **GPU Profiler**: Use vendor tools (NSight, RenderDoc, PIX) to trace execution
5. **Simplified Test**: Create minimal reproduction shader

### Potential Root Causes

- Workgroup barrier placement or synchronization issue
- Shared memory initialization race condition  
- Compiler optimization removing necessary synchronization
- GPU driver bug (less likely but possible)
- Incorrect loop bounds in reduction algorithm

## Dependencies

- **Requires**: GUP-145 (GPU Statistics Integration Tests) - ⚠️ Partial
- **Blocks**: GPU statistical operations being usable
- **Enables**: Reliable GPU-accelerated data analysis

## Testing Strategy

- Use existing 14 GPU integration tests from GUP-145
- Add reduction-specific unit tests
- Test on multiple GPU backends (Vulkan, Metal, DX12)
- Benchmark performance after fix

## Success Metrics

- All GUP-145 tests pass (currently 1/14 passing)
- GPU results match CPU within 0.001 epsilon for floating point
- Performance is 10-100x faster than CPU for 1M+ element datasets
- No regressions in shader compilation or memory layout

## Risk Assessment

**Medium Risk**: Shader debugging can be time-consuming without proper GPU profiling tools.

**Mitigation**: Start with simplified reproduction case, use multiple debugging approaches.

## Definition of Done

- [ ] Root cause identified and documented
- [ ] Shader fix implemented and tested
- [ ] All 14 GPU integration tests pass
- [ ] Multi-workgroup support added
- [ ] Performance validated (GPU faster than CPU for large datasets)
- [ ] Code review completed
- [ ] Documentation updated with findings

---

_Created from GUP-145 when shader bug was discovered during GPU integration test implementation._
