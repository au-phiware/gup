# GUP-393: Mark Fidelity Fixes

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: ⏸ Parked **Created**:
2026-10-04

**Parked 2026-10-04**: superseded by
[RFC-001](../rfcs/RFC-001_Core_Architecture.md) steps S0/S3. RFC-001 (Core
Architecture) is accepted and builds the new core in `crates/gup-core`; the old
`gup` crate path (including `src/mark/circle.rs`, `src/chart_builder.rs`,
`src/mark/renderer.rs`, and the area/line builders this story would have
touched) is frozen — no feature work — ahead of the S14 flip. GUP-384 (area
stroke width) remains folded here and stays parked for the same reason. Do not
implement this story on the old path; the evidence below is preserved as input
for whoever implements RFC-001 S0/S3, where equivalent mark-rendering
correctness work belongs in `gup-core` instead.

## Context (preserved evidence for RFC-001 S0/S3)

Verified against `main` on 2026-10-04, for reference when `gup-core` builds its
own mark rendering:

- **Circle radius is not aspect-corrected**: `src/mark/circle.rs` has no
  aspect-ratio handling;
  `world_pos = input.position * transformed_radius + transformed_center`
  (`circle.rs:267-269`) applies one scalar radius to both NDC axes, producing
  ellipses on non-square canvases.
- **No anti-aliasing**: `src/mark/renderer.rs:936` and
  `src/chart_builder.rs:231-234` both set `sample_count: 1` / `count: 1` — no
  MSAA anywhere in the render path; only circle/boxplot SDF shaders do their own
  `smoothstep` edge softening.
- **Area-chart stroke width** (absorbed from GUP-384):
  `AreaChartBuilder::build_with_data()` binds `seg.width` (logical pixels,
  `1.5`) directly to the Line mark's NDC-space `width` attribute, rendering
  strokes at ~75% of the viewport instead of ~1.5px. `LineChartBuilder` already
  converts correctly (`seg.width * (2.0 / chart_width)`); GUP-382's
  `SegmentNdcMapper` is the natural place to share that conversion if this
  pattern recurs in `gup-core`.
- **`composite_*` first-frame panic**: all four `composite_*` examples
  reportedly panic on first frame with a `BindGroup does not exist` wgpu
  validation error (review's visual audit; original renders in
  `/tmp/gup-visual-audit/` are gone — unverified against current `main` at time
  of parking). Likely a stale bind-group-across-buffer-resize pattern; worth
  checking whether `gup-core`'s render-target design (RFC-001 S2) avoids this
  class of bug structurally.
- **No scissor-clipping**: `grep -rln set_scissor_rect src/` finds one call site
  (`src/mark/advanced_rendering.rs`), not reachable from
  `src/chart_builder.rs`'s render path — marks can draw outside the plot rect.
  `gup-core`'s `RenderTarget`/`Scene` design (RFC-001 S2) should clip by
  construction rather than needing a bolt-on fix.
- **Horizontal line segments invisible**: reported by the dogfood audit
  (ephemeral, unverified); `src/mark/line.rs`'s normal computation looked
  orientation-agnostic on inspection, so the cause was not confirmed before
  parking.

## Dependencies

### Prerequisite Stories

- GUP-379 ✅, GUP-382 ✅ — background for the area-stroke-width evidence above.

## Definition of Done

Not applicable — parked. Re-evaluate after RFC-001 S0/S3, where this evidence
should inform (not be copy-pasted into) `gup-core`'s own mark rendering
correctness work.
