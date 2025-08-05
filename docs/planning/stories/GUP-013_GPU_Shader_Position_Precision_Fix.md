# GUP-013: GPU Shader Position Precision Fix

**Priority**: Medium  
**Complexity**: Medium  
**Created**: 2025-08-05  
**Status**: Open

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

- [ ] GPU shader correctly reads X coordinates from uploaded element data
- [ ] Hit testing precision matches expected element positions exactly
- [ ] All interaction system tests pass with strict equality assertions
- [ ] Performance maintains <1ms for 1M point queries
- [ ] Cross-platform compatibility (native and WebAssembly)

## Investigation Tasks

1. **Data Flow Validation**

   - [ ] Verify Rust `ElementData` struct layout with `std::mem::size_of` and
         alignment
   - [ ] Validate WGSL `ElementData` struct matches Rust layout exactly
   - [ ] Add debug staging buffer downloads to inspect uploaded GPU data

2. **Buffer Management Analysis**

   - [ ] Review buffer creation, upload, and binding code paths
   - [ ] Test with simplified single-element data to isolate the issue
   - [ ] Validate buffer usage flags and alignment requirements

3. **Shader Code Review**
   - [ ] Inspect WGSL array indexing and element access patterns
   - [ ] Test with hardcoded position values in shader for comparison
   - [ ] Verify bind group layout matches buffer structure

## Success Metrics

- Exact position matching in hit testing (tolerance < 0.001)
- All 13 interaction system tests pass without tolerance adjustments
- Performance regression < 5% from current implementation
- Zero position-related test warnings or debug output

## Dependencies

- Requires completed GUP-012 implementation
- May need enhanced GPU debugging tools
- WebGPU specification compliance validation

## Follow-up Stories

- Consider GUP-014 for performance optimization to achieve <1ms target
- Potential GUP-015 for enhanced GPU debugging and profiling tools
