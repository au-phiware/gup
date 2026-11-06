---
implementer: sonnet
implementer-reason:
  the key set, focus order, focus ring and composite-layer rule are settled in
  Decisions; the work reuses hover (set_hover, pick_anchor) and brush_item
touches:
  [
    crates/gup-core/src/input.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/pick.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/builder/boxplot.rs,
    crates/gup-core/src/builder/area.rs,
    crates/gup-core/tests/keyboard.rs,
  ]
---

# GUP-619: Keyboard Focus Over Rows

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: ✅ Complete (2026-11-06)
**Created**: 2026-11-04

## Context

Release 0.1 shipped a text alternative (GUP-592/593) and nothing else for
accessibility (owner decision 4, November review). A chart's rows can only be
explored with a pointer: `Input` (`crates/gup-core/src/input.rs`) has pointer
and wheel variants only, and `Plot::handle` (`plot.rs` around line 1445) moves
the hover, fires `Enter`/`Leave`/`Click` handlers and shows tooltips from them.
A keyboard user cannot reach a single data point (WCAG 2.1.1 Keyboard).

The hover machinery is most of what keyboard focus needs: `set_hover(row, at)`
(around line 1759) fires the handlers and positions the tooltip at
`pick_anchor(row)`, and `sync_overlay` composes the tooltip. This story adds key
input to `Input`, makes focus a keyboard-driven hover, defines the order the
keys walk, and draws a focus ring (WCAG 2.4.7 Focus Visible). Hosts forwarding
keys (window, egui, browser) are GUP-622 and GUP-623; zoom and pan keys are
GUP-620; what a screen reader hears is GUP-621.

## Read First

- `crates/gup-core/src/input.rs` (all of it, 230 lines).
- `crates/gup-core/src/plot.rs`: `handle` (around lines 1422–1560), `set_hover`
  and `dispatch` (around lines 1757–1790), `sync_overlay` (around lines
  1346–1405), `brush_item` (around line 314), `pick_index` (around line 1912).
- `crates/gup-core/src/pick.rs`: `PickIndex` (around line 463) and `anchor`
  (around line 638), for how line, path and area items carry two rows.
- `crates/gup-core/src/builder/boxplot.rs` lines 300–380 (its five layers) and
  `crates/gup-core/src/builder/area.rs` around line 481 (outline layers).

## User Story

> "As a keyboard user, I want to move through a chart's data points with the
> arrow keys and hear or see each one, so that I can read values without a
> mouse."

## Acceptance Criteria

### AC1: Key input

- [x] `Key`, `Modifiers` and `Input::KeyDown` exist as in Decisions, with
      `Input::key(key)` and `Input::key_with(key, modifiers)` constructors, and
      `Key` and `Modifiers` re-exported at the crate root (not in the prelude).
- [x] A doctest on `Key` sends `Input::key(Key::Right)` to a resolved two-row
      scatter and asserts `plot.focused()` is the first row.

### AC2: Focus is a keyboard hover

- [x] `Plot::focused(&self) -> Option<RowId>` returns the row focused by the
      keyboard, `None` when there is none or the hover came from the pointer.
- [x] Focusing a row calls `set_hover(Some(row), anchor)`: its `Enter` handlers
      run, the previous row's `Leave` handlers run, and its tooltip (if the
      layer has one) is shown at the row's anchor. `Plot::hovered()` equals
      `focused()` while the keyboard holds focus.
- [x] A pointer move that changes the hover ends keyboard focus (`focused()`
      becomes `None`); a pointer move that does not change the hovered row
      leaves focus alone.
- [x] `Response::redraw` is true whenever focus changes.

### AC3: The walk

