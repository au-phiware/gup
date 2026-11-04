---
implementer: sonnet
implementer-reason:
  three key cases in Plot::handle that call the wheel's and drag's existing
  zoom_axes and pan_from; steps and directions are settled in Decisions
touches: [crates/gup-core/src/plot.rs, crates/gup-core/tests/keyboard.rs]
---

# GUP-620: Keyboard Zoom, Pan and Reset

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: 📋 Planned **Created**:
2026-11-04

## Context

GUP-619 adds `Input::KeyDown` and the semantic keys `ZoomIn`, `ZoomOut` and
`ResetView`, and leaves them, and Shift+arrows, without effect. Today the view
moves only by the wheel (`Input::Wheel`, `plot.rs` `handle`, which calls
`zoom_axes(at, ZOOM_PER_LINE.powf(lines), self.movable())`) and by a primary
drag (`pan_from`). A keyboard user can walk the rows in view but cannot change
the view, which fails WCAG 2.1.1 for any chart whose interesting rows are
off-screen or overlapping. This story wires the keys to the same calls.

## Read First

- `crates/gup-core/src/plot.rs`: `ZOOM_PER_LINE` (line 380), `zoom`,
  `zoom_axes`, `pan`, `pan_from` (around lines 1015–1105), `reset_view` (around
  line 1106), `movable` (around line 1174), and the `Input::Wheel` arm of
  `handle` (around line 1560).
- `crates/gup-core/src/input.rs` (`Key`, `Modifiers` after GUP-619).
- GUP-619's story file, "Decisions" (the key set and focus rules).

## User Story

> "As a keyboard user, I want to zoom, pan and reset a chart's view with keys,
> so that I can reach rows that are off-screen or crowded together."

## Acceptance Criteria

### AC1: Zoom keys

- [ ] `Key::ZoomIn` zooms the movable axes about the centre of `Layout::plot` by
      `ZOOM_PER_LINE.powi(2)`, and `Key::ZoomOut` by its inverse: tested by
      comparing the domains with those after `Input::wheel(centre, 2.0)` and
      `Input::wheel(centre, -2.0)` on twin plots (equal within 1e-12 relative).
- [ ] Both respect `view_axes` (with `Axes::X`, y domains are unchanged) and
      leave band and point axes alone, as the wheel does.

### AC2: Pan keys

- [ ] Shift+Right moves the view right by a tenth of the plot width: the x
      domain's ends both increase by 10% of its span on a linear axis (content
      moves left 0.1 × `plot.width` pixels). Shift+Left is the opposite.
      Shift+Up shows higher values: the y domain's ends increase by 10% of its
      span; Shift+Down the opposite. Tested on a linear plot, and on a log x
      axis by pixels (a value drawn at pixel p is drawn at p ∓ 0.1 × width after
      the key).
- [ ] Pan keys respect `view_axes`, as the drag does, and are refused on band
      and point axes (no error surfaces; the axis just does not move).
- [ ] Arrows without Shift keep GUP-619's row walk.

### AC3: Reset and focus

- [ ] `Key::ResetView` does what `reset_view()` does and returns `redraw: true`
      only if the view had moved.
- [ ] Keyboard focus survives a key zoom or pan while its row stays in the plot;
      the ring and tooltip follow the row to its new anchor. When the row leaves
      the plot, focus clears (its `Leave` handlers run), as GUP-619 does for a
      wheel zoom.
- [ ] Before the first resolve every view key does nothing and returns
      `redraw: false`.

### AC4: Docs

- [ ] `Plot::handle`'s doc comment lists the keys (row walk, Enter, Escape,
      zoom, pan, reset) in one bulleted block.

## Decisions

- **Zoom step**: two wheel lines (`ZOOM_PER_LINE.powi(2)` ≈ 0.72 per press), so
  a key press equals a small wheel turn.
- **Pan step**: 10% of the plot rect's width (horizontal) or height (vertical)
  per press, through `pan_from(self.domains(), (dx, dy), self.movable())` with
  `dx = ∓0.1 × plot.width` (Shift+Right gives `dx < 0`: content moves left) and
  `dy = ±0.1 × plot.height` (Shift+Up gives `dy > 0`: content moves down, higher
  values come into view).
- **Ctrl, Alt, Meta**: any of them set means the key is not Gup's; return
  `redraw: false` and do nothing (hosts and browsers own those shortcuts).
- **Hover from the pointer**: a key zoom or pan does not clear a pointer hover
  first (unlike the wheel); the hover re-anchors or leaves at the next resolve,
  as it does after a programmatic `zoom`.

## Escalation Triggers

- `pan_from` errors on a band axis even when `movable()` excludes it (the keys
  would then need their own filtering).
- Keeping focus across a zoom requires re-picking rather than re-anchoring.

## Out of Scope

- Hosts mapping physical keys (GUP-622, GUP-623).
- Keyboard brushing.

## Technical Tasks

- [ ] The three key cases and Shift+arrows in `handle`.
- [ ] Tests in `crates/gup-core/tests/keyboard.rs`; the doc comment.

## Dependencies

### Prerequisite Stories

- GUP-619: Keyboard Focus Over Rows 📋 — `Input::KeyDown`, `Key`, `Modifiers`,
  focus.

## Testing Strategy

- **Integration tests**: domains after keys versus the wheel and drag
  equivalents; focus across a zoom.

## Success Metrics

- [ ] Every pointer view operation except brushing has a key.

## Risk Assessment

- **Low**: reuses the wheel and drag paths. _Mitigation_: AC1 compares with the
  wheel directly.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, no image changes (domains are
      checked numerically)
- [ ] `mask wasm-browser`: run it; input handling changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
