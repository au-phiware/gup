---
implementer: sonnet
implementer-reason:
  the mark's rectangle is already in the pick index (Shape::Box) at overlay
  time, so this is a lookup plus a ring variant; the geometry rule is decided
  below
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/pick.rs,
    crates/gup-core/tests/hover.rs,
    crates/gup-core/tests/keyboard.rs,
    crates/gup-core/wasm-size/scatter,
    CHANGELOG.md,
  ]
---

# GUP-684: Focus And Hover Outline The Mark

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-08

## Context

GUP-676 draws the hover ring, and GUP-619 draws the focus ring, as a small
square centred on the pick anchor (`ring_item`, `hover_item` 16 px, `focus_item`
18 px in `plot.rs`). The anchor of a box-shaped mark is the middle of its top
edge (`Shape::anchor`, `pick.rs`, `Shape::Box`), so on a bar the ring is a small
square straddling the top-centre of the bar
(`tests/golden/gup_core/hover_highlight_bar.png`). It reads weakly: nothing
shows how far the bar extends.

The mark's rectangle is already known where the overlay is composed. Bars,
histogram bins, heatmap cells and box plot boxes are all `HitShape::AlignedBox`
layers, which the pick index stores as `Shape::Box { lo, hi }` in the same
logical pixels as the anchor. `Plot::sync_overlay` calls `pick_anchor(row)`,
which is `PickIndex::anchor(row)`; an `extent(row)` beside it is the same linear
lookup. Nothing needs to be stored in `Plot::hover` (a `(RowId, Point, u64)`),
because the overlay is composed once per resolve generation.

Assumption checked against the code: lines, areas, segments and paths are
`Shape::Capsule`, `Path`, `Area` and `Segment`; discs are `Shape::Disc`. They
have no rectangle, and keep the vertex or disc ring.

## Read First

- `crates/gup-core/src/plot.rs`: `ring_item`, `focus_item`, `hover_item` (lines
  376 to 430), `sync_overlay` (about 1568 to 1650), `Layout::focus` and
  `Layout::hover` (about 75 to 85).
- `crates/gup-core/src/pick.rs`: `Shape` (line 34), `Shape::anchor` (about 184),
  `PickIndex::anchor` (about 672), the `AlignedBox` arm of `hit_items` (about
  345 to 372; `span` orders the corners).
- `crates/gup-core/tests/hover.rs` (all of it) and `keyboard.rs` lines 400
  to 430.
- GUP-676 story "Decisions" and its Implementation Summary.

## User Story

> "As a person reading a chart, I want the hovered or focused bar, cell or box
> outlined, so that I can see which mark the tooltip or screen-reader text names
> and how large it is."

## Acceptance Criteria

### AC1: A box-shaped mark is outlined

- [ ] `PickIndex::extent(&self, row: RowId) -> Option<Rect>` returns the
      rectangle of a `Shape::Box` row (corners ordered, logical pixels) and
      `None` for every other shape.
      `Plot::pick_extent(&mut self, row) ->     Option<Rect>` (`#[doc(hidden)]`,
      beside `pick_anchor`) wraps it.
- [ ] With the pointer over a bar, histogram bin, heatmap cell or box plot box
      (and not in the small-rectangle case of AC2), `sync_overlay` pushes an
      outline of that rectangle instead of the vertex ring, and sets
      `Layout::hover` to the outline's outer box, which equals the mark's
      rectangle clipped to the plot rect and snapped to whole pixels.
- [ ] The same for keyboard focus: `Layout::focus` is the outline's outer box.
      Tests assert `layout.hover` and `layout.focus` for a bar, a heatmap cell
      and a box plot box (rect equal to the mark's rectangle within 1 px,
      computed from `x_ticks`/`y_ticks` or `pick_extent`).
- [ ] Scatter (discs), line and area keep the existing ring byte-for-byte (AC4).

### AC2: Geometry rule

- [ ] Outline layers, as `ring_item` does: an edge of 1 px in `theme.background`
      on the outermost pixels of the snapped rectangle, then `ink_width` px in
      `theme.title` inside it. Hover `ink_width` 1, focus 2 (thinner for hover,
      as today). The outline lies inside the mark's rectangle, so it never
      covers a neighbouring bar or cell.
- [ ] If the clipped, snapped rectangle is narrower or shorter than
      `2 * (1 + ink_width) + 2` px (6 px hover, 8 px focus), or empty after
      clipping, the vertex ring at the anchor is drawn instead (a thin bar would
      otherwise vanish under its outline). A test covers a 3 px bar for both.
- [ ] A GPU-picked layer (no extent) gets the vertex ring.

### AC3: Goldens, read by eye

- [ ] `hover_highlight_bar.png` re-blessed: the bar is outlined, edge outside
      ink, the tooltip beside it. The implementer reads the PNG and reports that
      all four sides are visible and the neighbouring bars are untouched.
- [ ] New goldens, each read: `focus_outline_bar.png` (keyboard focus on a bar,
      2 px ink, thicker than the hover outline), `hover_outline_heatmap.png`
      (one cell outlined between adjacent cells), `hover_outline_boxplot.png`
      (the box, not the whiskers, outlined).
- [ ] Contrast: with `Theme::dark()` and `Theme::high_contrast()` the outline
      ink is `theme.title` and its edge `theme.background` (extend the existing
      3:1 test in `hover.rs`).

### AC4: Nothing else changes

