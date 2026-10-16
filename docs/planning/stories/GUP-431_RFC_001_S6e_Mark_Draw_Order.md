---
implementer: opus
implementer-reason:
  design work - the ordering mechanism changes how columns are bound or
  uploaded, must stay append-friendly and correct with translucent fills
---

# GUP-431: RFC-001 Step S6e: Size-Descending and Emphasis Draw Order

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-16

## Context

RFC-001's "S5b findings → Proposed adjustments" says: "Draw order. Bubble charts
want size-descending z-order; today it is data order." The orchestrator's review
of the GUP-420 goldens (2026-10-11) adds emphasis: in `wgsl_function_highlight`
the vermilion highlighted countries must not sit under grey ones. This story was
split out of [GUP-422](GUP-422_RFC_001_S6a_Derive_Mark_V2.md) on 2026-10-16
because it is design work, not execution.

Today a layer draws one instanced draw per column chunk (`render.rs`,
`ChunkDraw`, lines 74–85), rows in upload order. Columns are bound as
instance-rate vertex buffers (`column_ranges`), so an instance's attributes come
from its own row and cannot be permuted at draw time without changing how
columns are bound. The constraints the design must meet:

- **Append-friendly** (RFC-001 §3, S4a): appending rows uploads only the new
  rows' bytes; a design must not re-sort or re-upload the whole store on every
  append.
- **Translucent fills**
  ([GUP-430](GUP-430_RFC_001_S6d_Mark_Stroke_And_Fill_Opacity.md)): depth
  testing alone gives wrong blends for `fill_opacity < 1` (a nearer translucent
  mark drawn first is not blended over the farther one), and no target has a
  depth attachment today (`render.rs`, `depth_stencil: None`).
- **Picking** (RFC-001 §9, S8/S9) will map a drawn instance back to its row.
- **The size is encoded on the GPU** (for example `Pow::sqrt` of a population),
  but scales are monotonic (`ShaderFn::image`), so ordering by the size
  channel's data value orders by size.

Candidate designs to evaluate (not exhaustive): sort rows on the CPU at column
build with a per-chunk-only guarantee for appended chunks; a permutation buffer
read in the vertex stage with columns read from storage buffers (vertex
pulling); and expressing emphasis as a separate layer with a higher `.z()`
([GUP-424](GUP-424_RFC_001_S7a_Multi_Layer_Plot_And_Shared_Scales.md)), leaving
only size order to solve.

## Read First

- RFC-001 §3 "Data model" Design (lines 160–258) and "S4a findings" → "Chunks",
  "Drawing", "Append" (lines 2042–2118).
- RFC-001 §12 "Risks" (lines 796–866), risk 7 (vertex-buffer slots).
- `crates/gup-core/src/render.rs` lines 74–120 (`ChunkDraw`, `LayerGpu`) and
  740–790 (`with_chunks`, `write_uniforms`).
- `crates/gup-core/src/selection.rs` lines 470–600 (`prepare`).
- `crates/gup-core/tests/scales_png.rs` lines 52–92 (`sqrt_radius_scatter`) and
  `crates/gup-core/tests/wgsl_function.rs` near line 329
  (`wgsl_function_highlight`).

## User Story

> "As a visualization developer drawing a bubble chart or highlighting a
> category, I want the largest marks drawn first and emphasised marks drawn
> last, so that no mark is hidden by insertion order alone."

## Acceptance Criteria

### AC1: Design recorded

- [ ] A "GUP-431 findings" section appended to RFC-001 records the chosen
      mechanism, the alternatives rejected and why, its cost (bytes per row,
      draws, vertex-buffer slots) and its append behaviour.

### AC2: Size-descending order

- [ ] A layer whose `Size`-role channel is a column draws rows largest first. A
      test inserts rows smallest-first at varying radii with overlapping discs
      and asserts the pixel at each row's centre is that row's own fill within
      2/255.
- [ ] The same test passes after appending a second batch of rows (smaller and
      larger than existing ones), and the append uploads only the new rows'
      column bytes plus at most O(new rows) bytes of ordering data (asserted
      with `UploadStats`).
- [ ] With `fill_opacity < 1`, a pixel covered by two discs equals the smaller
      disc over the larger over the background, within 3/255.

### AC3: Emphasis drawn last

- [ ] `wgsl_function_highlight`'s highlighted rows draw above the others, by
      whichever mechanism AC1 records (a documented recipe, such as a second
      layer with a higher `.z()`, is acceptable if it needs no new API). The
      golden is re-blessed and read by eye: no vermilion point under a grey one.

### AC4: Goldens

- [ ] `sqrt_radius` is re-blessed and read by eye: small bubbles are visible on
      top of large ones.

## Decisions

- Depth testing is not the mechanism (translucent fills, no depth attachment); a
  depth-based proposal must explain how it blends correctly.
- Rows without a `Size`-role column (constant size) keep data order.
- The mechanism and any new public API are this story's to choose and record
  (AC1).

## Escalation Triggers

- The chosen design needs storage buffers in the vertex stage on a path that
  must run where `max_storage_buffers_per_shader_stage == 0` (compare
  `prepare`'s validity-bits check).
- The design changes the column store's chunk format or `Retain` behaviour.
- `mask perf-budget` regresses outside noise.

## Out of Scope

- GPU picking's instance-to-row mapping: RFC-001 S9 (not yet a story), though
  AC1 must say how picking will recover the row.
- Per-layer `.z()`: GUP-424.

## Technical Tasks

- [ ] Prototype the candidate(s) against AC2's adversarial test.
- [ ] Implement the chosen mechanism; record findings in RFC-001.
- [ ] Re-bless `sqrt_radius` and `wgsl_function_highlight`.

## Dependencies

### Prerequisite Stories

- GUP-430: Stroke and Fill Opacity 📋 — `fill_opacity`, which the mechanism must
  blend correctly.

### Soft Dependencies

- GUP-424: Multi-Layer Plot 📋 — per-layer `.z()`, if emphasis uses layers.

## Testing Strategy

- **Integration tests**: AC2's adversarial insertion and append tests.
- **Visual validation**: `sqrt_radius`, `wgsl_function_highlight`.
- **Performance**: `mask perf-budget`; record the numbers in the findings.

## Success Metrics

- [ ] No row in the adversarial test is hidden by insertion order.

## Risk Assessment

- **High**: every candidate trades append cost, binding complexity or guarantee
  strength. _Mitigation_: measure, record, and prefer the simplest design that
  meets AC2 as written.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
