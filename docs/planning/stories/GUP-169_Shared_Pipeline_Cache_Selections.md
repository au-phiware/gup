# GUP-169: Shared Pipeline Cache for Selections

**Status**: 📋 Planned

## Story Overview

**Title**: Extract a reusable PipelineCache shared across Selections **Epic**:
Phase 1 Initiative 3 - Mark System and Type Integration **Priority**: Low
**Story Points**: 3

## Context

GUP-165 has each Selection create its own render pipeline via
`MarkInfoImpl::create_render_pipeline()`. When many Selections of the same mark
type coexist (e.g., a dashboard with multiple scatter plots), each holds its own
pipeline handle. While wgpu may cache compiled shaders internally, a shared
Rust-side cache would avoid redundant pipeline descriptor construction.

## User Story

**As a** library developer building multi-chart dashboards **I want** Selections
of the same mark type to share a single render pipeline **So that** pipeline
creation overhead is eliminated for repeated mark types

## Acceptance Criteria

- [ ] `PipelineCache` struct holds `HashMap<TypeId, Arc<RenderPipeline>>`
- [ ] `Selection::prepare_render()` accepts an optional `&mut PipelineCache`
- [ ] Cache miss creates and caches the pipeline; cache hit returns Arc clone
- [ ] Cache invalidation on device loss or surface format change
- [ ] Benchmark shows reduced pipeline creation time for 100 Selections

## Dependencies

- **Requires**: GUP-165 (Selection API Render Integration) ✅
- **Requires**: GUP-068 (Mark Pipeline Integration) ✅

## Testing Strategy

- Unit tests for cache hit/miss behaviour
- GPU integration test: 10 Selections share one pipeline
- Device loss recovery test: cache cleared and rebuilt

## Definition of Done

- [ ] All acceptance criteria met
- [ ] No regression in existing tests
- [ ] `mask all-fix` clean
