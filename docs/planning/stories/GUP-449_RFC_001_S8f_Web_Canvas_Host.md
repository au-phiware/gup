---
implementer: sonnet
implementer-reason:
  host design, event mapping and size rule are settled in Decisions; follows
  WindowTarget and the wasm-size/scatter browser harness
---

# GUP-449: RFC-001 Step S8f: Interactive Charts on a Web Canvas

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete 2026-10-18
**Created**: 2026-10-17

## Context

RFC-001 §9 names four hosts that translate input into `gup::Input`: `GupApp`,
`gup::show`, the egui widget and the WASM canvas. GUP-448 covers the native
window. This story adds the browser host, which is the last of RFC-001 S8.

What exists today: gup-core builds for `wasm32-unknown-unknown` on WebGPU only
(RFC-001 "Decisions (2026-10-09)"). The `wasm-size/scatter` harness renders
through awaited `ImageTarget` readbacks, and `mask wasm-browser` drives it in
headless Chromium on SwiftShader (GUP-408). The findings carry three S8 notes
for the wasm entry:

- Start from the `wasm-size/scatter` harness, the first gup-core code proven in
  a browser (S3 findings).
- A canvas render loop reports a frame's GPU error only on the next frame, so
  the entry point should await the first frame's errors (GUP-410 findings).
- The size budget: gup-core may cost at most 400,000 B gz over bare wgpu. It
  costs 287,835 B today (`PERF_BUDGETS.md`), and the reference scatter's +2%
  gate is enforced (GUP-438).

`WindowTarget` (`window.rs`) is tied to winit. Using winit's web backend would
pull a second event system into the budget, and RFC-001 lists the canvas as a
host of its own. This host is therefore a canvas surface plus DOM listeners.

## Read First

- RFC-001 §9 "Events" (lines 655–666), "S3 findings → gup-core in a browser"
  (lines 1601–1628), "GUP-410 findings → Proposed adjustments" S8 bullet (lines
  1972–1975) and "Decisions (2026-10-09) → WASM targets WebGPU only" (lines
  1704–1723).
- [GUP-444](GUP-444_RFC_001_S8a_Cached_Resolve_And_Input.md) and
  [GUP-446](GUP-446_RFC_001_S8c_Hover_Click_Tooltips.md) "Decisions".
- `crates/gup-core/src/window.rs` lines 51–150 (surface setup) and 266–324
  (`RenderTarget`).
- `crates/gup-core/src/scope.rs` lines 250–300 (`take_gpu_errors`,
  `settle_gpu_errors`).
- `crates/gup-core/Cargo.toml` (features, wasm32 dependencies).
- `crates/gup-core/wasm-size/scatter/Cargo.toml`, `src/lib.rs` lines 1–60 and
  `index.html` lines 1–60 and 160–195.
- `scripts/browser_smoke.mjs` lines 1–24 and `crates/gup-core/PERF_BUDGETS.md`
  lines 70–80 and 145–160.

## User Story

> "As a web developer, I want to hand gup-core a `<canvas>` and a chart and get
> hover tooltips, clicks and wheel zoom, so that the same chart I render
> natively is interactive in a browser."

## Acceptance Criteria

### AC1: API

- [x] With feature `web` on `wasm32`,
      `gup_core::web::{CanvasTarget,     CanvasApp, show}` exist as in
      Decisions. Native builds with `web` enabled compile and export nothing new
      (`cargo check -p gup-core --features web`).
- [x] `cargo clippy --target wasm32-unknown-unknown -p gup-core --features web`
      is clean (part of `mask all-check`).

### AC2: Browser scenario in the smoke test

`wasm-size/scatter` enables `features = ["web"]` and exports
`show_scatter(canvas)` plus test hooks (Decisions). `index.html` adds a canvas
styled `w × h` CSS pixels (the size of the other renders) and runs, after the
existing renders:

- [x] **Hover**: a `PointerEvent("pointermove")` at row 3's client position,
      then two `requestAnimationFrame`s. `hovered_row()` is 3, `tooltip_text()`
      is the expected string, and in `render_rgba()`, at least 50% of the pixels
      inside `tooltip_rect()` are within ±2 of white and at least 20 are darker
      than 128 in every channel.
