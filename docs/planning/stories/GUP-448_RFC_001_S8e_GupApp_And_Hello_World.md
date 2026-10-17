---
implementer: sonnet
implementer-reason:
  ports gup_core::show onto Chart with a settled builder and event mapping; the
  10-line program is written out in Decisions
---

# GUP-448: RFC-001 Step S8e: `GupApp` and a Ten-Line Hello World

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-17

## Context

This is the headline of RFC-001 S8. The strategic review found that
`examples/hello_world.rs` is 265 lines of raw wgpu, and set the bar "`GupApp` /
`gup::show()` render any chart. Rewrite `hello_world` in ~10 lines" (T2).
RFC-001 §10(b) sketches
`gup::show(chart)?; // = GupApp::new(chart).title("Gup").size(800, 500).run()`.

`gup_core::show(plot: Plot)` (`crates/gup-core/src/show.rs`, GUP-396) opens one
window, handles resize, wheel zoom about the cursor, Escape and
`GUP_SCREENSHOT_PATH`, and tears GPU state down in
`ApplicationHandler::exiting`. That teardown is load-bearing on Wayland (RFC-001
"S0b findings" item 4). It takes only a `Plot`, and since GUP-444 it routes the
wheel through `Plot::handle`. This story turns it into `GupApp` on any `Chart`,
forwards every pointer event to `Chart::handle`, and replaces the old hello
world with ten lines that use GUP-447's adders and GUP-446's tooltip.

The old crate has its own `gup::app::GupApp` (`src/app.rs`), which the old hello
world uses. That path is frozen until the S14 flip and is not touched here. The
new hello world uses `gup_core` directly; the root crate already depends on it.

## Read First

- RFC-001 §10(b) (lines 687–694) and "S0b findings" items 4–5 (lines 1308–1319)
  and the S8 bullet of "Proposed adjustments to S1–S3" (lines 1339–1343).
- [GUP-444](GUP-444_RFC_001_S8a_Cached_Resolve_And_Input.md) "Decisions"
  (`Input`, `Chart::handle`, `title_text`).
- `crates/gup-core/src/show.rs` (all of it).
- `crates/gup-core/tests/window_parity.rs` lines 1–30 and 84–100.
- `tests/examples_smoke.rs` lines 76–150 (example classification) and
  `tests/visual_regression/examples_skip_list.toml` lines 1–20 and 76–78.
- `examples/hello_world.rs` lines 1–20 (what it replaces) and
  `examples/INDEX.md` line 20.

## User Story

> "As a newcomer, I want to put my data in a window with tooltips in ten lines,
> so that my first contact with Gup is a working interactive chart and not a
> page of GPU setup."

## Acceptance Criteria

### AC1: The ten-line hello world

- [ ] `examples/hello_world.rs` is replaced by the program in Decisions: the
      copyright header, one `//!` comment, and the ten lines exactly as written
      (after `cargo fmt`).
- [ ] A new always-run test `hello_world_is_at_most_ten_lines` in
      `tests/examples_smoke.rs` counts the lines of `examples/hello_world.rs`
      that are neither blank nor start with `//`, and asserts ≤ 10.
- [ ] The same ten lines are the "Hello, world" section of `gup-core`'s crate
      docs (`lib.rs`) as a `no_run` doctest, so they compile in an external
      crate. `cargo test -p gup-core --doc` passes.
- [ ] `examples/INDEX.md` describes it as "A sine-wave scatter with tooltips in
      a window, in ten lines".

### AC2: Headless golden of the same chart

- [ ] `crates/gup-core/tests/hello_world.rs` builds the same plot (lines 3–8 of
      the program, verbatim), resolves it at 640×400, sends
      `Input::PointerMoved` at row 10's `pick_anchor`, resolves again, and
      renders it. Its scene has a text run `"sin(2.0) = 0.91"`, and it matches a
      new golden `tests/golden/gup_core/hello_world.png`. The implementer reads
      the PNG: 50 dots on a sine wave, axes with tick labels, and the tooltip
      box above and right of the 11th dot.

### AC3: `GupApp` on any chart

