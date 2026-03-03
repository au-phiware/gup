# GUP-270: iOS Platform Support

## Story Overview

**Initiative**: Mobile **Status**: 📋 Planned **Created**: 2025-07-23

## Context

iOS is the dominant mobile platform for data-visualisation use cases — business
dashboards, scientific monitoring apps, and field data collection tools all run
on iPhone and iPad. wgpu supports Metal natively on iOS via `CAMetalLayer`,
making GPU-accelerated Gup charts achievable on Apple mobile hardware without a
browser intermediary.

GUP-004 established the `GupContext` and its underlying wgpu device/queue
lifecycle. GUP-039 extended that foundation with robust surface management
(resize, DPI changes, multi-window), and GUP-042 added advanced cross-platform
surface features. Together these provide the abstractions that need to be
extended — not replaced — to accommodate the iOS surface model, where a
`CAMetalLayer`-backed `UIView` (or a SwiftUI `MetalView`) owns the drawable
surface rather than a desktop window.

GUP-182 delivered touch gesture recognition and its integration with the mark
selection system, covering the logical event side. GUP-013 (still planned) will
expose a developer-facing `.on()` API driven by `InteractionEvent`. iOS touch
input arrives as `UITouch` sequences through UIKit or SwiftUI, and must be
translated into the same `InteractionEvent` stream that GUP-013 consumes, so the
two stories need a clean interface contract between them.

This story focuses on the platform surface (Metal/CAMetalLayer wiring, the
embedding shim, and orientation handling) and the touch-to-`InteractionEvent`
bridge, delivering a runnable iOS example that demonstrates a live scatter plot
embedded in a native Swift app.

## User Story

> "As a mobile application developer, I want to embed a Gup chart in my UIKit or
> SwiftUI app so that I can display GPU-accelerated data visualisations on
> iPhone and iPad with native touch interaction."

> "As a visualisation developer targeting iOS, I want touch events on a Gup
> chart to be translated to the same `InteractionEvent` type used on desktop so
> that I can write interaction logic once and have it work across platforms."

## Acceptance Criteria

### AC1: Metal Surface Creation

- [ ] `GupContext::add_surface` accepts a `CAMetalLayer` raw pointer (passed as
      a `*mut c_void` / `RawWindowHandle::AppKitIos` or `HasRawWindowHandle`
      impl) and creates a valid wgpu surface on iOS
- [ ] Surface creation succeeds on a real device and on the iOS Simulator
      (x86_64 and arm64 targets)
- [ ] Surface pixel format is negotiated to `Bgra8Unorm` or `Bgra8UnormSrgb` as
      available on the Metal device
- [ ] No wgpu validation errors or Metal API errors appear in the Xcode console
      during surface creation or frame presentation

### AC2: Swift / Obj-C Embedding Shim

- [ ] A `gup-ios` crate (or `gup` feature flag `ios-shim`) exposes a
      `#[no_mangle]` C ABI for: `gup_context_create`, `gup_context_destroy`,
      `gup_surface_attach_layer`, `gup_surface_detach`, `gup_render_frame`
- [ ] A companion Swift package (under `pkg/ios/`) wraps the C ABI in idiomatic
      Swift types: `GupContext`, `GupSurface`, `GupChartView` (a `UIView`
      subclass) and a SwiftUI `GupChart` view struct
- [ ] `GupChartView` calls `setNeedsDisplay` / `CADisplayLink` for frame pacing
      rather than spinning on a background thread
- [ ] The Swift package compiles without warnings under Xcode 15+ with strict
      concurrency enabled

### AC3: Touch Event Translation

- [ ] `UITouch` begin / moved / ended / cancelled phases are converted to
      `TouchEvent` / `TouchPhase` (already defined in GUP-233) and forwarded
      into the Gup event pipeline
- [ ] Multi-touch sequences preserve `touch_id` identity across phases, matching
      the contract expected by GUP-182's `GestureRecognizer`