- [ ] `git status` after the full suite shows only the re-blessed and new
      goldens above. Specifically unchanged: `hover_highlight_scatter.png`,
      `tooltip.png`, `hello_world.png`, `theme_dark.png`, `tour/08_tooltip.png`
      and `keyboard_focus.png` (a scatter; if it is not, report it).
- [ ] Search every test, example, tour chapter, tutorial and browser scene that
      hovers or focuses a bar, bin, cell or box (grep `pointer_moved`,
      `Key::Right`, `Key::Tab`, `layout.focus`, `layout.hover` under `crates/`,
      `docs/tour/snippets`, `docs/tutorials`, `examples/`) and list them. Update
      any that compare pixels or the ring's rect (the ring box was 16 or 18 px;
      it is now the mark's rectangle) and re-bless affected goldens, reading
      each. Hover pixel changes in a tour or tutorial golden must be listed in
      the report.
- [ ] `hover_highlight(false)` still removes exactly the hover item.

### AC5: Browser smoke test and size

- [ ] The browser scatter scene is scatter-only, so `hover_ring_rect()` and the
      `hover-ring` and `key-focus` steps are unchanged and pass.
      `mask     wasm-browser` passes. If a bar scene is cheap to add to the
      scatter app (it already builds a `Plot`), do not: that is out of scope;
      say so in the retrospective.
- [ ] `mask wasm-size` before and after; growth under 300 bytes gz. Over that,
      trigger an escalation. Re-recording the budget is the orchestrator's.
- [ ] CHANGELOG `[0.1.0]` Changed line.

## Decisions

- **Rectangle source: the pick index, not `Plot::hover`.** `PickIndex::extent`
  beside `anchor`; looked up in `sync_overlay` when composing, so `hover` and
  its generation re-anchoring are untouched. `anchor`'s linear scan is already
  paid there; `extent` doubles it once per composed overlay, not per frame.
- **Which marks**: exactly the `Shape::Box` rows (bars, histogram bins, heatmap
  cells, box plot boxes, rect marks). Lines, areas and segments keep the vertex
  or midpoint ring: a line's focus unit is a vertex, and an area's pick is a
  vertex of the top edge; outlining a polygon is a larger design.
- **Inset, not outset**, with the background edge outermost: matches the
  existing ring's layering (so the contrast guarantee carries over), and never
  paints onto adjacent bars or cells in a dense histogram or heatmap.
- **Small marks fall back to the vertex ring** (threshold above) rather than a
  special thin-mark style.
- **Shared code**: generalise `ring_item` into a rectangle form
  `rect_ring_item(theme, rect, ink_width, z) -> (Item, Rect)` and make the
  square `ring_item` call it with `Rect::new(l, t, outer, outer)`. The square
  ring's output stays byte-identical (guarded by the existing goldens).
- **Layout fields keep their names**; their doc comments change from "the ring's
  outer box" to "the outline's outer box (the mark's rectangle for a box-shaped
  mark)".
- **Z order, colours, `hover_highlight` opt-out, one-highlight-at-a-time**: as
  GUP-676. No new public API.

## Escalation Triggers

- The rectangle in the pick index is in different units or a different frame
  from the anchor for any box layer (for example a horizontal bar or a window
  view), so the outline lands off the mark in a golden.
- A box plot's `Shape::Box` is the whole whisker extent, not the box, so the
  outline is the wrong rectangle.
- A heatmap's cells are less than 6 px across in the golden scene, so every case
  takes the fallback and the golden shows no outline.
- Any golden outside AC4's list changes.
- The extra lookup shows in the interaction benchmark (`PERF_BUDGETS.md`).
- WASM growth over 300 bytes gz.

## Out of Scope

- Outlining areas, lines, segments, paths or discs (they keep GUP-619/GUP-676
  rings); dimming or recolouring marks; legend-hover highlighting.
- Changing the tooltip, its anchor or the focus order.
- A bar scene in the browser smoke test.

## Technical Tasks

- [ ] `PickIndex::extent`, `Plot::pick_extent`, with unit tests per shape.
- [ ] `rect_ring_item`; `ring_item` through it; fallback rule.
- [ ] `sync_overlay`: choose outline or ring for hover and focus.
- [ ] Update `Layout::focus` and `Layout::hover` docs.
- [ ] Tests and four goldens; read each PNG.
- [ ] The AC4 search; update affected tests; list them.
- [ ] CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-676: Hovered Row Is Highlighted ✅ — the hover ring and `Layout::hover`.
- GUP-619: Keyboard Focus Over Rows ✅ — the focus ring.
- GUP-625: High Contrast Theme ✅ — ring colours.

## Testing Strategy

- **Unit tests**: `extent` per shape; rectangle ring geometry; fallback
  threshold.
- **Integration tests**: `hover.rs` and `keyboard.rs` cases above; four goldens;
  3:1 contrast.
- **Visual validation**: every new or re-blessed golden read.
- **Performance**: one extra lookup per composed overlay; WASM size.

## Success Metrics

- [ ] A hovered or focused bar, cell or box shows its whole outline.
- [ ] Scatter, line and area output is byte-identical; WASM growth under 300
      bytes gz.

## Risk Assessment

- **Low**: the box plot's picked box may not be the drawn box. _Mitigation_: the
  boxplot golden and an escalation trigger.
- **Low**: inset ink on a mark of a similar colour. _Mitigation_: the background
  edge is outermost, and the 3:1 test covers ink against the background.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (every golden read)
- [ ] `mask wasm-browser` passes (overlay code changed)
- [ ] WASM size: `mask wasm-size` scatter gz bytes before and after
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
