---
implementer: sonnet
implementer-reason:
  the bits buffer, binding, glue lines, upload kind, theme field and default are
  fixed in Decisions; follows the validity-bits precedent (S4b)
---

# GUP-458: RFC-001 Step S9g: Dim Unbrushed Rows

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-18

## Context

GUP-454 (S9c) added `Drag::Brush`. A drag selects rows by mark position into
`Plot::brush().rows`, runs `on_brush` handlers and draws the rectangle as an
overlay (`tests/golden/gup_core/brush.png`). The selected rows look the same as
the others: in the golden, rows 1 and 2 are inside the brush and rows 0, 3 and 4
are not, and all five dots are the same blue. A user sees a rectangle, not a
selection.

The usual treatment, and the one this story makes the default, is to dim the
rows outside the brush. The brushed rows (`Brush::rows`) are the source of
truth, because handlers already receive them. Recomputing membership on the GPU
from the rectangle would disagree with them at the edges, for `GpuOnly` layers
(never brushed, GUP-454) and after an append. So the GPU reads the CPU's
membership as one bit per stored row, the way it reads validity bits (S4b). It
dims through the same alpha multiply that GUP-430's `fill_opacity` performs,
applied in the glue to every colour channel, so it works for all four library
marks and for user marks.

`Plot::handle` has no `Context`, so the bits are written at the next resolve,
which already happens after every brush (the overlay redraw).

## Read First

- [GUP-454](GUP-454_RFC_001_S9c_Brush_And_Drag_Zoom.md) Decisions and RFC-001
  "S9c findings" (lines 3422–3447): membership and lifecycle.
- RFC-001 "S4b findings" → "Validity bits" (lines 2203–2235): the precedent for
  per-row bits read in the vertex stage.
- `crates/gup-core/src/shader/glue.rs` lines 170–260 (`NULL_COLOR_FIELD`, the
  validity helpers) and 600–700 (vertex body, `fs_main`).
- `crates/gup-core/src/selection.rs` `prepare` (lines 779–990): Encodings
  fields, LUT bindings, bind groups.
- `crates/gup-core/src/context.rs` lines 50–80 (`Upload`).
- `crates/gup-core/src/theme.rs` (fields and `Default`).
- `crates/gup-core/tests/events.rs` lines 640–690 (`brush_golden`).
- `crates/gup-core/wasm-size/scatter/index.html` lines 300–345 (brush step).

## User Story

> "As a user brushing a scatter, I want the rows outside my selection to fade,
> so that I can see which rows I selected."

## Acceptance Criteria

### AC1: Dimmed by default

- [ ] `brush_golden` is re-blessed. The implementer reads the PNG: rows 1 and 2
      (inside) look as before, and rows 0, 3 and 4 are pale.
- [ ] Pixel checks in that test, at each dot's centre:
  - rows 1 and 2 equal the pre-story golden's pixels exactly;
  - rows 0, 3 and 4 are within 3/255 per channel of `(179, 213, 232)`, which is
    `#0072B2` at opacity 0.3 over white in sRGB space.
- [ ] Clearing the brush (`clear_brush`, then resolve) renders byte-identical to
      the same plot never brushed.

### AC2: Every mark, every colour channel

- [ ] A four-layer test (`Circle` with a stroke, `Rect`, `Segment`, `Line`),
      half the rows of each inside a brush: at each unbrushed mark's centre the
      colour equals 0.3 opacity of its colour over white (±3/255), and at each
      brushed mark's centre it is unchanged.
- [ ] A `Line` segment is dimmed when its first row is not brushed, and drawn at
      full opacity when it is.
- [ ] A `Retain::GpuOnly` layer past one chunk is dimmed entirely while a brush
      exists (it has no brushed rows, GUP-454).
- [ ] On the brushed four-layer plot, `pick_row_gpu` returns the same row as
      `pick_row` at every mark's pick anchor (the pick pipelines bind the new
      layout).

### AC3: Theme

- [ ] `Theme::unbrushed_opacity` (Decisions) set to 0.5 gives
      `0.5 · c + 0.5 · 255` (±3/255) at an unbrushed dot.
- [ ] Set to 1.0, a brushing plot's glue, uploads and image are identical to a
      plot without this story's change. The glue signature has no `brushed` and
      no bits are written.

### AC4: Uploads

- [ ] The resolve after a brush release writes exactly `4 · ⌈rows / 32⌉` bytes
      of `Upload::Brush` per layer and 4 bytes of `Upload::Uniform` per layer
      (the `brush_dim` field), and 0 column, validity and texture bytes.
- [ ] Zooming and panning a brushed plot for 60 frames writes 0 `Upload::Brush`
      bytes.
- [ ] `context::tests::every_gpu_write_is_counted` passes.

### AC5: Size-ordered layers

- [ ] A layer drawn size-descending (GUP-431 `DrawOrder`) dims exactly the rows
      outside the brush. Check per row at its pick anchor; the bits are indexed
      by stored row, mapped from `Brush::rows` through the inverse of
      `DrawOrder::row_of`.

### AC6: Changed default, searched

- [ ] `git grep -n "Drag::Brush\|on_brush\|brush()\|brush_rect" -- crates examples tests dogfood`:
      every test, example and harness scene that checks mark pixels after a
      brush is updated for dimming or clears the brush first. List them in the
      Implementation Summary.
- [ ] The browser brush step also checks that row 0's centre is dimmed.
      `mask wasm-browser` passes. Any later step that checks a mark's colour
      runs after a `clear_brush()` hook call; its check is not loosened.

