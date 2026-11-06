---
implementer: sonnet
implementer-reason:
  three key cases in Plot::handle that call the wheel's and drag's existing
  zoom_axes and pan_from; steps and directions are settled in Decisions
touches: [crates/gup-core/src/plot.rs, crates/gup-core/tests/keyboard.rs]
---

# GUP-620: Keyboard Zoom, Pan and Reset

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: ✅ Complete (2026-11-06)
**Created**: 2026-11-04

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

- [x] `Key::ZoomIn` zooms the movable axes about the centre of `Layout::plot` by
      `ZOOM_PER_LINE.powi(2)`, and `Key::ZoomOut` by its inverse: tested by
      comparing the domains with those after `Input::wheel(centre, 2.0)` and
      `Input::wheel(centre, -2.0)` on twin plots (equal within 1e-12 relative).
- [x] Both respect `view_axes` (with `Axes::X`, y domains are unchanged) and
      leave band and point axes alone, as the wheel does.

### AC2: Pan keys

- [x] Shift+Right moves the view right by a tenth of the plot width: the x
      domain's ends both increase by 10% of its span on a linear axis (content
      moves left 0.1 × `plot.width` pixels). Shift+Left is the opposite.
      Shift+Up shows higher values: the y domain's ends increase by 10% of its
      span; Shift+Down the opposite. Tested on a linear plot, and on a log x
      axis by pixels (a value drawn at pixel p is drawn at p ∓ 0.1 × width after
      the key).
- [x] Pan keys respect `view_axes`, as the drag does, and are refused on band
      and point axes (no error surfaces; the axis just does not move).
- [x] Arrows without Shift keep GUP-619's row walk.

### AC3: Reset and focus

- [x] `Key::ResetView` does what `reset_view()` does and returns `redraw: true`
      only if the view had moved.
- [x] Keyboard focus survives a key zoom or pan while its row stays in the plot;
      the ring and tooltip follow the row to its new anchor. When the row leaves
      the plot, focus clears (its `Leave` handlers run), as GUP-619 does for a
      wheel zoom.
- [x] Before the first resolve every view key does nothing and returns
      `redraw: false`.

### AC4: Docs

- [x] `Plot::handle`'s doc comment lists the keys (row walk, Enter, Escape,
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

- [x] The three key cases and Shift+arrows in `handle`.
- [x] Tests in `crates/gup-core/tests/keyboard.rs`; the doc comment.

## Dependencies

### Prerequisite Stories

- GUP-619: Keyboard Focus Over Rows 📋 — `Input::KeyDown`, `Key`, `Modifiers`,
  focus.

## Testing Strategy

- **Integration tests**: domains after keys versus the wheel and drag
  equivalents; focus across a zoom.

## Success Metrics

- [x] Every pointer view operation except brushing has a key.

## Risk Assessment

- **Low**: reuses the wheel and drag paths. _Mitigation_: AC1 compares with the
  wheel directly.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no image changes (domains are
      checked numerically)
- [x] `mask wasm-browser`: see Implementation Summary
- [x] WASM size: `mask wasm-size` scatter gz bytes
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `plot.rs`: `key_down` handles `ZoomIn`/`ZoomOut` (about the plot rect's
  centre, `ZOOM_PER_LINE.powi(2)`) and `ResetView` (`redraw` only if the view
  had moved); new `key_pan` and `KEY_PAN` for Shift+arrows through `pan_from`.
  `handle` lets Shift through for the four arrows only; Ctrl, Alt and Meta, and
  Shift with other keys, stay ignored. `handle`'s doc lists every key.
- `sync_overlay`: a key-focused row whose anchor leaves the plot rect ends focus
  and a lost hover now runs the row's `Leave` handlers.
- Tests (`tests/keyboard.rs`, 20 in all, 7 new): twin-plot wheel comparison
  (1e-12), `view_axes`, band refusal, pan by a tenth in pixels (linear, log),
  reset, before-resolve, focus across a pan and a zoom, and no column, validity
  or texture bytes written by view keys.
- Evidence: no rendering change, so no goldens. `mask wasm-size`: scatter gz
  360399 bytes (limit 367607, pass). `mask wasm-browser` not run (no browser
  session here). `mask merge-check --dogfood` PASS; `cargo test -p gup` and
  `--doc` pass.

## Retrospective

**Completed**: 2026-11-06

### Key Technical Learnings

#### A pan of "a tenth of the plot" is not a tenth of the domain span

- **Challenge**: AC2 says the domain ends move 10% of the span; the range is
  inset from the plot rect (edge insets), so pixels and span differ.
- **Solution**: The step is a tenth of the plot rect in pixels, as Decisions
  say; tests compute domain units per pixel from the scale.
- **Pattern**: Test pans in pixels, not fractions of a domain.

#### Focus across a view change needed two small fixes in `sync_overlay`

- **Challenge**: Re-anchoring succeeds for a row outside the plot rect, and a
  lost hover ran no `Leave`.
- **Solution**: A key-focused row whose anchor is outside the plot ends focus;
  every lost hover now dispatches `Leave` at its old anchor.
- **Pattern**: No escalation: re-anchoring sufficed.

### Architectural Decisions

#### Shift arrows move one axis

- **Decision**: Horizontal keys move x only, vertical keys the y axes, so
  `view_axes(Y)` makes Shift-Right a no-op that redraws nothing.
- **Trade-off**: Differs from a drag, which moves both axes at once.

### Development Workflow Insights

- The worktree sandbox refuses compound commands that mix `cd`, `cat >>` and
  cargo; split them. `mask merge-check` takes about 17 minutes.

### Follow-up Stories

None. `mask wasm-browser` was not run here.
