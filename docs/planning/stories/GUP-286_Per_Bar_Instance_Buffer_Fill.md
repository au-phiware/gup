# GUP-286: Per-bar Instance Buffer Fill

## Story Overview

**Initiative**: Chart Builders **Status**: 💡 New **Created**: 2025-07-27

## Context

GUP-245 (Bar Chart Builder) produces a `ComposedChart<T, Rectangle>` with
correct `BandScale` and `LinearScale` configuration, but the `Selection`'s
instance buffer is not yet populated with `RectangleAttributes` for each bar.
This story fills that gap by computing per-bar centre, size, and colour from the
CPU-side `BarRecord` data and the configured scales, then uploading the instance
data so bars are visually rendered at the correct positions.

## User Story

> "As a developer rendering a bar chart, I want the builder to produce a
> `ComposedChart` whose Rectangle instances have correct positions, sizes, and
> colours derived from the data and scales, so that bars are visible on screen
> without manual instance-buffer work."

## Acceptance Criteria

- [ ] Each bar record is converted to a `RectangleAttributes` with centre, size,
      and fill derived from the configured BandScale and LinearScale
- [ ] Grouped bars divide the band width equally across series
- [ ] Stacked bars accumulate from their baseline
- [ ] Horizontal orientation swaps x and y in the instance buffer
- [ ] A simple `cargo run --example bar_chart` displays visible coloured bars

## Dependencies

### Prerequisite Stories

- GUP-245: Bar Chart Builder ✅ — provides builder, scales, and BarRecord logic
- GUP-067: Rectangle and Line Mark Implementations ✅ — provides
  RectangleAttributes and instanced rendering

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
