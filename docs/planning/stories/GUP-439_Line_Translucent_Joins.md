# GUP-439: Translucent Lines Without Double-Blended Joins

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 💡 New **Created**: 2026-10-17

## Context

[GUP-432](GUP-432_RFC_001_S6f_Line_Mark.md) draws each `Line` segment as a
capsule, so consecutive segments overlap in a disc at their shared row. An
opaque line shows no seam. A translucent one blends the overlap twice: at alpha
0.5 a join has 0.75 ink against 0.5 on the arms
(`a_translucent_join_blends_twice` in `crates/gup-core/tests/line_png.rs` pins
this). A sharp turn also overlaps on its inner side, beyond the disc. A
translucent time series with many points therefore shows a bead at every vertex,
and overplotted translucent series, a common way to show density, look dotted.

The fix cannot come from the segment alone. Excluding the overlap needs the
neighbours' directions (rows `i − 1` to `i + 2`), which RFC-001 "GUP-432
findings" rejected for the slot budget. Clipping each capsule's start disc
leaves a seam where the coverage ramps cross. The usual remedy works per layer:
draw the line's coverage once (an offscreen target with `Max` blending, or a
stencil pass), then composite it with the line's colour and alpha.

## User Story

> "As a visualization developer, I want a translucent line to have one even
> opacity along its whole length, so that overplotted series show density, not
> beads at every data point."

## Acceptance Criteria

### AC1: Even opacity

- [ ] A `Line` with colour alpha 0.5 has the same ink at a join as on its arms,
      within 2/255, for turns of 30°, 90° and 170°, and where the line crosses
      itself.

### AC2: Opaque lines unchanged

- [ ] Opaque lines render byte-identically to before, with no extra pass, and
      `gup_core/line_series` is unchanged.

### AC3: Cost recorded

- [ ] The extra pass or target's cost (GPU time at 100K points, memory, draws)
      is measured and recorded in RFC-001, along with the design chosen and the
      designs rejected.

## Technical Tasks

- [ ] Prototype a per-layer coverage pass (offscreen `R8` target with `Max`
      blend, composited with the layer's colour) against a stencil approach.
- [ ] Decide whether per-row colour survives (a coverage target loses it), and
      how draw-in-pass hosts get the extra pass.
- [ ] Implement it, test AC1–AC2, and record the findings.

## Dependencies

### Prerequisite Stories

- GUP-432: `Line` Mark ✅.

## Testing Strategy

- **Integration tests**: ink at joins and self-crossings against the arms.
- **Visual validation**: a translucent overplotted-series golden.
- **Performance**: `zoom_bench`-style timing of a 100K-point translucent line.

## Success Metrics

- [ ] Overplotted translucent series show no beads at their vertices.

## Risk Assessment

- **Medium**: a per-layer pass conflicts with the single-render-pass model and
  with host-owned passes (`Prepared::draw`). _Mitigation_: make the pass opt-in
  for translucent line layers, and record what draw-in-pass hosts must do.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
