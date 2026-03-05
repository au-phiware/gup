# GUP-268C: Text Label Rendering in PNG Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 💡 New **Created**: 2025-07-19

## Context

GUP-268 delivered PNG export with axes, ticks, and grid lines. GUP-268A added
data mark rendering. However, the exported PNG still lacks text labels — axis
labels, chart titles, tick value labels, and subtitles are not rendered. These
text elements rely on Gup's SDF-based text rendering pipeline, which is separate
from the axis geometry and Selection pipelines.

This story wires the text rendering pipeline through the PNG export path so that
exported images are fully self-contained with all visual elements.

## User Story

> "As a visualisation developer, I want PNG exports to include text labels
> (titles, axis labels, tick labels) so that the exported chart is complete and
> readable without additional context."

## Acceptance Criteria

- [ ] Exported PNG includes the chart title and subtitle (if configured).
- [ ] Exported PNG includes axis labels and tick value labels.
- [ ] Text is correctly positioned and sized relative to the chart area.
- [ ] No regression in existing PNG export tests.

## Technical Tasks

- [ ] Wire the SDF text rendering pipeline through the off-screen render pass.
- [ ] Ensure text atlas textures are created and uploaded for the export path.
- [ ] Handle text scaling for HiDPI exports.
- [ ] Add integration tests verifying text presence in exported pixels.

## Dependencies

### Prerequisite Stories

- GUP-268A: Data Mark PNG Export ✅ — provides the mark rendering in export.
- Text rendering system (SDF pipeline) — must be functional.

## Testing Strategy

- Integration test: Export a chart with a title and axis labels, decode the PNG,
  and verify non-white pixels at expected text positions.
- Visual validation: Run the export example and inspect text rendering.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
