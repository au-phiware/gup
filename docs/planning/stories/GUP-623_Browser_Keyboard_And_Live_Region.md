---
implementer: sonnet
implementer-reason:
  a keydown listener beside the pointer listeners, two DOM attributes and one
  sibling element, checked by new steps in the existing browser smoke test
touches:
  [
    crates/gup-core/src/web.rs,
    crates/gup-core/Cargo.toml,
    crates/gup-core/wasm-size/scatter/,
    scripts/browser_smoke.mjs,
  ]
---

# GUP-623: Browser Keyboard and Live Region

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: ✅ Complete (2026-11-07)
**Created**: 2026-11-04

## Context

In the browser, `CanvasApp` (`crates/gup-core/src/web.rs`) listens for pointer,
wheel and resize events (`attach`, line 484) and names the canvas with
`role="img"` and the chart's description as `aria-label` (GUP-593,
`update_label`, line 280). The canvas cannot take keyboard focus (no
`tabindex`), no key reaches the chart, and nothing tells a screen reader what
the keyboard focus is on. GUP-619–621 give the chart keys and a
`Chart::focus_text()`; this story connects them in the browser:

- the canvas becomes focusable and forwards keys as `Input::KeyDown`;
- a visually hidden `aria-live="polite"` element after the canvas speaks
  `focus_text()` whenever it changes.

## Read First

- `crates/gup-core/src/web.rs`: `State` and `update_label` (lines 240–300),
  `CanvasApp::new` (line 424), `listen` and `attach` (lines 454–565), `Drop`
  (line 587).
- `crates/gup-core/wasm-size/scatter/index.html` lines 180–240 (the `aria-label`
  steps and the `step` helper) and the `show_scatter` harness in
  `crates/gup-core/wasm-size/scatter/src/lib.rs` (line 792; `hovered_row`,
  `row_px`, `tooltip_text`).
- RFC-001 "S12b findings" (lines 3654–3670): never read back a WebGPU canvas in
  the headless browser.
- GUP-622's story file, "Decisions" (the key mapping, shared with this story).

## User Story

> "As a keyboard and screen-reader user of a web page with a Gup chart, I want
> to Tab to the chart, move through its points with the arrow keys and hear each
> one, so that the chart is usable without a mouse or sight."

## Acceptance Criteria

### AC1: A focusable canvas that forwards keys

- [x] `CanvasApp::new` sets `tabindex="0"` on the canvas unless the page set a
      `tabindex` of its own (as `aria-label` is respected today).
- [x] A `keydown` listener on the canvas maps `KeyboardEvent.key` with GUP-622's
      mapping (`"ArrowLeft"`, …, `"Home"`, `"End"`, `"Enter"`, `" "`,
      `"Escape"`, `"+"`, `"="`, `"-"`, `"0"`) and the event's modifier flags,
      dispatches `Input::KeyDown`, and calls `preventDefault()` only for keys it
      mapped (never Tab), so arrows and Space do not scroll the page while the
      canvas has focus.
- [x] The listener is not passive (it must be able to prevent the default) and
      is removed on drop with the others.

### AC2: Live region

- [x] `CanvasApp::new` inserts, immediately after the canvas, a `div` with
      `aria-live="polite"`, `aria-atomic="true"`, a `data-gup-live` attribute
      and the visually hidden style of Decisions; it is removed on drop.
- [x] After each frame, when `chart.focus_text()` differs from the text last
      written, the region's `textContent` becomes the new text (empty string
      when focus clears). The DOM is written only on change.

### AC3: Browser smoke test

- [x] New steps in `crates/gup-core/wasm-size/scatter/index.html`, after the
      `aria-kept` step: the canvas has `tabIndex === 0`; dispatching
      `new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })` on
      the canvas makes `app.hovered_row()` the row with the smallest x pixel
      (from `app.row_px(i)` over all rows); the live region's text equals that
      row's tooltip with line breaks replaced by `"; "` plus `" (1 of 6)"`; the
      event's `defaultPrevented` is true; `Escape` clears the hover and empties
      the region.
- [x] A canvas whose page set `tabindex="-1"` keeps it (a second canvas, as the
      `aria-kept` step does).
- [x] `mask wasm-browser` passes and prints `GUP PASS`.