- [ ] `GupApp` and `show` have the signatures in Decisions, exported at the
      crate root (feature `window`, not on wasm32). `show(scatter::plot())`
      callers (the `scatter_window` example, `window_parity`) compile unchanged.
- [ ] A unit test of `input_from` (Decisions) covers `CursorMoved` at scale
      factor 2 (logical position halved), `CursorLeft`, left, right and middle
      `MouseInput` press and release, a `LineDelta` and a `PixelDelta` wheel, a
      mouse button other than those (`None`), and a wheel before any
      `CursorMoved` (`None`). Build winit's `DeviceId` with
      `unsafe { DeviceId::dummy() }`.
- [ ] The window title is `GupApp::title`, else `chart.title_text()`, else
      "Gup".

### AC4: In a real window

These need a display. Run them and record the results in the Retrospective:

- [ ] This command, with `<x>,<y>` the anchor from AC2, writes a PNG. The
      implementer reads it, and it shows the same chart and tooltip as the AC2
      golden.

      ```bash
      GUP_SCREENSHOT_PATH=/tmp/hello.png GUP_SCREENSHOT_WIDTH=640 \
        GUP_SCREENSHOT_HEIGHT=400 GUP_SCREENSHOT_POINTER=<x>,<y> \
        cargo run --example hello_world
      ```

- [ ] `cargo test -p gup-core --test window_parity -- --ignored` passes.
- [ ] A new ignored test `show_twice_fails_cleanly` (`tests/app_once.rs`, one
      test per file) calls `show` twice with `GUP_SCREENSHOT_PATH` set. The
      second call returns `Err(Error::Config { .. })` naming "one event loop per
      process", and does not panic.
- [ ] `GUP_SMOKE_WINDOWED=1 GUP_SMOKE_FILTER=hello_world mask smoke-examples`
      passes (the window runs for 5 s without panicking).

### AC5: Smoke test bookkeeping

- [ ] `tests/examples_smoke.rs` classifies a source containing `gup_core::show(`
      as `Windowed`. The `hello_world` skip-list reason reads "Windowed
      (gup_core::show); its GUP_SCREENSHOT_PATH capture needs a display".
      `skip_list_matches_examples` passes.
- [ ] `cargo check --examples` and `mask all-check` pass.

## Decisions

The program (`examples/hello_world.rs`, after the two copyright lines):

```rust
//! Hello, Gup: a sine wave in a window. Hover a point, scroll to zoom, Escape to close.

use gup_core::prelude::*;

fn main() -> gup_core::Result<()> {
    let points = (0..50).map(|i| f64::from(i) / 5.0).map(|x| (x, x.sin()));
    let mut plot = Plot::new();
    plot.circles(points)
        .x(|p| p.0)
        .y(|p| p.1)
        .tooltip(|p| format!("sin({:.1}) = {:.2}", p.0, p.1));
    gup_core::show(plot)
}
```

The API, in `crates/gup-core/src/app.rs` (`show.rs` is renamed; feature
`window`, `cfg(not(target_arch = "wasm32"))`):

```rust
/// A window showing one chart until it is closed or Escape is pressed.
pub struct GupApp { /* chart: Option<Box<dyn Chart>>, title, size */ }
impl GupApp {
    pub fn new(chart: impl Chart) -> Self;
    /// Window title (default: the chart's title, else "Gup").
    pub fn title(self, title: impl Into<String>) -> Self;
    /// Inner size in logical pixels (default 800 × 500).
    pub fn size(self, width: u32, height: u32) -> Self;
    /// Run the event loop. Once per process: a second call returns `Err`.
    pub fn run(self) -> Result<()>;
}
/// `GupApp::new(chart).run()`.
pub fn show(chart: impl Chart) -> Result<()>;
```