- [x] The keys do what the table in Decisions says, tested in
      `crates/gup-core/tests/keyboard.rs` on: a 5-row scatter whose rows are not
      in x order (Right visits them in x order), a 3-series line chart (Up and
      Down change series and land on the nearest x), a grouped bar chart, and a
      box plot (Left/Right walk boxes; whiskers, caps and medians are never
      focused; Up reaches the outliers' layer).
- [x] Enter on a focused row runs its `Click` handlers (`Plot::on_row` sees
      `Event::Click` with that row); Escape clears focus (`Leave` runs) and
      returns `redraw: true`; Escape with nothing focused returns
      `redraw: false`.
- [x] Rows outside the current view (culled, or outside the zoomed domain) are
      not visited; a zoom that moves the focused row out of the plot clears
      focus as it clears a hover.
- [x] Keys before the first resolve, and on a plot with no focusable row, do
      nothing and return `redraw: false`.

### AC4: Focus ring

- [x] While `focused()` is `Some`, the overlay scene draws the ring of Decisions
      centred on the row's anchor; it is not drawn for a pointer hover.
- [x] A golden `gup_core/keyboard_focus` (a scatter with a tooltip, the third
      row focused) is read by eye: a dark square ring with a light edge sits
      centred on the third disc, fully visible, under the tooltip.
- [x] `Layout` reports the ring in a new field `focus: Option<Rect>` (as it
      reports `tooltip`), so hosts and tests can find it.

### AC5: Composite builders

- [x] `Selection::focusable(&mut self, on: bool) -> &mut Self` exists (default
      true), documented: keyboard focus skips a layer with `focusable(false)`.
- [x] The box plot sets `focusable(false)` on its whiskers, caps and medians
      layers; the area builder on its outline line layers.

## Decisions

- **Types** (in `input.rs`):

  ```rust
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
  #[non_exhaustive]
  pub enum Key { Left, Right, Up, Down, Home, End, Enter, Escape, ZoomIn, ZoomOut, ResetView }

  #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
  pub struct Modifiers { pub shift: bool, pub ctrl: bool, pub alt: bool, pub meta: bool }
  impl Modifiers { pub const NONE: Self; pub const SHIFT: Self; }

  // in enum Input:
  KeyDown { key: Key, modifiers: Modifiers },
  ```

  Keys are semantic, not physical: a host maps its keyboard (GUP-622/623 map
  `+`/`=` to `ZoomIn`, `-` to `ZoomOut`, `0` to `ResetView`, Space to `Enter`).
  `ZoomIn`, `ZoomOut` and `ResetView` are defined here so hosts map every key
  once; until GUP-620 they do nothing. The name `Key` is not used elsewhere in
  the public API (the builders' `Key<T>` is private to `builder`).

- **Focusable rows**: rows of layers that are focusable, have a hit shape, and
  are in the CPU pick index (drawn in the current view; `Retain::GpuOnly` layers
  above one chunk are not in it and are skipped).
- **Order within a layer**: by anchor x, then anchor y, then row index. Built
  once per pick-index key (the same `(generation, revision)` key) from a new
  `PickIndex::anchors(&self) -> Vec<(RowId, Point)>` that walks the items once
  and yields each row once (a line, path or area item yields its `a` row, and
  its `b` row for the last item of a run), cached in `Plot` beside `pick`.
- **Keys, with no modifiers** (with Shift, arrows are GUP-620's pan; ignore them
  here):
  - **Right / Left**: with no focus, the first row of the first focusable layer;
    else the next / previous row in the focused layer's order, staying at the
    ends (no wrap).
  - **Home / End**: with no focus, the first / last row of the first layer; else
    the first / last row of the focused layer.
  - **Up / Down**: with no focus, the first row of the first layer; else the
    next / previous focusable layer in plot order, at its row whose anchor x is
    nearest the focused row's (ties: first in order), staying at the ends.
  - **Enter**: with focus, the focused row's `Click` handlers, at its anchor;
    else nothing.
  - **Escape**: with focus, clear it (`Leave`); else nothing (`redraw: false`).

  "Up" means the next layer (plot order + 1) because stacks are added bottom
  first, so Up climbs a stack.

- **State**: `hover_from_keys: bool` beside `hover`; set by key focus, cleared
  by any pointer-driven `set_hover` that changes the row, and by Escape.
  `focused()` is `hover` when the flag is set.
- **Ring**: `focus_item(theme, anchor) -> Item`, like `brush_item`: whole-pixel
  `RectPrim` sides; an outer 18 × 18 px square of 1 px sides in
  `theme.background`, and inside it a 16 × 16 px square of 2 px sides in
  `theme.title`, centred on the anchor rounded to whole pixels; z
  `Z_FOCUS = 275` (new constant in `scene.rs`, between `Z_BRUSH` and
  `Z_TOOLTIP`); clipped to nothing (drawn whole even at the plot edge).
- **GPU-picked plots** (`gpu_mode`): keyboard focus uses the CPU pick index
  only; layers it lacks are skipped, as above. No GPU pick request is made for
  keys.

## Escalation Triggers

- `PickIndex` items do not give every row of a line or area exactly once (a row
  missing or doubled in the walk).
- Making the ring part of the overlay scene requires changing how the overlay is
  cached (`overlay` keyed by generation) beyond adding the item.
- Adding `Input::KeyDown` breaks an exhaustive `match` on `Input` outside
  `gup-core` (the enum is `#[non_exhaustive]`, so none should exist); gup-egui
  and the WASM harness are the places to check.

## Out of Scope

- Zoom and pan keys (GUP-620); the announced text (GUP-621); hosts (GUP-622,
  GUP-623); a data table (GUP-624).
- Tab to move between charts (the host's focus order).

## Technical Tasks

- [x] `Key`, `Modifiers`, `Input::KeyDown`, constructors, re-exports.
- [x] `PickIndex::anchors`, the cached order, `hover_from_keys`, the key
      handling in `handle`, `focused()`.
- [x] `focus_item`, `Z_FOCUS`, `Layout::focus`.
- [x] `Selection::focusable`; box plot and area builder calls.
- [x] `tests/keyboard.rs`, the golden.

## Dependencies

### Prerequisite Stories

- GUP-563: Input Positions from Tuples ✅ — the current `Input`.
- GUP-546: Area Picking ✅ — area rows are in the pick index.

### Enables Stories

- GUP-620, GUP-621, GUP-622, GUP-623.

## Testing Strategy

- **Unit tests**: `Modifiers` constants; `PickIndex::anchors` on a line.
- **Integration tests**: `tests/keyboard.rs` (AC3), handlers via `on_row`.
- **Visual validation**: the `gup_core/keyboard_focus` golden.

## Success Metrics

- [x] Every data row in view of every builder can be reached with arrow keys.

## Risk Assessment

- **Medium**: `handle` is long and stateful (press, drag, marquee, GPU picks).
  _Mitigation_: keys touch only the hover and its flag; a drag or marquee in
  progress ignores keys (return `redraw: false`), tested.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the focus golden)
- [x] `mask wasm-browser`: run it; `Input` and `plot.rs` input handling change
- [x] WASM size: `mask wasm-size` scatter gz bytes
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Key`, `Modifiers`, `Input::KeyDown`, `Input::key` and `Input::key_with`
  (`input.rs`), re-exported at the crate root; a doctest on `Key`.
- `Plot::focused`, `hover_from_keys`, the key handling (`key_down`,
  `focus_target`, `focus_order`) in `plot.rs`; `PickIndex::anchors` in
  `pick.rs`.
- `focus_item`, `Z_FOCUS` (`scene.rs`) and `Layout::focus`.
- `Selection::focusable`; the box plot's whiskers, caps and medians and the area
  outline layers are `focusable(false)`.
- Tests: `crates/gup-core/tests/keyboard.rs` (13: scatter walk, pointer
  hand-over, Enter/Escape, guards, zoom, line, grouped bars, box plot, contrast,
  golden, dark ring), a `PickIndex::anchors` unit test, an `Input` constructor
  test. Golden `tests/golden/gup_core/keyboard_focus.png`: a dark square ring
  with a white edge centred on the third disc, under the tooltip. The dark
  theme's ring is a light ring with a dark edge (read by eye).
- WASM scatter gz: 364674 bytes (limit 367607, pass).

## Retrospective

**Completed**: 2026-11-06

### Key Technical Learnings

#### The story's name check was wrong

- **Challenge**: Decisions say `Key` is unused in the public API, but
  `encoding::Key<A>` was re-exported at the crate root.
- **Solution**: Dropped `Key` from that re-export in `lib.rs` (nothing outside
  names it; it still appears in builder signatures as an unnameable type). The
  input `Key` is now the root's `Key`.
- **Pattern**: Grep `pub use` at the root for a new public name before settling
  it in a story.

#### Focus is a hover with a flag

- **Challenge**: Keep focus consistent with picking.
- **Solution**: Rows come from the same pick index (`anchors`), so each is one
  `pick` can return; `set_hover` always clears `hover_from_keys`, and key focus
  sets it afterwards, so every pointer-driven change, zoom and re-anchor failure
  ends focus without extra code.
- **Pattern**: Derive a mode flag from the existing state transition that
  already clears it.

### Architectural Decisions

#### A key onto a row the pointer already holds

- **Decision**: No `Enter`/`Leave` events; the flag and ring are added.
- **Reasoning**: The hover did not change; the row was already entered.
- **Trade-off**: `redraw` is true though no handler ran.

#### Enter drops the brush

- **Decision**: `Enter` runs `drop_brush` like a pointer click.
- **Reasoning**: Consistency with picking's click.

#### Modifiers

- **Decision**: Any modifier at all makes arrow, Enter and Escape keys ignored
  here (not only Shift), so host shortcuts (Ctrl-Home, Alt-Left) pass.

### Development Workflow Insights

- A multi-line heredoc to a file was refused by the worktree sandbox; the Write
  tool was the way.
- `mask merge-check` outlasts a 10-minute foreground timeout; run it in the
  background.

### Follow-up Stories

None. Known limits: `Retain::GpuOnly` layers above one chunk are not focusable
(by Decision); the line-walk tie rule uses pixel x, so exact ties depend on
float rounding.