- [ ] Touch position is reported in visualisation-space coordinates (accounting
      for `UIScreen.scale`, view bounds, and any active chart transform)
- [ ] The translation layer compiles on `cfg(target_os = "ios")` only and is
      gated behind the `ios-shim` feature flag so it does not affect other
      platform builds

### AC4: Orientation Change Handling

- [ ] When device orientation changes, the `GupSurface` is automatically resized
      to match the new `CAMetalLayer.drawableSize`
- [ ] The resize path reuses `GupContext::resize_surface` (from GUP-039) without
      tearing down and recreating the wgpu device
- [ ] A 180° rotation does not produce a frame with swapped width/height
- [ ] The scatter-plot example renders correctly in portrait, landscape-left,
      and landscape-right orientations

### AC5: iOS Simulator CI Target

- [ ] A new CI job (GitHub Actions or equivalent) builds the `gup-ios` crate and
      the Swift package for the `aarch64-apple-ios-sim` target
- [ ] The job boots an iOS Simulator, installs the example app, runs a headless
      smoke test that renders one frame, and asserts no GPU errors
- [ ] The job is gated behind a `[ci ios]` commit-message flag or a dedicated
      workflow trigger so it does not run on every PR by default (simulator
      startup is slow)
- [ ] CI failure produces a human-readable log identifying any Metal validation
      or wgpu errors

### AC6: Example — Embedded Scatter Plot

- [ ] `examples/ios_scatter/` contains an Xcode project (or Swift Package with
      executable target) that embeds `GupChartView` in a `UIViewController`
- [ ] The example renders a live scatter plot of 10 000 random points that
      updates at ≥ 30 fps on an iPhone 12 or newer (or equivalent Simulator
      performance tier)
- [ ] Tapping a point highlights it (using GUP-182 touch selection), confirming
      the end-to-end touch → hit-test → selection pipeline works
- [ ] A `README.md` in `examples/ios_scatter/` documents the build steps

## Technical Tasks

- [ ] Add `aarch64-apple-ios` and `aarch64-apple-ios-sim` to the Cargo workspace
      targets; confirm `cargo check --target aarch64-apple-ios` passes for the
      `gup` crate
- [ ] Audit `cfg` guards in existing surface code (`src/context/surface.rs` and
      related) for any desktop-only assumptions (e.g. `winit` imports, X11/Win32
      handles) that need `#[cfg(not(target_os = "ios"))]` guards
- [ ] Implement `RawWindowHandle` / `HasRawWindowHandle` for a newtype wrapper
      around `*mut CAMetalLayer` (using `raw-window-handle`'s `AppKitIos` /
      `UiKitIos` handle variant)
- [ ] Wire `GupContext::add_surface` to accept the iOS handle and call
      `wgpu::Instance::create_surface_unsafe` (or the safe equivalent once wgpu
      stabilises it)
- [ ] Create `src/platform/ios.rs` (feature-gated) with: -
      `attach_metal_layer(ctx, layer_ptr) -> SurfaceId` -
      `translate_uitouch(touches, view_bounds, scale_factor) -> Vec<TouchEvent>` -
      `handle_orientation_change(ctx, surface_id, new_size)`
- [ ] Write the C ABI shim (`#[no_mangle]` functions) in `gup-ios/src/lib.rs`
- [ ] Create the Swift package skeleton at `pkg/ios/GupSwift/`; define
      `GupContext.swift`, `GupSurface.swift`, `GupChartView.swift`,
      `GupChart.swift` (SwiftUI)
- [ ] Implement `CADisplayLink`-based render loop in `GupChartView`
- [ ] Add unit tests for `translate_uitouch` using synthetic touch event data
      (no device required)
- [ ] Create `examples/ios_scatter/` Xcode project and wire to the Swift package
- [ ] Write GitHub Actions workflow `.github/workflows/ios-ci.yml` using
      `macos-latest` runner with `xcrun simctl` for simulator boot
