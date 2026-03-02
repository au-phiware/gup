# GUP-286: GPU-Accelerated Brush Region Query

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 📋 Planned **Created**:
2025-07-25

## Context

GUP-278 implemented brush selection using CPU-based hit testing via
`MarkSelectionSystem::filter_by_rect`, which iterates all mark positions
sequentially. This is correct and performant for datasets up to ~100K marks. For
larger datasets (500K–1M+ marks), the GPU interaction pipeline from GUP-012
should be used instead. `MarkSelectionSystem` already provides
`rect_hit_test_gpu` which dispatches a compute-shader region query — this story
wires that into `BrushBehavior::on_pointer_up` as an async path with a CPU
fallback.

## User Story

> "As a visualization developer working with large datasets (500K+ marks), I
> want the brush region query to use the GPU so that selection completes in
> under 16ms even for million-point datasets."

## Acceptance Criteria

- [ ] When a `MarkSelectionSystem` with an initialised `InteractionSystem` is
      provided, `BrushBehavior::on_pointer_up` uses `rect_hit_test_gpu`.
- [ ] Falls back to `filter_by_rect` (CPU) when no GPU interaction system is
      available.
- [ ] Region query completes within 16ms for 1M marks.
- [ ] No GPU validation errors.
- [ ] A benchmark test compares CPU vs GPU paths for 500K marks.

## Technical Tasks

- [ ] Add an `on_pointer_up_async` method (or make `on_pointer_up` accept a
      future) that dispatches `rect_hit_test_gpu`.
- [ ] Implement timeout logic: if the GPU query does not complete within a
      configurable threshold (default 50ms), fall back to CPU.
- [ ] Add a benchmark comparing CPU and GPU region query performance.
- [ ] Update the example to demonstrate GPU-accelerated selection.

## Dependencies

### Prerequisite Stories

- GUP-278: Brush Mark for Rectangular Selection ✅
- GUP-012: GPU Interaction System ✅
- GUP-075: Interactive Mark Selection ✅

## Testing Strategy

- Integration test: Simulate brush on 1M synthetic marks, verify GPU path
  returns correct IDs.
- Performance benchmark: CPU vs GPU for 100K, 500K, 1M marks.

## Risk Assessment

- **Medium**: Async GPU query must complete before event handlers fire. May need
  to block or use a callback pattern.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
