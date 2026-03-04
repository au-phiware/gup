# GUP-264: Tauri Integration

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
2025-02-28

## Context

Tauri is a framework for building lightweight, cross-platform desktop
applications with a web-technology frontend (HTML/CSS/JavaScript) and a Rust
backend. Unlike Electron, Tauri uses the OS-native WebView, keeping binary sizes
small and memory overhead low. Tauri 2.x ships with a first-class IPC bridge
that allows the Rust backend to invoke JavaScript functions — and vice versa —
over a typed message channel.

Gup's WASM story is now mature: GUP-172 established the headless browser
benchmarking pipeline that confirmed WebGPU/WASM performance characteristics,
and GUP-237 delivered a browser-based integration test suite that exercises GPU
context creation and mark rendering at runtime. Together these stories prove
that `wasm-pack build --target web` produces a loadable, functional Gup package
that can render charts inside any WebGPU-capable browser environment.

Tauri's WebView is that same browser environment on the desktop. This story
closes the loop between Gup's native Rust data-processing capabilities and its
browser rendering path: a Rust backend produces or streams data, serialises it
to JSON via the Tauri IPC bridge, and a WASM-compiled Gup chart running in the
WebView consumes that data and renders it with WebGPU. The deliverable is a
`gup-tauri` example crate (and optional feature flag) together with an
integration guide that shows developers how to embed Gup charts in their own
Tauri applications.

This story is expected to surface any friction points in the public WASM API —
particularly around chart initialisation from JavaScript, data ingestion, and
canvas lifecycle management inside a WebView — and to leave those surfaces
documented and tested.

## User Story

> "As a desktop-application developer using Tauri, I want to embed a Gup WebGPU
> chart in my WebView and feed it live data from my Rust backend, so that I can
> build GPU-accelerated visualizations in a native desktop app without leaving
> the Rust ecosystem."

> "As a Gup maintainer, I want an end-to-end Tauri example that exercises the
> WASM public API from JavaScript so that regressions in the WASM surface are
> caught before release."

## Acceptance Criteria

### AC1: `gup-tauri` example crate compiles and runs

- [ ] A new `examples/gup-tauri/` directory (or `crates/gup-tauri/`) contains a
      complete, self-contained Tauri application.
- [ ] `cargo tauri build` (or `cargo tauri dev`) succeeds without errors or
      warnings on Linux and macOS.
- [ ] The application window opens and displays a Gup scatter plot rendered via
      WebGPU inside the Tauri WebView.
- [ ] No GPU validation errors or `wgpu` warnings appear in the console during
      normal operation.

### AC2: Tauri IPC bridge carries Gup data

- [ ] The Rust backend exposes at least one Tauri command (e.g.
      `get_scatter_data`) that returns a JSON-serialisable dataset.
- [ ] The JavaScript frontend calls that command via `invoke()` and passes the
      result directly to the WASM Gup chart API.
- [ ] Updating the data source on the Rust side (simulated re-invoke) causes the
      chart to re-render with the new data without a page reload.

### AC3: WASM Gup chart API is usable from JavaScript

- [ ] The wasm-pack output exposes a public JS-callable function (e.g.
      `render_scatter(canvas_id, data_json)`) that initialises a Gup
      `RenderContext`, builds a scatter plot via the `Chart` builder API
      (GUP-018), and renders it to the supplied `<canvas>` element.
- [ ] The JS API is documented with JSDoc comments generated from the Rust
      `#[wasm_bindgen]` annotations.
- [ ] Calling the function a second time with different data updates the chart
      in place (no canvas leak or GPU resource leak).

### AC4: Integration guide is delivered

- [ ] `docs/TAURI_INTEGRATION.md` covers: prerequisites (Tauri CLI, wasm-pack,
      Node.js), project setup, building the WASM package, wiring the IPC bridge,
      and running the example.
- [ ] The guide includes a minimal working code snippet for both the Rust Tauri
      command and the JavaScript `invoke()` / WASM call sequence.
- [ ] Known limitations (e.g. WebGPU availability per OS/WebView backend) are
      documented.

### AC5: Existing test suites remain green

- [ ] `cargo test -- --test-threads=1` passes without regressions.
- [ ] `wasm-pack test --headless --chrome` (from GUP-237) continues to pass.
- [ ] `cargo check --examples` succeeds.

## Technical Tasks

- [ ] Scaffold the Tauri project under `examples/gup-tauri/` using
      `cargo tauri init` (Tauri 2.x).
- [ ] Add a `wasm-pack` build step in the Tauri `beforeBuildCommand` /
      `beforeDevCommand` (or a `maskfile.md` task) to compile the Gup WASM
      package into `examples/gup-tauri/src/` or `dist/`.
- [ ] Implement the `render_scatter` entry point in `src/lib.rs` (behind
      `#[cfg(target_arch = "wasm32")]`) using `#[wasm_bindgen]`: - Accept a
      canvas element ID and a JSON string of `[{x, y}, …]` points. - Initialise
      `RenderContext` from the canvas. - Build a scatter plot using the GUP-018
      chart builder API. - Return a JS-friendly error type on failure.
