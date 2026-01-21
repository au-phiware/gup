# GUP-013: GPU Shader Position Precision Fix

**Priority**: Medium  
**Complexity**: Medium  
**Created**: 2025-08-05  
**Completed**: 2025-08-05  
**Status**: ✅ Complete

## Problem Statement

The GPU interaction system (GUP-012) has a precision issue where the WGSL
compute shader sees incorrect X coordinates for element positions. All elements
appear to have X=0 while Y coordinates are correct, causing inaccurate hit
testing results.

## Current Behavior

- Element data in Rust: `position=[50.0, 50.0]` ✓
- GPU shader sees: `position=[0.0, 50.0]` ❌
- Hit testing works but with wrong positions
- Tests made tolerant of this issue but precision is compromised

## Root Cause Analysis Needed

**Potential Issues:**

1. **Memory Alignment**: Rust struct layout might not match WGSL struct layout
2. **Buffer Upload**: Data corruption during GPU buffer writing
3. **Shader Indexing**: Incorrect array indexing or buffer binding in WGSL
4. **Type Casting**: `bytemuck` conversion issues between Rust and GPU data

## Acceptance Criteria

- [x] GPU shader correctly reads X coordinates from uploaded element data
- [x] Hit testing precision matches expected element positions exactly
- [x] All interaction system tests pass with strict equality assertions
- [x] Performance maintains <5% overhead (target <1ms for 1M points achieved
      separately)
- [x] Cross-platform compatibility (native and WebAssembly)

## Investigation Tasks

1. **Data Flow Validation**
   - [x] Verify Rust `ElementData` struct layout with `std::mem::size_of` and
         alignment
   - [x] Validate WGSL `ElementData` struct matches Rust layout exactly
   - [x] Add debug staging buffer downloads to inspect uploaded GPU data

2. **Buffer Management Analysis**
   - [x] Review buffer creation, upload, and binding code paths
   - [x] Test with simplified single-element data to isolate the issue
   - [x] Validate buffer usage flags and alignment requirements

3. **Shader Code Review**
   - [x] Inspect WGSL array indexing and element access patterns
   - [x] Test with hardcoded position values in shader for comparison
   - [x] Verify bind group layout matches buffer structure

## Success Metrics

- ✅ Exact position matching in hit testing (tolerance < 0.001)
- ✅ All 12 interaction system tests pass without tolerance adjustments
- ✅ Performance regression < 5% from current implementation
- ✅ Zero position-related test warnings or debug output

## Dependencies

- ✅ Requires completed GUP-012 implementation
- ✅ May need enhanced GPU debugging tools (created comprehensive debug
  infrastructure)
- ✅ WebGPU specification compliance validation

## Implementation Summary

**Root Cause Identified**: Struct field alignment mismatches between Rust and
WGSL memory layouts caused `vec2<f32>` fields to be misaligned, corrupting X
coordinates during GPU data transfer.

**Key Fixes Applied**:

1. **GpuInteractionQuery**: Moved `position` field to offset 8 (8-byte aligned)
2. **InteractionResult**: Moved `intersection_point` field to offset 16 (16-byte
   aligned)
3. **WGSL Struct Updates**: Updated compute shader struct definitions to match
   Rust layouts
4. **Buffer Usage**: Added `COPY_SRC` flag for debug buffer inspection
   capabilities
5. **Test Updates**: Replaced tolerance-based assertions with strict equality
   checks

**Validation Results**:

- All 12 interaction system tests pass with exact precision
- Examples run successfully (context_demo, buffer_demo, shader_pipeline_demo)
- Performance impact <5% (within acceptable limits)
- Debug infrastructure preserved for future GPU development

## Follow-up Stories

- GUP-014 for performance optimization to achieve <1ms target for 1M+ points
- GUP-015 for enhanced GPU debugging and profiling tools (partially addressed in
  this story)

## Retrospective

### What Went Well

- **Systematic Investigation**: Layer-by-layer debugging approach effectively
  isolated the root cause
- **Debug Infrastructure**: Comprehensive staging buffer inspection tools
  accelerated problem resolution
- **Test Coverage**: Existing test suite caught the precision issue and
  validated the fix
- **Performance**: Achieved precision fix without significant performance
  degradation

### Key Learnings

- **Memory Alignment Critical**: GPU-CPU data transfer requires exact struct
  field alignment between Rust and WGSL
- **WGSL Alignment Rules**: `vec2<f32>` fields require 8-byte or 16-byte
  alignment boundaries depending on context
- **Debugging Strategy**: GPU bugs often manifest as data layout issues rather
  than logic errors
- **Testing Strategy**: Start with tolerance-based tests during investigation,
  then move to strict equality once precision is achieved

### Patterns for Future GPU Development

- Always validate struct layouts with `std::mem::offset_of!()` during GPU data
  structure design
- Include `COPY_SRC` buffer usage flags during development for debug
  capabilities
- Test GPU code with `--test-threads=1` to avoid resource conflicts
- Layer debugging approach: Rust data → GPU upload → shader processing → result
  download

### Technical Debt Addressed

- Removed tolerance-based test assertions that were masking precision issues
- Cleaned up debug output while preserving debug infrastructure
- Updated all GPU interaction tests to use strict precision requirements
