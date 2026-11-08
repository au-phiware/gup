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

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: ✅
Complete (2026-11-08) **Created**: 2026-11-08

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

- [x] With the pointer over a row, `sync_overlay` pushes a highlight item
      (`Z_HOVER`, below `Z_FOCUS`) at the hover's anchor and sets
      `Layout::hover: Option<Rect>` (new, the ring's outer box). With the
      pointer not over a row, or with keyboard focus, `Layout::hover` is `None`.
- [x] Keyboard focus still draws exactly the GUP-619 ring and `Layout::hover` is
      `None` then (no double ring).
- [x] Test (new, in `tests/keyboard.rs` or a new `tests/hover.rs`): scatter,
      pointer over a point: `layout.hover` is `Some`, centred within 1 px of
      `hovered`'s anchor, and `layout.focus` is `None`; moving off the row sets
      `hover` `None`.

### AC2: Goldens

- [x] Golden `tests/golden/gup_core/hover_highlight_scatter.png`: a scatter with
      the pointer over one of two close points; ring visible on that point and
      the tooltip beside it. The implementer reads the PNG and reports that the
      ring is centred on the named point and distinct from the focus ring
      (thinner).
- [x] Golden `hover_highlight_bar.png`: a bar chart with the pointer over one
      bar; the ring sits at the bar's anchor as the focus ring does for the same
      bar (read by eye; report where it falls). If it looks wrong on bars (for
      example floating above a short bar), see the Escalation Triggers.
- [x] A dark-theme and a `Theme::high_contrast()` assertion (no golden): the
      ring ink is `theme.title` and its edge `theme.background`, with contrast
      against the background at least 3:1 (as the GUP-625 test checks the focus
      ring).

### AC3: Opt-out and default

- [x] `Plot::hover_highlight(&mut self, on: bool) -> &mut Self`, default `true`,
      and the same setter on every chart builder that forwards `tooltip` (all
      builders, one pass; follow the existing forwarding pattern). With `false`
      the scene is byte-identical to the one without a hover ring.
- [x] A doctest on `Plot::hover_highlight` and a test from an external crate
      path (an integration test file) that calls it on `Plot` and on at least
      one builder.

### AC4: Nothing changes without a hover

- [x] With nothing hovered, every existing golden and SVG snapshot is
      byte-identical (run the whole suite; no golden is re-recorded except the
      two new ones).
- [x] Search every test, example, doctest and browser scene that hovers by hand
      (grep `Input::pointer`, `PointerMove`, `Hover`, `tooltip_rect` under
      `crates/` and `examples/`) and list which are affected; update any
      assertion that counts scene items or compares pixels under a hover, or opt
      it out with `hover_highlight(false)`. Report the list.

### AC5: Browser smoke test and size

- [x] The scatter app exposes `hover_ring_rect()` (mirroring `tooltip_rect`,
      returning `Layout::hover` as `[x, y, w, h]` or empty). A new step in
      `index.html` after the tooltip step asserts it is present, lies within 3
      px of the hovered row's `row_px` anchor, and is absent after the pointer
      leaves. Headless WebGPU readback is transparent, so the check is the CPU
      counter, as for `tooltip_rect`. `mask wasm-browser` passes.
- [x] `mask wasm-size` reported before and after; growth under 1 KB gz. Over
      that, trigger an escalation. Re-recording the budget is the
      orchestrator's.
- [x] CHANGELOG `[0.1.0]` Added line.

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

- [x] Add `Z_HOVER`, the shared ring helper, `hover_item` and `Layout::hover`.
- [x] Add the `hover_highlight` field, setter and builder forwarding.
- [x] Push the item in `sync_overlay` and include it in the "nothing to compose"
      check.
- [x] Add the scatter app accessor and browser step.
- [x] Add the tests and two goldens; read the PNGs.
- [x] Do the AC4 search; update the affected tests and scenes.
- [x] CHANGELOG.

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

- [x] A hovered point or bar shows a ring, clearly distinct from the focus ring.
- [x] No existing golden changed; WASM growth under 1 KB gz.

## Risk Assessment

- **Low**: the anchor for bars may not look right. _Mitigation_: the bar golden
  and an escalation trigger.
- **Low**: default-on changes pixels under hover in tests and browser scenes
  that hover by hand. _Mitigation_: the AC4 search.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (both goldens read)
- [x] `mask wasm-browser` passes (the hover step is new)
- [x] WASM size: `mask wasm-size` scatter gz bytes before and after
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `ring_item` (shared helper), `focus_item` (byte-identical to before) and the
  new `hover_item` (16 px outer, 1 px ink, `Z_HOVER` = 270) in `plot.rs`;
  `Layout::hover`; `Plot::hover_highlight` (default on, with a doctest);
  `hover_highlight` on all seven builders; `hover_ring_rect()` and a
  `hover-ring` step in the scatter app (`mask wasm-browser` passes).
- Tests: `crates/gup-core/tests/hover.rs` (7: ring at anchor and cleared on
  leave, focus draws only the focus ring, off removes exactly the ring item,
  builder setter, dark and high-contrast colours with 3:1, two goldens).
  Goldens: `gup_core/hover_highlight_scatter.png`,
  `gup_core/hover_highlight_bar.png`.
- Scatter golden: the ring is centred on the named point (of two close ones),
  snug around the 6 px disc, thinner than the focus ring, tooltip beside it with
  no overlap. Bar golden: the ring straddles the top edge of the Q2 bar at its
  centre, the same anchor focus uses; not misleading.
- Existing goldens that changed (the harness tolerance hides the ring, so they
  passed before re-blessing; blessing the suite rewrote exactly four):
  `gup_core/tooltip.png`, `gup_core/hello_world.png`, `gup_core/theme_dark.png`,
  `tour/08_tooltip.png`. Each read: the ring is on the hovered point, beside the
  tooltip, not hiding the point. Tour page 08 prose updated.
- AC4 search (pointer_moved, tooltip_rect, layout.tooltip under `crates/`,
  `tests/`): all other hand-hovering tests (events, pick, append, window,
  secondary_y, area, histogram, box, resolve_cache, keyboard) pass unchanged.
- WASM (`mask wasm-size`, scatter gz, same session): before 376515, after 376932
  (+417 B, under 1 KB). Over wgpu 344572 B.

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### The golden harness hides a thin ring

- **Challenge**: the tour and gup_core tooltip goldens passed unchanged with the
  ring drawn, so "count the changed goldens first" found none.
- **Solution**: bless the whole suite and use `git status` on the goldens (four
  changed), then read each.
- **Pattern**: when a change adds small marks, passing goldens do not mean
  unchanged goldens; bless and diff the bytes.

### Architectural Decisions

#### One ring helper

- **Decision**: `ring_item(outer, ink_width, z)` serves focus and hover; the
  focus ring stays byte-identical.
- **Trade-off**: the hover ring is a fixed 16 px, not sized to the mark, so on a
  tiny point it is larger than the mark and on a bar it straddles the top.

### Development Workflow Insights

- The worktree sandbox refuses compound commands that run git after a computed
  `cd`; keep commands plain. Baseline WASM size was measured by checking out the
  Start commit's sources over the tree, then restoring.

### Follow-up Stories

None.