- [ ] Update `maskfile.md` with an `ios-build` task alias for
      `cargo build --target aarch64-apple-ios-sim`

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides `GupContext`, wgpu device/queue
  lifecycle, and the surface map that will be extended for iOS
- GUP-039: Context Window Integration ✅ — provides `add_surface` /
  `resize_surface` / `remove_surface` APIs this story extends rather than
  replaces
- GUP-182: Touch Selection Support ✅ — provides `GestureRecognizer` and the
  touch → selection pipeline that this story feeds with `UITouch`-translated
  events
- GUP-013: Event Handling System 📋 — defines `InteractionEvent` and the `.on()`
  dispatch contract; touch translation output must conform to this interface
  (implementation can proceed in parallel, but the public type definitions must
  be agreed before AC3 is finalised)

### Enables Stories

- Android Platform Support (future) — the platform shim pattern established here
  (C ABI + language-native wrapper + `translate_*touch`) is the template for an
  Android/Vulkan story
- Mobile Chart Builder (future) — once iOS surface and touch are solid, a
  higher-level mobile chart convenience API becomes feasible

## Testing Strategy

- **Unit tests**: `translate_uitouch` with synthetic multi-touch sequences
  covering: single tap, two-finger pinch, rapid phase transitions, cancelled
  touches
- **Integration tests**: boot the iOS Simulator in CI, install the example app,
  assert one clean frame renders (exit code 0, no Metal validation errors in
  log)
- **Visual validation**: screenshot of the scatter-plot example on Simulator
  compared against a reference image (pixel-diff tolerance ≤ 2%)
- **Performance**: on iPhone 12 / Simulator A14 tier, 10 000-point scatter plot
  must sustain ≥ 30 fps; measure with `CADisplayLink` frame timing and log to CI
  artefacts
- **Platform isolation**: `cargo test` on Linux and Windows must still pass —
  all iOS code must be behind `cfg(target_os = "ios")` or feature flags

## Success Metrics

- [ ] `cargo check --target aarch64-apple-ios` passes with zero errors for the
      `gup` and `gup-ios` crates
- [ ] iOS Simulator CI job passes end-to-end (surface creation → frame render →
      no GPU errors)
- [ ] 10 000-point scatter plot renders at ≥ 30 fps on target hardware tier
- [ ] Touch tap selects the correct mark in the scatter-plot example
- [ ] Orientation change does not drop frames or produce visual artefacts

## Risk Assessment

- **High**: `wgpu`'s iOS/Metal surface creation path is less exercised than
  desktop targets; API surface may have breaking changes between wgpu releases.
  _Mitigation_: Pin wgpu version; write a minimal wgpu-only Metal surface smoke
  test early and run it in CI before building the full shim.

- **Medium**: `raw-window-handle` crate's `UiKitIos` / `AppKitIos` handle
  variants and the correct way to pass a `CAMetalLayer` pointer have changed
  across crate versions. _Mitigation_: Audit the exact handle variant required
  by the pinned wgpu version before writing any integration code.

- **Medium**: iOS Simulator GPU behaviour differs from real hardware (especially
  for Metal feature sets and texture formats). Performance numbers measured on
  Simulator may not transfer to device. _Mitigation_: Document Simulator vs
  device caveats in the example README; add a real-device testing note to the
  Definition of Done.

- **Low**: Swift strict-concurrency rules may conflict with the C ABI boundary
  (Sendable / actor isolation). _Mitigation_: Isolate the C-interop layer behind
  `@preconcurrency` or `nonisolated` as needed; keep the Swift wrapper thin.

- **Low**: The `ios-shim` Cargo feature increases compile time for all
  developers who run `cargo build` with `--all-features`. _Mitigation_: Ensure
  the feature is not included in the default feature set and is only activated
  by explicit opt-in or the iOS CI job.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] iOS Simulator CI job passes on `main`
- [ ] `examples/ios_scatter/` builds and runs on iOS Simulator without errors
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