- [ ] Implement the Tauri Rust backend command `get_scatter_data` that returns a
      `Vec<Point>` (serialised to JSON by Tauri's `serde_json` integration).
- [ ] Wire the frontend HTML/JS to call `invoke("get_scatter_data")` on load and
      pass the result to `render_scatter`.
- [ ] Add a "refresh data" button in the UI that re-invokes the command and
      re-renders the chart to demonstrate live update.
- [ ] Handle GPU unavailability gracefully: show a fallback message if
      `navigator.gpu` is undefined (older WebViews without WebGPU).
- [ ] Add a `mask tauri-example` task to `maskfile.md` that builds the WASM
      package and launches `cargo tauri dev`.
- [ ] Write `docs/TAURI_INTEGRATION.md` covering prerequisites, build steps, and
      known limitations.
- [ ] Verify the example on at least two platforms (Linux + macOS or Windows).

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides the `RenderContext` API that the
  WASM entry point initialises from a WebGPU canvas.
- GUP-018: Observable Plot Chart Builders ✅ — provides the `Chart` / scatter
  plot builder used to construct the visualisation from data.
- GUP-172: WebAssembly Performance Benchmarks ✅ — confirms the wasm-pack
  pipeline is functional and documents WebGPU/WASM performance characteristics.
- GUP-237: WASM Integration Test Suite ✅ — proves GPU context creation and mark
  rendering work at runtime inside a browser/WebView environment.

### Enables Stories

- A future story could add a `gup-tauri` plugin crate that wraps the IPC pattern
  into a reusable Tauri plugin, removing boilerplate for downstream consumers.
- A future story could demonstrate streaming / event-driven data updates using
  Tauri's event system rather than explicit `invoke` calls.

## Testing Strategy

- **Unit tests**: The `render_scatter` WASM entry point should have unit tests
  (behind `#[cfg(test)]`) verifying JSON deserialisation and error paths (empty
  data, malformed JSON, unknown canvas ID).
- **Integration tests**: Extend the GUP-237 headless browser test suite with a
  test that loads the `gup-tauri` WASM package and calls `render_scatter`
  directly, verifying no GPU errors and that the canvas has non-empty content.
- **Visual validation**: Run `mask tauri-example`, observe the scatter plot in
  the application window, and confirm it matches the reference screenshot
  committed to `docs/assets/tauri-scatter-reference.png`.
- **Manual platform check**: Run `cargo tauri dev` on Linux (WebKitGTK) and one
  of macOS (WebKit) or Windows (WebView2) to confirm cross-platform behaviour.

## Success Metrics

- [ ] The Tauri example application starts and displays a Gup scatter plot in
      under 3 seconds on a development machine.
- [ ] Refreshing the data via the IPC bridge re-renders the chart in under 200
      ms (matching the GUP-172 WASM rendering baseline).
- [ ] Zero new `wgpu` validation errors introduced by the integration.
- [ ] The integration guide can be followed from scratch by a developer who has
      not previously worked on Gup (validated by a fresh-clone walkthrough).

## Risk Assessment

- **Medium**: WebGPU availability in OS-native WebViews is still uneven.
  WebKitGTK on Linux requires a recent version (≥ 2.42) and the
  `WEBKIT_DISABLE_COMPOSITING_MODE` workaround may be needed. WebView2 on
  Windows has WebGPU behind a flag in older versions. _Mitigation_: Gate the
  example on a runtime `navigator.gpu` check and display a clear "WebGPU not
  available" message; document minimum WebView versions in the integration
  guide.

- **Medium**: The public `#[wasm_bindgen]` API surface for Gup may not yet
  expose everything needed to initialise a `RenderContext` from an HTML canvas
  element. The existing WASM work (GUP-172, GUP-237) may have used internal test
  hooks rather than a stable public API. _Mitigation_: Audit the current WASM
  public API during task implementation; expand `#[wasm_bindgen]` exports as
  needed and document any new public surface in a follow-up.

- **Low**: Tauri 2.x introduces breaking changes from 1.x in the plugin and IPC
  APIs. The story targets Tauri 2.x exclusively. _Mitigation_: Pin the Tauri
  dependency in `Cargo.toml` and note the version requirement in the integration
  guide.

- **Low**: The `mask tauri-example` task requires Node.js and the Tauri CLI to
  be installed, which are not part of the standard Gup dev environment.
  _Mitigation_: Document the additional prerequisites clearly in the integration
  guide; keep the core `cargo test` suite free of any Node.js dependency.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `wasm-pack test --headless --chrome` passes.
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] `docs/TAURI_INTEGRATION.md` is committed and complete.
- [ ] Story status updated to ✅ Complete in story file and `INDEX.md`.
- [ ] Retrospective added to story document.
