# GUP-287: Dynamic Data Refresh for ComposedChart

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 📋 Planned **Created**:
2025-07-19

## Context

GUP-284 established a build-time pipeline preparation pattern where
`prepare_render_bound()` is called in `build_with_data()`. This works well for
static charts but means there is no easy way to update data and re-render
without rebuilding the entire chart. Interactive visualisations and streaming
data scenarios need a lightweight `refresh_data()` path.

## User Story

> "As a Gup developer, I want to update the data behind a `ComposedChart` and
> re-render without rebuilding the entire chart from scratch."

## Acceptance Criteria

- [ ] `ComposedChart` exposes a `refresh_data()` method that replaces the
      Selection's data, re-evaluates attr bindings, and re-uploads GPU instances.
- [ ] Calling `refresh_data()` then `render_to_png()` produces a chart
      reflecting the new data.
- [ ] Pipeline objects (render pipeline, shaders) are reused — only instance
      buffers are re-uploaded.
- [ ] At least one test demonstrates data update followed by correct rendering.

## Technical Tasks

- [ ] Add `ComposedChart::refresh_data()` that replaces data on the inner
      Selection, re-computes the data domain, and calls `prepare_render_bound()`.
- [ ] Optionally support domain-locking (keep old axis range) vs auto-rescale.
- [ ] Add tests for data refresh with both domain modes.

## Dependencies

### Prerequisite Stories

- GUP-284 ✅ (Unify Chart Builder Data Layer)

## Testing Strategy

- Unit test: replace data, render to RGBA, verify pixel differences.
- Test that pipeline objects are reused (instance count changes but pipeline
  address remains the same).

## Risk Assessment

- **Low**: The Selection already supports multiple `prepare_render_bound()` calls
  which re-upload instances. The main work is recalculating the data domain and
  NDC bounds.

## Definition of Done

- [ ] `refresh_data()` method implemented and documented.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
