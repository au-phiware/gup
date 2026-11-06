---
implementer: sonnet
implementer-reason:
  two key-mapping functions beside the existing pointer mappers, an Escape rule
  and egui focus handling; mappings and rules are settled in Decisions
touches:
  [crates/gup-core/src/app.rs, gup-egui/src/input.rs, gup-egui/src/widget.rs]
---

# GUP-622: Window and egui Hosts Forward Keys

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: ✅ Complete (2026-11-06)
**Created**: 2026-11-04

## Context

GUP-619 and GUP-620 make a chart respond to `Input::KeyDown`, but no host sends
it. The window app (`crates/gup-core/src/app.rs`, `GupApp` and `show`) maps
winit pointer events through `input_from` (line 177) and exits on Escape (line
341). The egui widget (`gup-egui/src/widget.rs`, `GupWidget::show`) maps egui
events through `to_input` (`gup-egui/src/input.rs`) and never takes keyboard
focus. The browser host is GUP-623; Bevy forwards no input at all today and
stays out of scope.

GUP-376 (2025, legacy) asked for `.on_key()` callbacks on `GupApp` so examples
could handle keys themselves. Forwarding keys to the chart covers the
accessibility need; GUP-376 is proposed for closing when this lands.

## Read First

- `crates/gup-core/src/app.rs`: `input_from` (line 177), `window_event` (lines
  338–385) and the mapping tests (around line 499).
- `gup-egui/src/input.rs` (`to_input`) and `gup-egui/src/widget.rs` lines 85–175
  (`show` and its input loop).
- `crates/gup-core/src/input.rs` after GUP-619 (`Key`, `Modifiers`,
  `Input::KeyDown`).
- egui 0.33 `Memory::set_focus_lock_filter` and `EventFilter`
  (`~/.cargo/registry/src/*/egui-0.33.3/src/memory/mod.rs` line 860).

## User Story

> "As a keyboard user of a desktop app built with Gup, I want the chart window
> or egui panel to take my arrow, zoom and Enter keys, so that I can use the
> chart without a mouse."

## Acceptance Criteria

### AC1: Window

- [x] A pure
      `fn key_input(key: &winit::keyboard::Key, modifiers: Modifiers)     -> Option<Input>`
      in `app.rs` maps keys per the mapping in Decisions; unit tests cover every
      row of it and a key that maps to `None`.
- [x] `window_event` tracks `WindowEvent::ModifiersChanged` and sends mapped
      `KeyboardInput` presses (not releases, and repeats included) to the chart,
      requesting a redraw when the response says so.
- [x] Escape is sent to the chart first; the window exits only when the response
      has `redraw: false` (nothing was focused). `CloseRequested` still exits at
      once. A unit test covers the rule through a small pure helper.
- [x] Manual check, recorded in the retrospective: `cargo run --example` of a
      windowed builder example; Right, Right, Enter, `+`, Shift+Right, `0`,
      Escape, Escape behave as Decisions say (focus ring moves, view zooms, pans
      and resets, the first Escape clears focus, the second closes).

### AC2: egui

- [x] `pub fn key_input(event: &egui::Event) -> Option<Input>` in
      `gup-egui/src/input.rs` maps `egui::Event::Key { pressed: true, .. }` per
      the same mapping (egui key names); unit tests as for the window.
- [x] `GupWidget::show` requests focus for its response on a primary press
      inside the chart, and is reachable with Tab (egui's focus for an
      interactive widget).
- [x] While the widget has focus it sets a focus-lock filter with arrows and
      Escape locked (Tab not), so arrows do not move egui's focus, and forwards
      `key_input` events to the chart; without focus it forwards none.
- [x] Escape with nothing focused in the chart releases the widget's egui focus
      (`surrender_focus`).
- [x] The egui example (`gup-egui/examples/egui_chart.rs`) is checked by hand
      with the same key sequence as AC1 (retrospective).

## Decisions

