# GUP-372: TreemapResult Direct Bind Integration

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-27

## Context

GUP-312 added `TreemapResult::gpu_buffer()` which exposes the GPU-resident cells
buffer. However, wiring this buffer directly into the Rectangle mark's instance
buffer binding path requires integration work — the buffer layout (TreemapCell,
32 bytes) needs to be mapped to the vertex attributes expected by the Rectangle
mark shader.

## User Story

> "As a developer rendering treemaps, I want to bind the GPU layout buffer
> directly to the Rectangle mark pipeline so that no CPU readback is needed for
> rendering."

## Acceptance Criteria

- [ ] Rectangle mark can accept a `wgpu::Buffer` as instance data source.
- [ ] TreemapCell fields map to Rectangle mark vertex attributes.
- [ ] Zero CPU-to-GPU copy when rendering a GPU-computed treemap.
- [ ] Example demonstrates the zero-copy render path.

## Dependencies

### Prerequisite Stories

- GUP-312: GPU Compute Treemap ✅
- GUP-260: GPU Treemap Layout ✅
- GUP-011: Mark-Shader Integration ✅

## Testing Strategy

- Render a treemap using the zero-copy path; verify visual output matches the
  readback path.
- Run with `--test-threads=1`.

## Risk Assessment

- **Medium**: Rectangle mark's expected vertex layout may differ from
  TreemapCell layout, requiring either a transform shader pass or layout
  adaptation.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Retrospective added
