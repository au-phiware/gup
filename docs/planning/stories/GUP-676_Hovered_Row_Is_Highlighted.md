---
implementer: sonnet
implementer-reason:
  the ring drawing and overlay path exist (focus_item, sync_overlay); the style,
  default and setter shapes are decided below
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/scene.rs,
    crates/gup-core/src/builder,
    crates/gup-core/wasm-size/scatter,
    crates/gup-core/PERF_BUDGETS.md,
    CHANGELOG.md,
  ]
---

# GUP-676: The Hovered Row Is Highlighted

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 📋
Ready **Created**: 2026-11-08

## Context

The feature tour (GUP-669, chapter 8) found that hovering draws a tooltip but
nothing on the hovered mark, so with close points the reader must guess which
one the tooltip names. Keyboard focus already draws a ring (GUP-619, with the
high-contrast theme's title-coloured ring from GUP-625).

The mechanism: focus is a hover with a flag. `Plot` holds one
`hover: Option<(RowId, Point, u64)>` (row, anchor, generation) and
`hover_from_keys: bool`; `Plot::hovered()` is the row either way and
`Plot::focused()` is the hover only when `hover_from_keys`. In
`Plot::sync_overlay` (`plot.rs`) the ring is `focus_item(&theme, anchor)` (16 px
square of 2 px sides in `theme.title`, inside an 18 px square of 1 px sides in
`theme.background`), pushed at `Z_FOCUS` (275, above the brush, below the
tooltip `Z_TOOLTIP` 300) onto the cloned base scene, and its outer box is
recorded in `Layout::focus`. The anchor comes from the pick index
(`pick_anchor`), the same point for every mark type, so the ring already works
for scatter, line, bar, area and the rest; hover needs no per-mark code.

Because there is one hover state, pointer hover and keyboard focus never name
two rows: a pointer move onto a row ends focus (`set_hover` clears
`hover_from_keys`), and a key sets the hover from keys. So the "both at once"
question does not arise.

## User Story

> "As a person reading a chart, I want the point under my pointer marked, so
> that I know which mark the tooltip is describing when points are close."

## Read First

- `crates/gup-core/src/plot.rs`: `focus_item` (about line 370), `sync_overlay`
  (about 1516), `Plot::hovered` and `focused` (about 1612), `Layout::focus`
  (about line 79).
- `crates/gup-core/src/scene.rs` lines 21 to 26 (`Z_*`).
- `crates/gup-core/tests/keyboard.rs`: the golden test and the ring tests
  (`layout.focus`).
- `crates/gup-core/wasm-size/scatter/src/lib.rs` (`hovered_row`, `tooltip_rect`,
  about 960 to 980) and `index.html` steps `key-focus` and `tooltip` (about 286
  and 337).
- GUP-619 and GUP-625 Implementation Summaries.
- How an existing `Plot` setter such as `tooltip` is forwarded by the builders
  (`crates/gup-core/src/builder/scatter.rs` line 221 and the sibling files).

## Acceptance Criteria

### AC1: A hovered row is ringed

- [ ] With the pointer over a row, `sync_overlay` pushes a highlight item
      (`Z_HOVER`, below `Z_FOCUS`) at the hover's anchor and sets
      `Layout::hover: Option<Rect>` (new, the ring's outer box). With the
      pointer not over a row, or with keyboard focus, `Layout::hover` is `None`.
- [ ] Keyboard focus still draws exactly the GUP-619 ring and `Layout::hover` is
      `None` then (no double ring).
- [ ] Test (new, in `tests/keyboard.rs` or a new `tests/hover.rs`): scatter,
      pointer over a point: `layout.hover` is `Some`, centred within 1 px of
      `hovered`'s anchor, and `layout.focus` is `None`; moving off the row sets
      `hover` `None`.

### AC2: Goldens

- [ ] Golden `tests/golden/gup_core/hover_highlight_scatter.png`: a scatter with
      the pointer over one of two close points; ring visible on that point and
      the tooltip beside it. The implementer reads the PNG and reports that the
      ring is centred on the named point and distinct from the focus ring
      (thinner).
- [ ] Golden `hover_highlight_bar.png`: a bar chart with the pointer over one
      bar; the ring sits at the bar's anchor as the focus ring does for the same
      bar (read by eye; report where it falls). If it looks wrong on bars (for
      example floating above a short bar), see the Escalation Triggers.
- [ ] A dark-theme and a `Theme::high_contrast()` assertion (no golden): the
      ring ink is `theme.title` and its edge `theme.background`, with contrast
      against the background at least 3:1 (as the GUP-625 test checks the focus
      ring).

### AC3: Opt-out and default

- [ ] `Plot::hover_highlight(&mut self, on: bool) -> &mut Self`, default `true`,
      and the same setter on every chart builder that forwards `tooltip` (all
      builders, one pass; follow the existing forwarding pattern). With `false`
      the scene is byte-identical to the one without a hover ring.
- [ ] A doctest on `Plot::hover_highlight` and a test from an external crate
      path (an integration test file) that calls it on `Plot` and on at least
      one builder.

### AC4: Nothing changes without a hover

- [ ] With nothing hovered, every existing golden and SVG snapshot is
      byte-identical (run the whole suite; no golden is re-recorded except the
      two new ones).
- [ ] Search every test, example, doctest and browser scene that hovers by hand
      (grep `Input::pointer`, `PointerMove`, `Hover`, `tooltip_rect` under
      `crates/` and `examples/`) and list which are affected; update any
      assertion that counts scene items or compares pixels under a hover, or opt
      it out with `hover_highlight(false)`. Report the list.

### AC5: Browser smoke test and size

- [ ] The scatter app exposes `hover_ring_rect()` (mirroring `tooltip_rect`,
      returning `Layout::hover` as `[x, y, w, h]` or empty). A new step in
      `index.html` after the tooltip step asserts it is present, lies within 3
      px of the hovered row's `row_px` anchor, and is absent after the pointer
      leaves. Headless WebGPU readback is transparent, so the check is the CPU
      counter, as for `tooltip_rect`. `mask wasm-browser` passes.
- [ ] `mask wasm-size` reported before and after; growth under 1 KB gz. Over
      that, trigger an escalation. Re-recording the budget is the
      orchestrator's.
- [ ] CHANGELOG `[0.1.0]` Added line.

## Decisions

- **Reuse the focus ring's drawing with a distinct style.** A new
  `hover_item(theme, anchor) -> (Item, Rect)` beside `focus_item`: a 14 px
  square of 1 px sides in `theme.title` inside a 16 px square of 1 px sides in
  `theme.background` (outer box 16 px, whole-pixel edges as `focus_item`).
  Thinner and smaller than the focus ring (2 px sides, 18 px), visible on any
  mark or background, and the same colours, so the high-contrast theme's
  guarantee carries over. The identical ring was rejected: a reader should be
  able to tell a pointer hover from keyboard focus.
- **Share the rect-building code**: `focus_item` and `hover_item` call one
  private helper taking `(half_outer, ink_width)`; the focus ring's output stays
  byte-identical (guarded by the existing golden).
- **One highlight at a time**: because hover and focus share one state, no
  special handling. Hover ring when `hover.is_some() && !hover_from_keys`; focus
  ring when `hover_from_keys`.
- **Z order**: `Z_HOVER = 270` in `scene.rs`, below `Z_FOCUS`, above the brush,
  below the tooltip.
- **Default on**, because a tooltip without a highlight is ambiguous. Setter
  `hover_highlight(bool)` on `Plot` and every builder.
- **All marks**: the ring goes at the pick anchor, as focus does, so scatter,
  line (nearest vertex), bar (the bar's anchor), area, histogram, box and
  heatmap cells follow focus. Marks that are `focusable(false)` (box whiskers,
  caps, medians, area outlines) are still pickable for hover only if they are
  today; the ring follows whatever `hovered()` returns, with no extra rules.
- **`Layout::hover`** is a new public field next to `Layout::focus`
  (`Option<Rect>`), updating every struct literal of `Layout`.
- A hover that has no tooltip text still gets its ring (as focus does).
- No outline-of-the-mark highlight (bar fill change, dimming other marks): a
  separate, larger design.

## Escalation Triggers

- The anchor of a bar, area or box falls somewhere that makes the ring
  misleading (floating off the bar, on a neighbour) in the bar golden.
- The 1 px edge of the ring is not visible on a dense scatter or on the dark
  theme when read by eye.
- A `Layout` struct literal outside `plot.rs` makes adding the field a wide
  change, or any existing golden changes.
- WASM growth is over 1 KB gz.
- The hover overlay is rebuilt per pointer move and the extra item measurably
  slows the interaction benchmark.

## Out of Scope

- Changing the tooltip or the focus ring.
- Highlighting by dimming or recolouring the mark; highlighting all rows of a
  series; legend-hover highlighting.
- GPU-picked layers whose anchor is only the pointer (they get the ring at the
  pointer anchor like focus does; no new work).
- Editing the tour story GUP-669.

## Technical Tasks

- [ ] Add `Z_HOVER`, the shared ring helper, `hover_item` and `Layout::hover`.
- [ ] Add the `hover_highlight` field, setter and builder forwarding.
- [ ] Push the item in `sync_overlay` and include it in the "nothing to compose"
      check.
- [ ] Add the scatter app accessor and browser step.
- [ ] Add the tests and two goldens; read the PNGs.
- [ ] Do the AC4 search; update the affected tests and scenes.
- [ ] CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-619: Keyboard Focus Over Rows ✅ — the ring and overlay path.
- GUP-625: High Contrast Theme ✅ — the ring's colours.

## Testing Strategy

- **Unit tests**: `hover_item` geometry; `Layout::hover` set and cleared.
- **Integration tests**: scatter and bar goldens; setter from an external path.
- **Visual validation**: both goldens read by eye.
- **Performance**: the overlay path is unchanged in cost (one extra eight-rect
  item); WASM size reported.

## Success Metrics

- [ ] A hovered point or bar shows a ring, clearly distinct from the focus ring.
- [ ] No existing golden changed; WASM growth under 1 KB gz.

## Risk Assessment

- **Low**: the anchor for bars may not look right. _Mitigation_: the bar golden
  and an escalation trigger.
- **Low**: default-on changes pixels under hover in tests and browser scenes
  that hover by hand. _Mitigation_: the AC4 search.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (both goldens read)
- [ ] `mask wasm-browser` passes (the hover step is new)
- [ ] WASM size: `mask wasm-size` scatter gz bytes before and after
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