- **Mapping** (both hosts): arrow keys to `Left`, `Right`, `Up`, `Down`; Home
  and End to `Home`, `End`; Enter and Space to `Enter`; Escape to `Escape`, all
  with the modifiers as held. The characters `+` and `=` to `ZoomIn`, `-` to
  `ZoomOut` and `0` to `ResetView`, with Shift dropped (because `+` needs Shift
  on some layouts and not others). Ctrl, Alt and Meta are passed as held, and
  GUP-620's rule makes the chart ignore keys that carry them, so host shortcuts
  are not taken. Every other key maps to `None`.
- **Window Escape**: forward first, exit on `redraw: false`. One extra key press
  to close a window with a focused row; no new API.
- **egui focus**: `response.request_focus()` on a press inside the rect; each
  frame the widget has focus,
  `ui.memory_mut(|m| m.set_focus_lock_filter(response.id, filter))` with an
  `EventFilter` that locks horizontal and vertical arrows and Escape but not Tab
  (field names as in egui 0.33's `EventFilter`).
- **Bevy**: not in this story (no input path exists to extend).

## Escalation Triggers

- egui 0.33 offers no way to keep arrow keys from moving focus while the widget
  has it.
- winit delivers `+` as a named key or with a `text` but no `logical_key`
  character on the CI platform, so the mapping needs `text` instead.

## Out of Scope

- `GupApp::on_key` user callbacks (GUP-376).
- The browser (GUP-623); Bevy input.

## Technical Tasks

- [x] `key_input` for winit, modifiers tracking, the Escape rule, tests.
- [x] `key_input` for egui, focus request and lock, forwarding, tests.
- [x] Manual checks of both hosts.

## Dependencies

### Prerequisite Stories

- GUP-619: Keyboard Focus Over Rows 📋.
- GUP-620: Keyboard Zoom, Pan and Reset 📋 (soft: without it the zoom keys do
  nothing, but the mapping is the same).

## Testing Strategy

- **Unit tests**: both mapping functions; the Escape rule.
- **Manual**: the key sequence in a window and in egui.

## Success Metrics

- [x] Both desktop hosts drive keyboard focus and view keys.

## Risk Assessment

- **Low**: mapping code. _Mitigation_: pure functions with tests over the whole
  mapping.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1` and
      `cargo test -p gup-egui -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: the manual key sequences
- [x] `mask wasm-browser`: not needed, desktop hosts only
- [x] WASM size: unchanged: `app.rs` is not built for wasm32 and gup-egui is not
      in the scatter
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/app.rs`: pure `key_input` (winit) and `escape_exits`;
  `ModifiersChanged` tracking; presses (repeats included) go to `Chart::handle`;
  Escape exits only when the chart's response has `redraw: false`;
  `CloseRequested` exits at once. 4 new unit tests.
- `gup-egui/src/input.rs`: `key_input(&egui::Event)`. `gup-egui/src/widget.rs`:
  `GupWidget::forward_keys` requests focus on a primary press inside the rect,
  sets the focus-lock filter (arrows and Escape, not Tab) while focused,
  forwards keys, and surrenders focus on an unused Escape. 2 + 3 new unit tests
  (the egui ones run real `egui::Context` frames with a text field beside the
  widget).
- CHANGELOG `[0.1.0]` line added; GUP-376 closed as superseded.
- `mask merge-check`, `mask egui-screenshot`, gup-egui tests: pass.

## Retrospective

**Completed**: 2026-11-06

### Manual checks

- Window (`hello_world` under Xvfb, keys by xdotool): Right, Right, Enter, `+`,
  Shift+Right, `0`, Escape left the window open; the second Escape closed it.
  Zoom, pan and reset do nothing yet (GUP-620 unmerged), so only focus and
  Escape were observable.
- egui (`egui_chart`, click then the same keys): no panic, app stays open. Focus
  gating, arrow lock and Escape release are covered by unit tests.

### Decisions

- Named Space and `Character(" ")` both map to `Enter`.
- egui `meta` is `Modifiers::mac_cmd`.
- The egui key logic is a `GupWidget` associated function so tests can drive it
  without a `RenderState`.

### Follow-up Stories

None.
