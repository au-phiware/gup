# GUP-265: winit Application Shell

## Story Overview

**Initiative**: Ecosystem Integration  
**Status**: 🚧 In Progress  
**Created**: 2025-07-14

## Context

winit is already Gup's primary window management backend — it appears as a
direct dependency in `Cargo.toml` and is used directly by examples such as
`windowed_demo.rs`, `multi_font_demo.rs`, and `basic/02_scatter_window.rs`.
However, every one of those examples must manually implement winit's
`ApplicationHandler` trait, construct and manage a `GupContext`, wire up surface
creation on `resumed`, dispatch `WindowEvent::Resized` to
`context.resize_surface(...)`, call `window.request_redraw()` in
`about_to_wait`, handle `CloseRequested` and keyboard quit shortcuts, and
finally drive the loop with `EventLoop::run_app(...)`. This boilerplate is
repeated across more than ten examples and spans 50–100 lines in each file
before any chart-specific code is reached.

GUP-039 delivered robust surface and window management within `GupContext`,
GUP-047 added the surface event integration layer (DPI changes, focus/unfocus,
visibility), and GUP-049 optimised multi-surface performance. Together these
three stories form a solid foundation for a higher-level shell that hides
winit's lifecycle details entirely.

This story delivers `GupApp` — a thin, opinionated wrapper around winit's event
loop that reduces a standalone Gup desktop application to a handful of lines.
The shell handles the entire winit lifecycle internally, exposing only a clean
builder API and a `run()` entry point. It is intentionally minimal: it targets
single-window desktop use and does not attempt to replace the full manual API
for multi-window or embedded scenarios.

## User Story

> "As a visualization developer, I want a `GupApp` builder that accepts a
> `Chart` and runs a native desktop window without my having to touch winit
> directly, so that I can create a working application in under ten lines of
> code."

## Acceptance Criteria

### AC1: Builder API

- [ ] `GupApp::new(chart: Chart) -> GupApp` constructs the shell with sensible
      defaults (title `"Gup"`, size 800 × 600).
- [ ] `.title(impl Into<String>) -> Self` overrides the window title.
- [ ] `.size(u32, u32) -> Self` overrides the initial logical window size.
- [ ] `.resizable(bool) -> Self` controls whether the window is user-resizable
      (default: `true`).
- [ ] `.run(self) -> Result<(), GupError>` consumes the builder, creates the
      winit event loop, and blocks until the window is closed.
- [ ] All builder methods are chainable: the idiomatic one-liner
      `GupApp::new(chart).title("My Chart").size(1200, 800).run()?` compiles and
      runs.

### AC2: Lifecycle Handling

- [ ] The GPU context and surface are created automatically on winit's `resumed`
      callback; no manual async setup is required by the caller.
- [ ] `WindowEvent::Resized` and `ScaleFactorChanged` are forwarded to
      `GupContext::resize_surface` without caller intervention.
- [ ] `AboutToWait` triggers `window.request_redraw()` so the chart re-renders
      each frame.
- [ ] `WindowEvent::RedrawRequested` calls the chart's render method and
      presents the frame.
- [ ] Device loss is handled via `GupContext`'s existing recovery path
      (GUP-048); the shell retries rather than panicking.

### AC3: Built-in Keyboard Shortcuts

- [ ] `Escape` and `Q` quit the application.
- [ ] `F` or `F11` toggles fullscreen.
- [ ] `S` triggers a screenshot (PNG saved to the working directory with an
      auto-generated filename such as `gup_screenshot_001.png`).
- [ ] Shortcut behaviour can be suppressed by calling `.shortcuts(false)` on the
      builder (for applications that manage their own key handling via GUP-013
      when that story is complete).

### AC4: Cross-Platform Correctness

- [ ] Compiles and runs without warnings on macOS, Windows, and Linux (X11 and
      Wayland).
- [ ] The `hello_world` example (see AC5) passes `cargo check --examples` on all
      three platforms in CI.
- [ ] HiDPI / Retina displays are handled correctly: the surface is resized to
      the physical pixel dimensions, not the logical ones.

### AC5: Example — `hello_world.rs`

- [ ] `examples/hello_world.rs` exists and is a complete, runnable example.
- [ ] The body of `main()` is five lines or fewer (excluding `use` statements
      and error propagation).
- [ ] The example's doc comment explains `GupApp`'s role and lists the built-in
      keyboard shortcuts.
- [ ] Running `cargo run --example hello_world` opens a window displaying a
      basic chart.

## Technical Tasks

- [ ] Create `src/app.rs` (or `src/app/mod.rs`) containing the `GupApp` struct
      and its builder implementation.