- **Event mapping**, a pure private function
  `fn input_from(event: &WindowEvent, cursor: Option<Point>, scale: f64) -> Option<Input>`:
  - `CursorMoved` → `PointerMoved` (logical position).
  - `CursorLeft` → `PointerLeft`.
  - `MouseInput` `Left`/`Right`/`Middle` × `Pressed`/`Released` →
    `PointerDown`/`PointerUp` at the cursor, as `Primary`/`Secondary`/`Middle`.
    Other buttons, or no cursor, give `None`.
  - `MouseWheel` → `Wheel { at: cursor, lines }`, with `LineDelta` `y` as is and
    `PixelDelta` `y / 40` (as today). No cursor gives `None`.

  The app keeps `cursor` from `CursorMoved` (cleared on `CursorLeft`), calls
  `chart.handle(input)`, and calls `request_redraw` when `redraw` is set.

- **Kept from `show`**: Escape and close exit, resize, `GUP_SCREENSHOT_PATH` and
  the `with_any_thread` builder. Teardown in `exiting` takes the chart out of
  its `Option` and drops it there, with the surface and renderer first. Each run
  creates its own `Context::new_blocking()`.
- **New**: `GUP_SCREENSHOT_POINTER=x,y` (logical px; ignored without
  `GUP_SCREENSHOT_PATH`). Before the captured frame, resolve once, handle
  `PointerMoved { at: (x, y) }`, then resolve and render as usual.
- **Re-entry**: not supported (`run_app_on_demand` is not used). The error from
  building a second event loop maps to
  `Error::config("show", "winit allows one event loop per process; show or GupApp::run has already run")`.
- **No keyboard shortcuts** beyond Escape. The old `GupApp`'s Q, F, F11 and S
  are not ported.

## Escalation Triggers

- The ten lines do not compile against GUP-446 and GUP-447 as merged (an
  annotation or method name differs). Do not change the program; report the
  difference.
- rustfmt reflows the program to more than ten counted lines.
- winit needs the event loop on the main thread in a way that breaks
  `with_any_thread` or the test harness.
- The window screenshot with `GUP_SCREENSHOT_POINTER` shows no tooltip while the
  headless golden does.

## Out of Scope

- The web canvas host: GUP-449.
- Zoom limits, pan, brush and keyboard navigation: RFC-001 S9.
- Re-entrant windows and multiple windows: not planned.
- The old `gup::app::GupApp` and its examples: deleted at S14.

## Technical Tasks

- [ ] Rename `show.rs` to `app.rs`. Add `GupApp`, generalise to
      `Box<dyn Chart>`, and add `input_from` with its unit test.
- [ ] Add `GUP_SCREENSHOT_POINTER` and the second-run error mapping.
- [ ] Replace `examples/hello_world.rs`. Add the crate-docs doctest, the
      line-count test and the `INDEX.md` line.
- [ ] Add `tests/hello_world.rs` and its golden (`GUP_BLESS=1` once, then read
      it).
- [ ] Update the smoke classifier and skip-list reason.
- [ ] Run AC4 on a display.

## Dependencies

### Prerequisite Stories

- GUP-444: RFC-001 S8a cached `resolve` and `Chart::handle` 📋.
- GUP-446: RFC-001 S8c hover, click and tooltips 📋 — `.tooltip`.
- GUP-447: RFC-001 S8d mark adders and default axes 📋 — `circles` and closures.

### Enables Stories

- RFC-001 S9 (zoom/pan/brush) adds behaviours behind `handle` that `GupApp`
  already forwards.
- RFC-001 S10's builders can be shown with `show(chart)` unchanged.

## Testing Strategy

- **Unit tests**: `input_from`.
- **Integration tests**: `tests/hello_world.rs` (golden), the line count, and
  the doctest.
- **Visual validation**: the golden, plus the window screenshot with a pointer.
- **Windowed (manual, display)**: `window_parity`, `show_twice_fails_cleanly`,
  and the windowed smoke run.

## Success Metrics

- [ ] `examples/hello_world.rs` has 10 counted lines (was 265).
- [ ] The window and the headless render show the same chart and tooltip.

## Risk Assessment

- **Medium**: the ten lines depend on two sibling stories' exact API.
  _Mitigation_: their Decisions name every method used here, and a mismatch is
  an escalation trigger, not a rewrite.
- **Low**: the AC4 checks need a display, so CI does not run them. _Mitigation_:
  AC2 covers the same chart headless in CI.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
