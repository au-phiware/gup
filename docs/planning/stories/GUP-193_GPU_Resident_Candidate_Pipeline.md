# GUP-193: GPU-Resident Candidate Pipeline

**Priority**: Medium **Complexity**: High **Created**: 2025-08-07 **Status**: 📋
Planned

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

As a developer building interactive visualisations with million-point datasets, I
want spatial query candidates to stay GPU-resident so that the full query
pipeline executes without CPU round-trips.

## Acceptance Criteria

- [ ] Candidate indices from GPU Morton query feed directly into hit test shader
- [ ] No GPU→CPU→GPU readback for candidate narrowing
- [ ] End-to-end query latency improves over GUP-175 implementation
- [ ] Correctness maintained (same results as readback path)
- [ ] Compatible with existing InteractionSystem API

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

## Definition of Done

- [ ] Gather compute pass implemented and integrated
- [ ] No CPU readback in the query hot path
- [ ] All existing spatial index tests pass
- [ ] Performance benchmarks show improvement over GUP-175
- [ ] `mask all-fix` passes
