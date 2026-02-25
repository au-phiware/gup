# GUP-188: Automatic Draw Call Metrics in MarkRenderer

**Status**: 📋 Planned
**Priority**: Low
**Category**: Performance / Developer Experience
**Estimated Effort**: 0.5 days
**Dependencies**: GUP-070 (Mark Performance Optimization)

## Overview

Add a `render_marks_tracked(&mut self, ...)` variant to `MarkRenderer` that
automatically increments `MarkPerformanceMetrics` counters (draw calls,
instances rendered, pipeline switches) without requiring callers to manually
accumulate statistics.

## Context

GUP-070 added `MarkPerformanceMetrics` and `metrics_mut()` to `MarkRenderer`,
but the existing `render_marks()` method takes `&self` and cannot update the
metrics struct. Users who want per-frame metrics must manually call
`metrics_mut().draw_calls += 1` after each render, which is error-prone.

## User Story

**As a** visualization developer using the mark renderer
**I want** draw call and instance metrics to be tracked automatically
**So that** I can monitor rendering performance without manual bookkeeping

## Acceptance Criteria

- [ ] `render_marks_tracked()` takes `&mut self` and updates metrics
- [ ] Draw call count, instance count, and pipeline switch count are tracked
- [ ] Existing `render_marks(&self)` API is unchanged for backward compatibility
- [ ] Performance overhead of tracking is <1% (simple counter increments)

## Technical Tasks

1. Add `render_marks_tracked<M: Mark>(&mut self, ...)` method
2. Track `draw_calls += 1` and `total_instances += instance_count`
3. Add corresponding `render_marks_with_patterns_tracked()` variant
4. Add unit tests verifying counter accuracy

## Testing Strategy

- Unit test: render N batches, verify `metrics.draw_calls == N`
- Unit test: render M instances, verify `metrics.total_instances == M`
- Benchmark: verify <1% overhead vs non-tracked path

## Risk Assessment

- **Low risk**: Additive API change with no breaking changes

## Definition of Done

- [ ] `render_marks_tracked()` implemented and tested
- [ ] Documentation updated with metrics usage examples
- [ ] Benchmark confirms negligible overhead
- [ ] All existing tests pass