- [x] **Leave**: `pointerleave` makes `hovered_row()` −1 and `tooltip_text()`
      `null`.
- [x] **Click**: `pointerdown` and `pointerup` (button 0) at row 1 make
      `clicks()` 1 and `last_clicked()` 1.
- [x] **Wheel**: a `WheelEvent` with `deltaY: -120`, `deltaMode: 0` at the plot
      centre narrows `x_domain()` to `0.85³` of its width (relative error ≤
      1e-9). `defaultPrevented` is `true`.
- [x] The result line gains `interactive=ok` (or the failed step's name), and
      `ok` requires all four. `mask wasm-browser` passes with no console error.
- [x] The `GUPPNG` image gains a seventh row: the hover step's `render_rgba()`.
      The six rows above it are unchanged. The implementer reads
      `browser_scatter.png` and sees the tooltip box beside row 3's dot.

### AC3: Errors and first frame

- [x] `CanvasApp::new` awaits the first frame's GPU errors (`settle_gpu_errors`)
      and returns them as `Err`, and returns surface errors as `Err` rather than
      panicking. In the page, `show_scatter` on a canvas whose `"2d"` context
      was taken first rejects, and the page catches the rejection and checks
      that its message mentions the canvas.
- [x] A later frame's error goes to `console.error` and is kept in
      `CanvasApp::last_error()`. After the AC2 steps the page asserts
      `last_error()` is `null` (exported as a hook).

### AC4: Size

- [x] `mask wasm-size` passes. If the scatter harness grew, re-record
      `wasm.scatter.gz_bytes` and note the growth in `PERF_BUDGETS.md`'s log.
      Decision rule: growth over 30,000 B gz is an escalation, not a re-record.

## Decisions

```rust
// crates/gup-core/src/web.rs, #[cfg(all(target_arch = "wasm32", feature = "web"))]

/// A `<canvas>`'s WebGPU surface as a `RenderTarget`.
pub struct CanvasTarget { /* canvas, surface, config, samples, msaa */ }
impl CanvasTarget {
    pub fn new(cx: &Context, canvas: web_sys::HtmlCanvasElement) -> Result<Self>;
    /// Physical size; zero is ignored.
    pub fn resize(&mut self, cx: &Context, width: u32, height: u32) -> Result<()>;
    pub fn size(&self) -> (u32, u32);
    /// `window.devicePixelRatio`.
    pub fn dpr(&self) -> f32;
}
impl RenderTarget for CanvasTarget { /* as WindowTarget, without capture */ }

/// A chart on a canvas: DOM input goes to `Chart::handle`, redraws on demand.
/// Dropping it removes its listeners.
pub struct CanvasApp { /* Rc<RefCell<State>>, listeners */ }
impl CanvasApp {
    pub async fn new(canvas: web_sys::HtmlCanvasElement, chart: impl Chart) -> Result<Self>;
    /// Run `f` on the chart, then schedule a redraw.
    pub fn with_chart<R>(&self, f: impl FnOnce(&mut dyn Chart) -> R) -> R;
    /// The last frame error, if any.
    pub fn last_error(&self) -> Option<String>;
}
/// `CanvasApp::new(canvas, chart)`.
pub async fn show(canvas: web_sys::HtmlCanvasElement, chart: impl Chart) -> Result<CanvasApp>;
```

- **Feature**: `web = ["dep:web-sys", "dep:js-sys", "dep:wasm-bindgen"]`, with
  those deps optional under
  `[target.'cfg(target_arch = "wasm32")'.dependencies]`. The web-sys features
  are `HtmlCanvasElement`, `Element`, `DomRect`, `EventTarget`, `Event`,
  `MouseEvent`, `PointerEvent`, `WheelEvent`, `AddEventListenerOptions`,
  `Window` and `console`. It is independent of `window` (no winit).
- **Surface**: `instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas))`.
  Extract `WindowTarget::new`'s format, alpha and usage choice into a private
  `fn surface_config(adapter, &surface, width, height) -> Result<(wgpu::SurfaceConfiguration, Vec<wgpu::PresentMode>)>`
  shared by both targets. That is the only change to `window.rs`.
- **Size**: logical size is the canvas's `client_width`/`client_height` (CSS
  px). If either is 0, use the `width`/`height` attributes. Physical size is
  logical × `devicePixelRatio`, rounded, and is written to
  `canvas.width/height`. A `resize` listener on `window` repeats this and
  redraws.
