# GUP-232: Fix Pre-existing Mark Renderer Metric Test Failures

**Priority**: Medium **Complexity**: Low **Created**: 2026-02-27 **Status**: ✅
Complete (2025-07-18)

## Overview

Three tests in `mark::renderer::tests` consistently fail:

- `test_non_tracked_render_does_not_update_metrics`
- `test_render_marks_tracked_accumulates_across_frames`
- `test_render_marks_tracked_draw_calls`

These failures appear to be related to the draw call metric tracking system
introduced in GUP-188. They were observed during GUP-173 implementation and
confirmed to be pre-existing (present before any GUP-173 changes).

## Context

The `MarkRenderer` has a draw call tracking system that records metrics when
rendering marks. These tests verify that metrics are correctly accumulated and
that non-tracked renders don't affect metrics. The failures suggest a mismatch
between how the tests set up tracking and how the renderer actually records
metrics.

## User Story

As a developer, I want all existing tests to pass so that CI pipelines provide
reliable feedback on regressions.

## Acceptance Criteria

- [x] All three `mark::renderer::tests` tests pass
- [x] No regressions in other mark renderer tests
- [x] Root cause documented

## Technical Tasks

- [x] Reproduce and diagnose the 3 test failures
- [x] Fix the metric tracking logic or update the tests
- [x] Verify fix doesn't affect other mark renderer behaviour

## Dependencies

- **Requires**: GUP-188 (Automatic Draw Call Metrics in MarkRenderer)

## Testing Strategy

- Run `cargo test mark::renderer::tests -- --test-threads=1 --nocapture`
- Verify all 12 mark renderer tests pass
- Run full test suite to check for regressions

## Risk Assessment

- **Low**: Isolated to metric tracking, unlikely to affect rendering

## Definition of Done

- [x] All mark renderer tests pass
- [x] Full test suite passes with no new failures

## Implementation Summary

### Root Cause

The test failures were **not** in the metric tracking logic itself, but in the
test helper `create_circle_render_context`. The bind group layout for custom-
shader marks (like Circle) includes two bindings:

- Binding 0: Instance data storage buffer
- Binding 1: Viewport dimensions uniform buffer

The test helper only provided binding 0 (the instance buffer) and passed an
empty `uniform_buffers` slice to `MarkRegistry::create_bind_group()`. When wgpu
validated the bind group against the layout, it failed because binding 1 was
missing. This caused all three tests that depended on `create_circle_render_context`
to fail.

The same issue also affected two integration test files.

### Fix

Added a `ViewportUniforms` buffer (8 bytes: width + height as f32) to the test
helper and passed it as uniform_buffers[0] when creating the bind group.

### Files Changed

- `src/mark/renderer.rs` — Fixed `create_circle_render_context` test helper
- `tests/mark_pipeline_integration_tests.rs` — Fixed `test_bind_group_creation`
  and `test_complete_rendering_workflow`
- `tests/mark_pipeline_performance_tests.rs` — Fixed
  `test_bind_group_creation_performance` and
  `test_end_to_end_workflow_performance`

### Test Results

- All 12 mark renderer unit tests pass
- All 10 mark pipeline integration tests pass
- All 9 mark pipeline performance tests pass (except pre-existing flaky
  `test_registry_scalability` which has a too-tight 5ms timing threshold,
  unrelated to this change)
