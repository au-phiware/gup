# GUP-235: Radix Sort Scatter Optimization

**Story ID**: GUP-235 **Title**: Radix Sort Scatter Optimization **Status**: 📋
Planned **Priority**: Low **Effort**: — **Created**: 2025-07-20
**Dependencies**: GUP-184 (GPU Radix Sort for Z-Order)

## Overview

Optimize the scatter pass in the GPU radix sort to replace the
O(workgroup_size²) serial local rank computation with per-digit shared memory
prefix sums for O(n) total work per workgroup.

## Context

GUP-184's radix sort scatter pass computes each thread's local rank (position
within its digit group) by serially scanning all preceding threads in shared
memory. For a 256-thread workgroup, this is 256 comparisons per thread = 65K
operations per workgroup. While correct and stable, this is a performance
bottleneck for large datasets.

## User Story

As a developer sorting millions of instances by Z-depth, I want the sort scatter
pass to be compute-efficient so that sorting overhead stays under 1ms for 1M
instances.

## Acceptance Criteria

- [ ] Scatter pass uses shared memory prefix sums instead of serial scan
- [ ] Sort remains stable (preserves input order for equal keys)
- [ ] Benchmark shows measurable improvement at 1M instances
- [ ] All existing radix sort tests continue to pass

## Technical Tasks

1. Replace serial local rank loop with per-digit exclusive prefix sum in shared
   memory
2. Use a 256-entry shared memory array per digit value (or iterate over digits
   with a single shared array)
3. Verify stability with existing tests
4. Benchmark comparison before/after optimization

## Dependencies

- GUP-184: GPU Radix Sort for Z-Order

## Testing Strategy

- All existing radix sort tests must pass unchanged
- Add benchmark comparing old vs new scatter performance
- Test with large datasets (1M+) to verify no regressions

## Success Metrics

- Sort scatter pass runs in O(workgroup_size) rather than O(workgroup_size²)
- Measurable improvement in sort benchmarks at 1M scale

## Risk Assessment

- **Risk**: Per-digit prefix sums add complexity and may not be faster for small
  workgroups
  - **Mitigation**: Keep the serial approach as a fallback; only optimize if
    benchmarks show improvement

## Definition of Done

- [ ] Optimized scatter pass implemented and tested
- [ ] Benchmark shows improvement
- [ ] No test regressions
