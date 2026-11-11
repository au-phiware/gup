---
implementer: sonnet
implementer-reason:
  the design is settled in the RFC findings and Decisions (items drawn are items
  picked); follows `hit_items` and the SVG writer's existing patterns
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/svg.rs,
    crates/gup-core/src/pick.rs,
    crates/gup-core/tests/lod.rs,
  ]
---

# GUP-718: LOD in CPU Pick, Hover, Tooltips and SVG

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-11

## Context

Under `Lod::Auto` (GUP-716) the GPU draws and GPU-picks representatives (GUP-715
returns data rows). The CPU side is still built for every row: `hit_items`
(`crates/gup-core/src/selection.rs` ~2675) indexes a released shared-column
layer only while its visible rows are at most `Retain::AUTO_MAX_ROWS`, and the
SVG writer (`crates/gup-core/src/svg.rs`) writes a mark per row. RFC-001
"GUP-631 findings" ("Interaction under LOD") settles the rule: what you see is
what you pick. The CPU pick index holds the drawn representatives (34,000-92,000
items at any row count), hover (GUP-684's outline) outlines the representative,
tooltips read its data row, and SVG writes the representatives (`Lod::Off`:
every row).

**GUP-667** (interaction above the CPU pick bound) is **kept, narrowed**, not
folded in: this story covers `pick_row`, hover and tooltips for LOD scatters
above `Retain::AUTO_MAX_ROWS` (so their CPU index is the representatives), while
GUP-667 keeps what LOD does not give: the marquee brush's exact row set and
keyboard focus over data rows, `Lod::Off` layers, and lines, areas and other
marks LOD does not take. Both stories say so.

## Read First

- RFC-001 "GUP-631 findings": "Interaction under LOD" (GPU pick, CPU pick,
  Hover, Brush, SVG, Accessibility).
- GUP-667's Context and ACs; GUP-684 (focus and hover outline the mark).
- `crates/gup-core/src/selection.rs`: `hit_items` (~2675-2800), `cpu_readable`,
  `row_id` (~2910), the `PickItem` type in `crates/gup-core/src/pick.rs`.
- `crates/gup-core/src/plot.rs`: `gpu_mode`, `set_hover`, `sync_overlay`,
  `pick_row`, how the pick index is rebuilt once per resolve (search
  `pick_index`).
- `crates/gup-core/src/svg.rs` lines 1-130 (the `Retain` note, `SvgTarget`) and
  how a mark layer's rows are written.
- GUP-716's representative selection (`Tiers::prefix`, per-chunk tiers per
  frame) and GUP-715's `LodGpu` order.

## User Story

> "As a developer showing a 30M-point scatter, I want hover, click and tooltips
> to work on what I see, and an SVG export of what I see, without indexing every
> row."

## Acceptance Criteria

### AC1: CPU pick over the representatives

- [ ] For a LOD-drawn layer, `hit_items` pushes `PickItem`s for the drawn
      representatives only, each with its data row, from the same tiers the last
      prepare used (store the selected `(chunk, count)` list on the layer at
      prepare, read it in `hit_items`). A test with `Retain::AUTO_MAX_ROWS`
      lowered through the existing seam (the GUP-664 test's seam; find it with
      `grep -n AUTO_MAX_ROWS crates/gup-core/tests crates/gup-core/src`) on a
      layer of 300,000 shared-column rows: `pick_row` at a drawn representative
      returns its data row (not `None`), and the index holds at most the
      representatives' count (assert under 10% of the rows).
- [ ] The CPU pick and the GPU pick agree: for 200 pointer positions on a seeded
      1M-row scatter, `pick_row` (CPU) and the GPU pick return the same row
      whenever either returns one.
