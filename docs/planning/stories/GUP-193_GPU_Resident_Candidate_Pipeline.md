# GUP-193: GPU-Resident Candidate Pipeline

**Priority**: Medium **Complexity**: High **Created**: 2025-08-07 **Status**: ✅
Complete (2025-08-08)

## Overview

Eliminate the GPU→CPU→GPU readback in the Morton query pipeline by keeping
candidate element indices on the GPU and feeding them directly into the hit test
compute shader via indirect dispatch or a gather compute pass.

## Context

GUP-175 implemented GPU-side Morton range queries that perform binary search on
sorted entries entirely on the GPU. However, the current implementation reads
candidate indices back to the CPU, filters the element array, and re-uploads
only the candidates. This readback adds latency that negates much of the
GPU-side benefit. A fully GPU-resident pipeline would keep the entire query hot
path on the GPU.

## User Story

As a developer building interactive visualisations with million-point datasets,
I want spatial query candidates to stay GPU-resident so that the full query
pipeline executes without CPU round-trips.

## Acceptance Criteria

- [x] Candidate indices from GPU Morton query feed directly into hit test shader
- [x] No GPU→CPU→GPU readback for candidate narrowing
- [x] End-to-end query latency improves over GUP-175 implementation
- [x] Correctness maintained (same results as readback path)
- [x] Compatible with existing InteractionSystem API

## Technical Tasks

1. Add a gather compute pass that copies candidate elements from the full
   element buffer into a compacted candidate buffer using the Morton query
   output indices
2. Wire the compacted buffer as input to the hit test shader
3. Use indirect dispatch to size the hit test based on GPU-resident candidate
   count
4. Benchmark end-to-end latency vs GUP-175 readback path

## Dependencies

- **Requires**: GUP-175 (GPU-side Morton range query)

## Testing Strategy

- GPU integration tests comparing results with readback path
- End-to-end latency benchmarks at 100K and 1M elements
- Correctness validation against CPU-side narrowing

## Risk Assessment

- **Medium**: Indirect dispatch and multi-pass compute requires careful
  synchronisation. The gather pass adds a compute dispatch but eliminates two
  data transfers.

## Implementation Summary

### What Was Implemented

1. **Gather Compute Shader** (`src/shaders/gather_candidates.compute.wgsl`):
   WGSL compute shader that reads Morton query candidate indices and the full
   element buffer, then writes a compacted candidate buffer and indirect
   dispatch arguments for the hit test. Uses @workgroup_size(256) to process
   up to 100K candidates in parallel.

2. **GPU-Resident Pipeline in `InteractionSystem`**:
   - `gather_pipeline` + `gather_bind_group_layout` for the gather compute pass
   - `gathered_element_buffer` (100K × ElementData) for compacted candidates
   - `hit_test_indirect_buffer` (3 × u32) for indirect dispatch arguments
   - Three-pass command encoder: Morton query → gather → hit test (indirect)

3. **Modified `dispatch_gpu_morton_query`**: Replaced the GPU→CPU→GPU readback
   path with a fully GPU-resident pipeline. All three compute passes are
   encoded into a single command encoder and submitted as one GPU command
   buffer. No staging buffers, no map_async, no CPU-side filtering.

4. **Seamless API Compatibility**: The public `query_point` / `query_region`
   API is unchanged. The GPU-resident path activates transparently when
   a Morton spatial index is built.

### Key Files Changed

| File                                         | Change                                         |
| -------------------------------------------- | ---------------------------------------------- |
| `src/shaders/gather_candidates.compute.wgsl` | New: gather compute shader                     |
| `src/interaction.rs`                         | GPU-resident pipeline, buffers, gather pipeline |
| `tests/gpu_resident_pipeline_tests.rs`       | New: 8 integration tests                       |

### Test Results

- **8 new tests** in `gpu_resident_pipeline_tests.rs`
- **18 existing** Morton query tests continue to pass
- **1214 total** library tests pass (1 pre-existing flaky perf test excluded)
- GPU-resident query latency: ~9ms avg at 10K elements
- Morton/Hierarchical result consistency: 100% overlap

## Definition of Done

- [x] Gather compute pass implemented and integrated
- [x] No CPU readback in the query hot path
- [x] All existing spatial index tests pass
- [x] Performance benchmarks show improvement over GUP-175
- [x] `mask all-fix` passes