- **Coordinates**: `at = (clientX − rect.left, clientY − rect.top)` from
  `getBoundingClientRect()`, in logical px. This works for synthetic events too.
  `offsetX` is not used.
- **Listeners** (on the canvas unless noted):
  - `pointermove` → `PointerMoved`
  - `pointerleave` → `PointerLeft`
  - `pointerdown`/`pointerup` → `PointerDown`/`PointerUp`, with `button` 0, 1, 2
    as `Primary`, `Middle`, `Secondary` and other buttons ignored
  - `wheel` (`passive: false`, `preventDefault()`) → `Wheel` with
    `lines = −deltaY / 40` (mode 0), `−deltaY` (mode 1) or `−deltaY × 10`
    (mode 2)
  - `resize` on `window`

  Closures are `Closure<dyn FnMut(web_sys::Event)>`, stored with their target
  and type, and removed in `Drop`.

- **Frames**: a `Response { redraw: true }` or a resize schedules one
  `requestAnimationFrame`, coalesced by a `pending` flag. A frame does
  `resolve(w / dpr, h / dpr)` then `Renderer::render` into the `CanvasTarget`.
  An `Err` is sent to `console.error` and kept in `last_error`. `new` creates
  the `Context` with `Context::new().await`, draws the first frame synchronously
  and awaits `settle_gpu_errors`.
- **Harness hooks** (`wasm-size/scatter/src/lib.rs`), as
  `#[wasm_bindgen] struct InteractiveScatter`:
  - `show_scatter(canvas) -> Promise<InteractiveScatter>`: six named rows,
    explicit x/y domains, a tooltip, and a click handler that counts into an
    `Arc<AtomicU32>` and stores the row
  - `row_px(row) -> Vec<f32>` (client-relative, via the kept `ScaleRef`s)
  - `hovered_row() -> i32` (−1 for none)
  - `tooltip_text() -> Option<String>` (from the scene's `TextRole::Tooltip`
    runs, joined with `\n`)
  - `tooltip_rect() -> Vec<f32>`, `x_domain() -> Vec<f64>`, `clicks() -> u32`,
    `last_clicked() -> i32`
  - `render_rgba() -> Promise<Vec<u8>>`, the current resolved scene through an
    awaited `ImageTarget` at the logical size
  - `last_error() -> Option<String>`
- No touch, pinch or keyboard. Pointer events already cover mouse and pen, and
  touch taps arrive as pointer events.

## Escalation Triggers

- Synthetic `PointerEvent`s do not reach the listeners, or
  `requestAnimationFrame` never fires in headless Chromium. Do not swap the test
  for direct `handle` calls; report it.
- Creating a canvas surface fails on SwiftShader, or the taken-context canvas
  (AC3) makes the browser log a console error that the smoke test counts.
- `CanvasTarget` would need winit or the `window` feature.
- WASM growth over 30,000 B gz (AC4).
- An existing browser render's numbers in the result line change.

## Out of Scope

- The native window: GUP-448.
- Touch gestures, pinch zoom, pan and brush: RFC-001 S9.
- A WebGL fallback: excluded by the 2026-10-09 decision.
- DOM-based tooltips, and the egui and bevy hosts (RFC-001 S13).

## Technical Tasks

- [ ] Add the `web` feature and dependencies. Extract `surface_config` from
      `WindowTarget::new`.
- [ ] `web.rs`: `CanvasTarget`, `CanvasApp` (state, listeners, frames, `Drop`),
      and `show`.
- [ ] Harness: `InteractiveScatter` and its hooks, and the `index.html` scenario
      and result line.
