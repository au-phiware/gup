# GUP-232: Fix Pre-existing Mark Renderer Metric Test Failures

**Priority**: Medium **Complexity**: Low **Created**: 2026-02-27 **Status**: 📋
Planned

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

- [ ] All three `mark::renderer::tests` tests pass
- [ ] No regressions in other mark renderer tests
- [ ] Root cause documented

## Technical Tasks

- [ ] Reproduce and diagnose the 3 test failures
- [ ] Fix the metric tracking logic or update the tests
- [ ] Verify fix doesn't affect other mark renderer behaviour

## Dependencies

- **Requires**: GUP-188 (Automatic Draw Call Metrics in MarkRenderer)

## Testing Strategy

- Run `cargo test mark::renderer::tests -- --test-threads=1 --nocapture`
- Verify all 12 mark renderer tests pass
- Run full test suite to check for regressions

## Risk Assessment

- **Low**: Isolated to metric tracking, unlikely to affect rendering

## Definition of Done

- [ ] All mark renderer tests pass
- [ ] Full test suite passes with no new failures
