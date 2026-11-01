---
implementer: opus
implementer-reason:
  a per-layer coverage pass against the single-pass render model, with design
  alternatives to measure; design work by nature
touches: [crates/gup-core/src/selection.rs, crates/gup-core/src/render.rs]
---

# GUP-439: Translucent Lines Without Double-Blended Joins

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress **Created**: 2026-10-17

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

## N2 Notes (2026-10-24)

Absorbed into track N2 by the November review's triage ("same polyline geometry
as line curves and area outlines; do them together") and promoted from 💡 to 📋.
It runs after GUP-550 and GUP-551.

### Read First

- RFC-001 "GUP-432 findings" (capsules, bridges, "Joins and caps"), "GUP-550
  findings" (joint ownership, `joins`/`others`), "GUP-551 findings" and "GUP-576
  findings" (plane joints, piece counts, cost table): `grep -n` for the headings
  in `docs/planning/rfcs/RFC-001_Core_Architecture.md`.
- `crates/gup-core/src/render.rs`: `Renderer::prepare_unscoped`,
  `Renderer::render`, `Prepared::draw`, `create_pipeline`.
- `crates/gup-core/src/selection.rs`: `Layer::prepare` (where the batch is
  built) and `brush_dim`.
- `crates/gup-wgsl/library/line.wgsl`: `shade` (premultiplied output).
- `crates/gup-core/tests/line_png.rs`: `a_translucent_join_blends_twice`.
- `crates/gup-core/src/svg.rs`: `write_layer`.

### Decisions

(Written 2026-11-02 at the start of implementation; the N2 notes' decisions are
kept, the coverage format is amended.)

- **Scope includes curve pieces**: after GUP-550/551 a translucent step or
  monotone line overlaps at every piece joint as well as at rows; AC1's even
  opacity applies to those joints too (add 3 piece joints of a `MonotoneX` line
  and the corners of a `StepAfter` line to the AC1 measurements).
- **Areas are not in scope**: `Area` trapezoids share edges and do not
  double-blend (GUP-545). Area outlines are `Line` layers and get the pass like
  any other line.
- **Mechanism: an offscreen target per translucent line layer.** The layer is
  drawn with its own glue and `line.wgsl` (no shader change) into an
  `Rgba8Unorm` texture cleared to 0, with `BlendOperation::Max` on colour and
  alpha. `shade` outputs premultiplied colour times coverage, so each texel
  holds the largest coverage any segment or piece gives it: the coverage of the
  union, the round-joined stroke drawn once, at joins, piece joints, overlapping
  segments and self-crossings alike. The scene pass then draws one full-target
  triangle per such layer, scissored to the layer's clip, that loads the texel
  under each pixel and blends it with premultiplied over. Outside the line the
  texel is 0, which leaves the target's bytes unchanged.
- **`Rgba8Unorm`, not `R8`** (amends the N2 note): a premultiplied colour target
  keeps per-row colour, and needs no CPU copy of the layer's colour. Where rows
  of different colours overlap, each channel takes the larger value; where they
  share a colour (the usual case: one series per layer) the result is exact.
  Cost: 4 bytes per target pixel per translucent layer, pooled across frames.
- **Which layers**: `Polyline` geometry whose colour can have alpha below 1: a
  constant, derived or default colour with alpha < 1; a colour column with a LUT
  holding alpha < 255, or with no LUT (unknown, so assumed translucent); an
  active brush with `Fade` below 1 or a translucent `Grey`; or a read null
  colour with alpha < 1. Every other layer, and every opaque line, takes today's
  path byte for byte, with no texture and no extra pass.
- **Gup-owned targets** (`Renderer::render`: image, texture, window and web
  targets) record the offscreen passes in the frame's encoder before the scene
  pass. The scene is still drawn in one pass.
- **Draw-in-pass hosts** (`Renderer::prepare` + `Prepared::draw`) are unchanged
  until the escalation below is answered: their translucent lines draw directly,
  as today.
- **Stencil is not built** unless the offscreen target fails the 2x rule: it
  needs a depth-stencil attachment in every target and host pass (a `TargetDesc`
  change), and a binary stencil either drops the analytic antialiasing or keeps
  the seam where two rims cross (RFC-001 "GUP-432 findings", alternative 3). The
  reasons are recorded in RFC-001.
- **Decision rule**: keep the offscreen target unless the translucent layer
  costs more than 2x its direct draw at 100K points (`zoom_bench`), or cannot
  run on the browser smoke test's adapter (WebGPU baseline only: `Max` blend,
  `Rgba8Unorm` render attachment and `textureLoad` are core).
- **SVG**: a translucent line layer whose strokes share one alpha is written as
  `<g opacity="a">` around opaque strokes: the group is composited once, as the
  GPU's offscreen target is. A layer with mixed alphas keeps per-element
  `stroke-opacity`.

### Escalation Triggers

- Neither approach keeps opaque lines byte-identical with no extra pass (AC2).
- Draw-in-pass hosts (`Prepared::draw`, egui, Bevy) would need an API change to
  get the extra pass: report the smallest change before making it.
- The offscreen layer costs more than 2x the direct draw at 100K points.
- WASM growth over 3 KB gz, or any opaque golden changes.

### Out of Scope

- Areas, segments, circles and other marks; blending between different layers
  (overplotted series still blend over each other, which is the density the
  story wants).
- Sharing one texture between several translucent layers (memory grows with
  their count; see the RFC record).

### Additional Dependencies

- GUP-550: Line curves part 1 📋; GUP-551: monotone curves 📋.

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