- [ ] A layer under `Lod::Off`, or below 200,000 rows, behaves as today (the
      existing pick tests pass unchanged); above `AUTO_MAX_ROWS` with `Off` it
      still returns `None` (GUP-667's case, unchanged).

### AC2: Hover, focus outline, tooltip

- [ ] Hover over a representative outlines it (GUP-684's outline at the
      representative's position and size), and the tooltip text equals the text
      of that data row's tooltip under `Lod::Off` (`tests/hover.rs` shape). The
      PNG of the hover state at 1M rows is read: one outlined disc under the
      pointer, no outline elsewhere.
- [ ] Hover's tooltip anchor reads the representative's position (the row's
      position from the store), not the layer's index (this is GUP-667's anchor
      idea for LOD layers; GUP-667 keeps the non-LOD case).

### AC3: SVG

- [ ] `SvgTarget` of a LOD layer writes the representatives of the view (the
      count equals the drawn instance count after culling to the viewport, at
      most the same prefix GUP-716 draws), with each mark's colour and size as
      the drawn one; `Lod::Off` writes every row. A test at 300,000 rows: the
      SVG under `Auto` has under 10% of the circles, and rasterised (via the
      repo's SVG comparison helper in `tests/svg.rs`) it differs from the `Off`
      SVG by at most 1.0% of plot-rect pixels at ΔE00 > 10.
- [ ] A layer with `Retain::GpuOnly` still rasterises (unchanged); the module's
      doc note in `svg.rs` says which layers write representatives.

### AC4: Brush and descriptions unchanged

- [ ] The marquee brush's row set is exact (data rows) wherever it worked before
      (a test below the bound compares row sets with `Lod::Off`); the
      accessibility description of a LOD layer is identical to `Off`'s (string
      equality in `tests/describe.rs` shape). Brush above the bound stays
      GUP-667's.

## Decisions

- **GUP-667 stays, narrowed** (see Context): this story only gives scatters what
  the representatives give; the exact brush set, keyboard focus order over data
  rows, lines and `Off` layers remain 667's. GUP-667 stays 💡 because its design
  question (GPU pick fallback vs a streaming read) is untouched.
- **The index holds the drawn set** per resolve, rebuilt when tiers or the view
  change (the index is already rebuilt per resolve).
- **SVG writes what is drawn** (RFC-001): an SVG of 10M circles is not a useful
  file.
- **No change to `Layer::row_id`**: the pick id already carries the data row
  (GUP-715).

## Escalation Triggers

- The pick index is built once per resolve while tiers change per frame (zoom
  without a resolve): the index would hold stale representatives. Stop and
  report how often each rebuilds before choosing.
- Hover or the tooltip anchor reads rows from a path that cannot take a
  representative list (keyboard focus uses the same index: do not change its
  order here).
- The CPU/GPU agreement test fails by more than 1% of positions.

## Out of Scope

- The brush row set and keyboard focus above the bound, `Off` layers above it,
  lines (GUP-667); LOD with appends (GUP-719); size order (GUP-721).

## Technical Tasks

- [ ] Layer stores the drawn `(chunk, count)` selection; `hit_items` reads it.
- [ ] Hover and tooltip anchor from the representative; tests and PNG.
- [ ] `SvgTarget` writes representatives; doc note; tests.
- [ ] Brush and description parity tests; update GUP-667's story text (done in
      this story's drafting; verify it still matches).

## Dependencies

### Prerequisite Stories

- GUP-716: `Selection::lod` and Per-Frame Tier Draws 📋.
- GUP-684: Focus and Hover Outline the Mark ✅.
- GUP-664: Shared Columns as the CPU Copy ✅.

### Enables Stories

- GUP-632's hover at 100M (soft); narrows GUP-667.

## Testing Strategy

- **Unit tests**: index holds representatives; tooltip text parity.
- **Integration tests**: CPU/GPU pick agreement; SVG parity; describe parity.
- **Visual validation**: the hover PNG at 1M rows read by eye.

## Success Metrics

- [ ] A 1M-row LOD scatter above a lowered bound picks, hovers and writes SVG
      with an index and file of at most 10% of the rows.

## Risk Assessment

- **Medium**: stale representatives between a zoom frame and the index rebuild
  (escalation trigger). _Mitigation_: rebuild the layer's items when the
  selection changes.
- **Low**: tooltips differ between `Auto` and `Off` for the same pointer (a
  neighbour is picked). _Mitigation_: documented ("what you see is what you
  pick").

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the hover PNG)
- [ ] `mask wasm-browser`: run (hover and input behaviour)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
