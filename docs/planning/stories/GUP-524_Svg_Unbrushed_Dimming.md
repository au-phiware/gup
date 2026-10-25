---
implementer: sonnet
implementer-reason: a vector-target parity fix with a measured golden comparison
touches: [crates/gup-core/src/svg.rs, crates/gup-core/src/vector.rs]
---

# GUP-524: SVG Draws Unbrushed Rows Dimmed

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-26

## Context

GUP-553's brush golden (`gup_core/theme_dark_brush`) could not use the SVG
parity check (`tests/common/svg_marks.rs`): the GPU draws rows outside a brush
dimmed (`Theme::unbrushed`, `Fade(0.3)` by default), and `SvgTarget` draws every
mark at full strength. 668 of 16000 4×4 blocks differ (max ΔE 46). Any exported
SVG of a brushed chart therefore shows something different from the screen.

## User Story

> "As a developer exporting a brushed chart, I want the SVG to dim unbrushed
> rows as the window does, so that the export matches what I see."

## Acceptance Criteria

- [ ] `SvgTarget` draws rows outside the brush with the theme's `Unbrushed`
      treatment (`Fade` multiplies alpha, `Grey` replaces the colour).
- [ ] `theme_dark_brush` runs the SVG parity check (`svg: true` in
      `tests/theme_dark_png.rs`), within `MAX_BLOCK_DE`.
- [ ] No unbrushed-chart SVG output changes.

## Technical Tasks

- [ ] Carry the brush state (rows, `Unbrushed`) into the vector snapshot.
- [ ] Apply it per mark in `vector.rs`; enable the check; commit.

## Dependencies

### Prerequisite Stories

- GUP-553 ✅ (the golden).

## Testing Strategy

- The `theme_dark_brush` golden with the SVG parity check on.

## Risk Assessment

- **Low**: SVG size grows only for brushed charts.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` clean; `cargo test -p gup-core -- --test-threads=1` passes
- [ ] Story status updated in the story file and INDEX.md