- [ ] Run `mask wasm-browser` and `mask wasm-size`, and re-record if needed.
- [ ] Append "S8f findings (GUP-449)" to RFC-001: the size cost and any
      browser-only behaviour found.

## Dependencies

### Prerequisite Stories

- GUP-444: RFC-001 S8a cached `resolve` and `Chart::handle` 📋.
- GUP-446: RFC-001 S8c hover, click and tooltips 📋 (which needs GUP-445).
- GUP-408: browser smoke test in CI ✅, GUP-438: WASM size gate ✅.

GUP-447 and GUP-448 are not required. This story can run in parallel with
GUP-448.

### Enables Stories

- RFC-001 S9's gestures get a browser host to test in.
- RFC-001 S13 (egui/bevy) reuses the surface-config extraction.

## Testing Strategy

- **Unit tests**: none native; the module is wasm-only.
- **Integration tests**: the browser scenario (AC2, AC3) in `mask wasm-browser`,
  which runs on every push.
- **Visual validation**: the harness PNG
  (`$CARGO_TARGET_DIR/visual-regression/gup_core/browser_scatter.png`), read by
  the implementer, with the hovered tooltip drawn in a new bottom row from
  `render_rgba()`.
- **Size**: `mask wasm-size`.

## Success Metrics

- [x] All four interactive steps pass in the CI browser job.
- [x] Growth stays within the AC4 rule.

## Risk Assessment

- **Medium**: headless Chromium's event and animation-frame timing may be flaky.
  _Mitigation_: coordinates come from `clientX`, the test waits two frames, and
  flakiness is an escalation.
- **Low**: the shared `surface_config` extraction touches `WindowTarget`.
  _Mitigation_: it is a pure extraction, and GUP-448's `window_parity` run
  covers it.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `gup_core::web::{CanvasTarget, CanvasApp, show}` (`src/web.rs`, feature `web`,
  wasm32 only, no winit). `surface_config` is shared with `WindowTarget` from
  the new `src/surface.rs`.
- `wasm-size/scatter` gains `show_scatter` and `InteractiveScatter`;
  `index.html` runs hover, leave, click, wheel, taken-context rejection and
  `last_error`, and the result line gains `interactive=ok`. The PNG gains a
  seventh row (the hovered tooltip beside row 3's dot, read and checked).
- Evidence: `mask wasm-browser` PASS, existing numbers unchanged.
  `mask wasm-size`: scatter 398,657 B gz (+26,565 B over the recorded 372,092;
  under the 30,000 B escalation line; `PERF_BUDGETS.md` not edited, left for the
  orchestrator to re-record). `over_wgpu` 356,944 of 400,000. `mask all-fix`
  clean. `mask old-path-loc` total 28,910 (old path untouched).
- Not met as written: AC4's re-record is deferred to the orchestrator by
  instruction; the `null` checks are `== null` because wasm-bindgen yields
  `undefined`.

## Retrospective

**Completed**: 2026-10-18

### Key Technical Learnings

#### Browser events in headless Chromium

- **Challenge**: whether synthetic events and `requestAnimationFrame` behave.
- **Solution**: `dispatchEvent` of `PointerEvent`/`WheelEvent` with `clientX`
  from `getBoundingClientRect` works; two animation frames settle a redraw.
- **Pattern**: read the page state through hooks, then check pixels from an
  awaited `ImageTarget` render of the same resolved scene.

### Architectural Decisions

#### `CanvasApp::context()` and `src/surface.rs`

- **Decision**: expose the app's `Context`; put `surface_config` in its own
  module rather than `window.rs`.
- **Reasoning**: GPU columns belong to one `Context`, so the harness cannot
  render the scene with its own; `window.rs` is gated on `window`.
- **Trade-off**: a small public addition to the Decisions' API.
- **Future**: the egui and bevy hosts reuse `surface_config`.

### Development Workflow Insights

- Worktree-isolated shells refuse compound commands; use Write/Edit and single
  simple commands.
- Pre-commit runs prettier on edited Markdown: run `prettier --write` first.

### Follow-up Stories

None. (The budget re-record is the orchestrator's.)