## Decisions

- **Visually hidden style**:
  `position:absolute;width:1px;height:1px;margin:-1px;padding:0;border:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap`.
  The standard pattern: read by screen readers, invisible, takes no layout
  space.
- **Placement**: `canvas.after(div)` (an `Element` method; add web-sys features
  as needed, likely `Document` and `KeyboardEvent`). Not `aria-describedby`: the
  live region announces changes; the canvas's name stays the description.
- **Politeness**: `polite`, so an announcement does not cut off the screen
  reader mid-sentence; repeated identical text is not re-announced (written only
  on change).
- **Focus outline**: Gup draws its own focus ring on the focused row (GUP-619);
  the canvas's browser focus outline is left to the page's CSS.

## Escalation Triggers

- Headless Chromium does not deliver a synthetic `keydown` to a listener on the
  canvas (then dispatch on `document.activeElement` after `canvas.focus()`, and
  report if that fails too).
- `mask wasm-size` grows by more than 4,000 B gz for this story.

## Out of Scope

- The hidden data table (GUP-624).
- Announcing hover from the pointer (only keyboard focus is announced).

## Technical Tasks

- [x] `tabindex`, the keydown listener and mapping, preventDefault rules.
- [x] The live region: create, update after frames, remove on drop.
- [x] Browser smoke test steps; web-sys features.

## Dependencies

### Prerequisite Stories

- GUP-619: Keyboard Focus Over Rows 📋.
- GUP-621: Row Announcements 📋 — `Chart::focus_text`.
- GUP-622: Window and egui Hosts Forward Keys 📋 (soft: shares the mapping;
  either may define it first).

### Enables Stories

- GUP-624: the data table sits after the live region.

## Testing Strategy

- **Integration tests**: the browser smoke steps (AC3).
- **Manual**: optional, with a screen reader (Orca on Linux); record in the
  retrospective if done.

## Success Metrics

- [x] A Gup chart in a page can be reached with Tab and read point by point.

## Risk Assessment

- **Low**: DOM wiring with a test in a real browser. _Mitigation_: AC3.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, DOM only (the ring is
      GUP-619's)
- [x] `mask wasm-browser` passes (AC3)
- [x] WASM size: `mask wasm-size` scatter gz bytes
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/input.rs`: `Input::from_key_name(&str, Modifiers)`
  (`pub(crate)`), GUP-622's mapping keyed by W3C `KeyboardEvent.key` names.
  `app.rs::key_input` now converts winit keys to those names and calls it, so
  the window and browser hosts share one table (its unit tests still pass).
- `crates/gup-core/src/web.rs`: `tabindex="0"` unless set, a non-passive
  `keydown` listener (`preventDefault` only for mapped keys), the live region
  (`canvas.after`, removed on drop) written after each frame only on change.
- Harness: `InteractiveScatter::focus_text`; new smoke steps `tabindex-kept`,
  `live-region-added/removed`, `tabindex`, `live-region`, `key-focus`,
  `key-announced`, `key-default-prevented`, `key-escape`.
- Smoke output: `GUP live region: "Tui; x 10 (1 of 6)"`,
  `GUP PASS ... interactive=ok`.
- WASM scatter gz: 370,055 B (before, same session) to 371,571 B (after), +1,516
  B.
- Decision: the live-region expectation is `focus_text()`; for the scatter that
  still equals the tooltip with `"; "` plus `" (1 of 6)"`, so the AC text holds
  and the test asserts both.

## Retrospective

**Completed**: 2026-11-07

### Key Technical Learnings

#### One key table for three hosts

- **Challenge**: GUP-622's mapping took winit types, unusable on wasm32.
- **Solution**: a string-keyed `Input::from_key_name` using W3C names; winit
  converts to names first.
- **Pattern**: share a table at the lowest common representation, not a type.

### Architectural Decisions

#### Live region owned by `State`

- **Decision**: the element lives in `State` and is removed in `Drop`.
- **Trade-off**: the page cannot supply its own region; GUP-624 may want that.

### Development Workflow Insights

- The worktree sandbox refuses long compound shell commands with heredocs; write
  scripts with the Write tool and run them plainly.
- Not run with a screen reader (no Orca).

### Follow-up Stories

None.
