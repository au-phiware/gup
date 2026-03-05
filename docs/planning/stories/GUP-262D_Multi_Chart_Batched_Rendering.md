# GUP-262D: Multi-Chart Batched Rendering

**Initiative**: Ecosystem Integration **Status**: 💡 New **Created**: 2025-07-25

## Overview

When multiple `GupChart` entities exist in a Bevy application, batch their
render commands into a single command encoder submission rather than submitting
separately per chart. This reduces GPU synchronisation overhead for dashboards
with many charts.

## Context

GUP-262A renders each chart individually: each `render_to_texture_view` call
creates its own command encoder, render pass, and `queue.submit()`. With N
charts this means N separate GPU submissions per frame. Batching these into a
single encoder with N render passes (or fewer submissions) reduces driver
overhead.

## User Story

As a developer building a multi-chart dashboard in Bevy, I want all charts to
render in a single batched GPU submission so that the per-frame overhead scales
sublinearly with chart count.

## Acceptance Criteria

- [ ] A single `queue.submit()` covers all chart renders in a frame.
- [ ] Each chart still renders to its own offscreen texture.
- [ ] Performance scales sublinearly with chart count (benchmark with 10+
      charts).
- [ ] No regressions for single-chart use cases.

## Technical Tasks

1. Modify `gup_render_system` to collect all dirty charts before rendering.
2. Create a single `CommandEncoder` shared across all charts.
3. For each chart, begin a new render pass on the shared encoder targeting that
   chart's `ChartTextureTarget`.
4. Submit the single encoder once.
5. Benchmark against the per-chart submission baseline.

## Dependencies

- GUP-262A ✅ (builds on the texture target architecture)

## Testing Strategy

- Benchmark: 1, 5, 10, 20 charts per frame — measure total frame time.
- Visual regression: multi-chart output must be identical.
- GPU validation layer: no errors with batched submission.

## Risk Assessment

- **Low**: wgpu supports multiple render passes per command encoder. The main
  risk is that `prepare_draw_commands` modifies chart state that prevents
  sharing a single encoder across multiple charts, but each chart's buffers are
  independent.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] Tests pass
- [ ] Benchmark demonstrates sublinear scaling
- [ ] Documentation updated