- [ ] Implement `ApplicationHandler` for an internal `GupAppRunner` struct that
      holds the `GupApp` configuration and the runtime state (window, context,
      surface ID).
- [ ] In `resumed`: create the winit `Window`, call
      `GupContext::new_with_window` (or the equivalent `add_surface` path from
      GUP-039), store the surface ID.
- [ ] In `window_event` match on `Resized`, `ScaleFactorChanged`,
      `CloseRequested`, `RedrawRequested`, and `KeyboardInput`; delegate to the
      appropriate `GupContext` methods for resize and event reporting.
- [ ] In `about_to_wait`: call `window.request_redraw()`.
- [ ] Implement screenshot capture by reading back the most recently rendered
      surface texture and encoding it as PNG (reuse or extend the existing
      screenshot infrastructure if present, otherwise use the `image` crate
      behind a feature flag).
- [ ] Add `pub use app::GupApp;` to `src/lib.rs` so consumers can write
      `use gup::GupApp;`.
- [ ] Write `examples/hello_world.rs` using the new API.
- [ ] Add unit tests for the builder (verify field values, default values, and
      the `shortcuts` flag).
- [ ] Add an integration test (headless, no window) that constructs a `GupApp`
      with default settings and verifies that `.run()` would call the correct
      sequence of operations (can be done via a mock `Chart` or by testing the
      internal `GupAppRunner` state machine directly).
- [ ] Verify `cargo check --examples` passes after the new example is added.

## Dependencies

### Prerequisite Stories

- GUP-039: Context Window Integration ✅ — provides `add_surface`,
  `resize_surface`, and multi-surface management within `GupContext`.
- GUP-047: Surface Event Integration ✅ — provides the event handler trait and
  DPI / focus / visibility callbacks that the shell delegates to.
- GUP-049: Surface Performance Optimization ✅ — ensures the per-surface render
  scheduling the shell relies on is efficient.
- GUP-013: Event Handling System 📋 — when complete, the shell's
  `shortcuts(false)` mode becomes useful; keyboard events captured by the shell
  can be forwarded through the `EventManager` introduced by GUP-013.

### Enables Stories

- Any future story that builds a higher-level "chart server", Tauri integration,
  or egui/Bevy embedding will benefit from having `GupApp` as a reference
  implementation of the winit lifecycle pattern.

## Testing Strategy

- **Unit tests**: verify `GupApp` builder defaults and that each setter mutates
  only the intended field.
- **Integration tests**: construct a `GupAppRunner` in headless mode (no real
  window) and drive it through the `resumed` → `RedrawRequested` →
  `CloseRequested` state sequence, asserting that `GupContext` methods are
  called in the right order.
- **Visual validation**: run `cargo run --example hello_world` manually and
  confirm a window opens, renders the chart, responds to resize, and exits
  cleanly on `Escape`.
- **Screenshot test**: press `S` in the hello_world window and verify a PNG file
  is created in the working directory and is non-empty.
- **Cross-platform**: CI matrix already covers Linux in headless mode via
  GUP-154; `cargo check --examples` on the new example should be gated the same
  way as other windowed examples.

## Success Metrics

- [ ] `examples/hello_world.rs` compiles and runs on all supported platforms.
- [ ] The `main()` body in `hello_world.rs` is five lines or fewer.
- [ ] All existing windowed examples continue to compile unchanged (the new API
      is additive; the manual `ApplicationHandler` approach remains available).
- [ ] `GupApp` public API is documented with `rustdoc` examples that pass
      `cargo test --doc`.

## Risk Assessment

- **Low**: winit 0.30's `ApplicationHandler` API is already used in multiple
  examples, so the implementation pattern is well understood. The shell is
  straightforward adaptation of existing example code into a reusable struct.
  _Mitigation_: Copy the skeleton from `windowed_demo.rs` and refactor; risk of
  API surprise is minimal.

- **Medium**: Screenshot capture requires reading back a GPU texture to CPU
  memory, which involves an async map operation and may introduce complexity
  around synchronisation and the `image` crate dependency. _Mitigation_: Gate
  screenshot support behind a `screenshot` Cargo feature so it does not add a
  mandatory dependency. If the feature proves complex, defer AC3's screenshot
  shortcut to a follow-up story and note it explicitly.

- **Low**: The `shortcuts(false)` mode depends on GUP-013 being complete before
  it is genuinely useful. However, the flag itself can be implemented
  unconditionally — it simply suppresses the built-in handlers regardless of
  whether GUP-013's `EventManager` is present. _Mitigation_: Implement the flag
  now; document that full custom key handling depends on GUP-013.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