### AC7: Size

- [ ] `mask wasm-size` passes. Growth over 3,000 B gz is an escalation, and the
      scatter row is re-recorded with the reason if it moves more than +2%.

## Decisions

- **Theme**: `pub unbrushed_opacity: f32` on `Theme`, default `0.3`, documented
  as "opacity multiplier for rows outside the brush while one exists; 1.0 turns
  dimming off".
- **When the glue changes**: a layer's glue is the brushed variant when its plot
  has `Drag::Brush` and `theme.unbrushed_opacity < 1.0`. Changing either already
  bumps the revision, so the layers re-prepare. The variant appends ` brushed`
  to the glue signature, which is the pipeline cache key.
- **Bits**: per layer, a storage buffer
  `@group(1) @binding(N) var<storage, read> brush_bits: array<u32>;`, where N is
  the first binding after the layer's LUTs.
  - Size: `max(1, ⌈stored rows / 32⌉)` words, created at prepare. wgpu zeroes
    it, so it starts with no rows brushed.
  - Bit `r` is set when stored row `r` is brushed.
  - It is rewritten whole, as `Upload::Brush` (a new variant: "brushed-row bits,
    one bit per stored row, written whole when the brush changes"), at the first
    resolve after the brush changes, and on prepare if the row count changed.
  - Pick pipelines share group 1's layout, so they bind it too and never read
    it.
- **Factor**: an `Encodings` field `brush_dim: f32`. It is 1.0 while the plot
  has no brush, and `theme.unbrushed_opacity` while it has one. It is written at
  prepare, and as a 4-byte write at its offset when the brush appears or clears.
- **WGSL**, emitted after the `m.<channel> = …` assignments, using the stored
  row the pick already uses:

  ```wgsl
  let brush_row = chunk.row_base + instance_index;
  let brush_in = ((brush_bits[brush_row / 32u] >> (brush_row % 32u)) & 1u) == 1u;
  let brush_k = select(enc.brush_dim, 1.0, brush_in);
  m.<c> = vec4<f32>(m.<c>.rgb, m.<c>.a * brush_k);   // for every Role::Color channel c
  ```

  No identifier ends in a digit (RFC-001 "S5c findings").

- **Plot plumbing**:
  - `Layer::prepare` gains `brushable: bool`.
  - A new
    `Layer::write_brush(&mut self, cx: &Context, rows: Option<&[usize]>) -> Result<()>`
    takes the layer's brushed data rows (`None` means no brush), maps them to
    stored rows, and writes the bits and the factor.
  - `Plot` keeps a brush generation, bumped by every brush set or clear, and
    calls `write_brush` in `resolve` when the generation differs from the last
    one written. A brush change alone does not invalidate the cached scene.
- **What "brushed" means**: exactly `Brush::rows`. Nothing is recomputed on the
  GPU.

## Escalation Triggers

- A storage buffer read in the vertex stage fails validation on SwiftShader or
  lavapipe. Validity bits already do this, so it would be new information.
- Writing the bits needs a full re-prepare, so a brush release re-uploads
  columns (AC4 fails).
- Any golden other than `gup_core/brush` changes.
- `mask wasm-size` grows by more than 3,000 B gz.

## Out of Scope

- Brush styling beyond opacity (greying, outlines): a theme follow-up if asked.
- Linked brushing across plots: a user's `on_brush` handler (dogfood task 4).
- Brushing `GpuOnly` layers (GUP-454 Out of Scope).

## Technical Tasks

- [ ] `Theme::unbrushed_opacity`; `Upload::Brush`.
- [ ] Glue: the `brushed` spec flag, binding, field and WGSL; signature suffix.
- [ ] `Selection`: buffer, bind group entry, `prepare(.., brushable)`,
      `write_brush` with the `DrawOrder` mapping.
- [ ] `Plot`: brush generation; calling `write_brush` in `resolve`.
- [ ] Tests (AC1–AC5); re-bless `gup_core/brush`; harness step (AC6).
- [ ] `mask wasm-size`; `mask wasm-browser`.

## Dependencies

### Prerequisite Stories

- GUP-454: RFC-001 S9c Brush and Drag-Zoom ✅ — `Brush`, membership, overlay.
- GUP-430: RFC-001 S6d Stroke and Fill Opacity ✅ — the alpha multiply this
  reuses.
- GUP-431: RFC-001 S6e Mark Draw Order ✅ — stored versus data row order.

### Enables Stories

- GUP-465: dogfood task 4 (linked views) can show its own selection without
  re-encoding colours.

## Testing Strategy

- **Integration**: pixel checks per mark and theme value, upload counts,
  draw-order mapping (AC1–AC5) in `tests/events.rs`.
- **Visual**: re-blessed `brush.png`, read by the implementer.
- **Browser**: the brush step checks dimming.

## Success Metrics

- [ ] A brushed chart shows its selection at a glance, in a window and in a
      browser.
- [ ] Brushing costs one small bits upload per layer, never a column upload.

## Risk Assessment

- **Medium**: the extra group-1 binding changes pick pipeline layouts.
  _Mitigation_: picks share group 1's layout today (S9d findings), and AC2
  checks GPU picks on a brushed plot.
- **Low**: sRGB-space blending makes 0.3 look lighter than expected.
  _Mitigation_: the value is themable, and AC1 pins the arithmetic.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: the re-blessed `brush.png` and
      `browser_scatter.png`
- [ ] `mask wasm-browser` and `mask wasm-size` pass
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
