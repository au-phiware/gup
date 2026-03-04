# GUP-271: Android Platform Support

## Story Overview

**Initiative**: Mobile **Status**: 🚧 In Progress **Created**: 2025-07-25

## Context

Android holds the largest share of mobile users globally, and wgpu has
first-class support for both Vulkan and OpenGL ES — the two GPU backends
available on Android devices. Making Gup available on Android opens the library
to a broad range of native mobile applications where GPU-accelerated data
visualization can provide real value: dashboards, scientific instruments, sensor
monitoring, and financial apps running on phones and tablets.

Android's surface lifecycle differs fundamentally from desktop platforms.
Rendering is bound to a `SurfaceView` (or `TextureView`), whose underlying
`Surface` object is created and destroyed asynchronously as the app moves
through foreground and background states. The render context must be torn down
and recreated in response to `surfaceCreated`, `surfaceChanged`, and
`surfaceDestroyed` callbacks. Failure to handle these transitions correctly
causes GPU resource leaks, validation errors, or crashes on resume.

GUP-004 established the core render context abstraction and GUP-039 provided the
window-integration layer that maps platform surfaces to wgpu instances. GUP-182
delivered touch gesture support at the Gup interaction level. This story wires
those existing pieces together for the Android platform: adapting the surface
lifecycle, translating Android `MotionEvent` touch input via the NDK, and
packaging the result as an Android library (`.aar`) that Kotlin and Java
applications can embed.

GUP-270 (iOS Platform Support) is being developed in parallel. Where the two
stories share patterns — Rust FFI boundary design, mobile surface lifecycle
handling, JNI/ObjC wrapper conventions — this story should adopt the same
conventions to keep the mobile platform layer coherent and maintainable.

## User Story

> "As an Android app developer, I want to embed a Gup visualization in my Kotlin
> or Java application so that I can display GPU-accelerated live charts without
> leaving the Android SDK ecosystem."

> "As a Gup library maintainer, I want the Android surface lifecycle to be
> correctly handled so that chart surfaces survive Activity pause/resume cycles
> without GPU validation errors or resource leaks."

## Acceptance Criteria

### AC1: SurfaceView Integration

- [ ] A `GupSurfaceView` (Java/Kotlin class, thin wrapper) can be placed in an
      Android layout XML and will initialize a wgpu instance on
      `surfaceCreated`.
- [ ] `surfaceChanged` (resize / format change) triggers a correct swapchain
      resize without losing chart state.
- [ ] `surfaceDestroyed` (e.g. app goes to background) tears down the wgpu
      surface and all GPU resources cleanly; no Vulkan or GLES validation errors
      appear in logcat.
- [ ] On `surfaceCreated` following a prior `surfaceDestroyed` (resume from
      background), the wgpu surface and swapchain are re-created and rendering
      resumes correctly.

### AC2: Touch Input Translation

- [ ] Android `MotionEvent` pointer events (down, move, up, cancel) are
      translated into Gup `InteractionEvent` values via the NDK/JNI bridge.
- [ ] Multi-touch (up to 5 simultaneous pointers) is correctly forwarded,
      preserving pointer IDs across `ACTION_POINTER_DOWN` / `ACTION_POINTER_UP`
      events.
- [ ] The translated events are compatible with GUP-182 touch-gesture
      recognition (tap, long-press, drag, two-finger tap).
- [ ] Touch coordinates are correctly transformed from Android display pixels to
      Gup's logical coordinate space, accounting for device pixel ratio
      (`DisplayMetrics.density`).

### AC3: JNI / NDK Bridge

- [ ] A Rust `gup-android` crate (or `gup` feature flag) exposes a stable
      `extern "C"` + `#[no_mangle]` JNI surface that the Java/Kotlin wrapper
      calls.
- [ ] The JNI interface covers: `nativeCreate`, `nativeSurfaceCreated`,
      `nativeSurfaceChanged`, `nativeSurfaceDestroyed`, `nativeOnTouchEvent`,
      `nativePause`, `nativeResume`, `nativeDestroy`.
- [ ] Panics in Rust JNI functions are caught at the boundary and converted to
      Java exceptions rather than aborting the process.
- [ ] The bridge compiles for `aarch64-linux-android` and
      `armv7-linux-androideabi` targets.

### AC4: Android Library Packaging

- [ ] `cargo ndk` (or equivalent) produces `.so` files for `arm64-v8a` and
      `armeabi-v7a` ABI directories.
- [ ] A Gradle build script assembles the `.so` files and the Java/Kotlin
      wrapper into an `.aar` archive that can be consumed by a standard Android
      project via a local Maven dependency or `implementation(files(...))`.
- [ ] The `.aar` declares a minimum SDK version of 24 (Android 7.0, the minimum
      for Vulkan) and gracefully falls back to OpenGL ES 3.0 on devices without
      Vulkan support.

### AC5: Example Application

- [ ] An `examples/android/` directory contains a minimal Android project
      (single Activity, `GupSurfaceView` in layout) that renders a live line
      chart with simulated streaming data.
- [ ] The example compiles with `./gradlew assembleDebug` and runs on an Android
      emulator (API level 30+) without errors.
- [ ] A `README.md` in `examples/android/` documents the build steps, required
      NDK version, and how to attach the debugger.

### AC6: CI Android Emulator Target

- [ ] A CI job builds the `.aar` and the example APK for `arm64-v8a`.
- [ ] The CI job runs the example on an Android emulator (API 30, x86_64 image)
      and verifies it launches and renders at least one frame without crashing.
- [ ] CI uses `cargo ndk` and the Android NDK pinned to a specific version to
      ensure reproducible builds.

## Technical Tasks

- [ ] Add `aarch64-linux-android` and `armv7-linux-androideabi` targets to the
      Rust toolchain configuration (`.cargo/config.toml` or
      `rust-toolchain.toml`).
- [ ] Create `gup-android/` crate (or add an `android` feature to `gup`) with
      JNI entry points (`nativeCreate` … `nativeDestroy`).
- [ ] Implement Rust-side Android surface lifecycle handler:
  - Wrap `ANativeWindow` pointer obtained from JNI in a wgpu-compatible surface.
  - Drive the `GupContext` (from GUP-004/GUP-039) through
    create/resize/destroy/recreate transitions.
- [ ] Implement `MotionEvent` → `InteractionEvent` translation in the JNI
      bridge, extracting pointer ID, action, `getX`/`getY`, and `getEventTime`.
- [ ] Handle `ACTION_CANCEL` by synthesising `InteractionEvent::TouchCancelled`
      for all active pointers.
- [ ] Add panic-to-Java-exception adapter using `std::panic::catch_unwind` at
      each JNI entry point.
- [ ] Write `GupSurfaceView.kt` (Kotlin) implementing `SurfaceHolder.Callback`
      and delegating all lifecycle calls to the JNI bridge.
- [ ] Write Gradle build (`build.gradle.kts`) that runs `cargo ndk` for the
      required ABIs and packages the resulting `.so` files as `jniLibs`.
- [ ] Create `examples/android/` project with one Activity, layout XML, and
      simulated data source feeding a `LineChart` mark.
- [ ] Write `examples/android/README.md` with build instructions.
- [ ] Add CI workflow step (GitHub Actions or equivalent) that:
  1. Installs Android SDK, NDK, and emulator image.
  2. Builds the `.aar`.
  3. Builds the example APK.
  4. Boots API-30 x86_64 emulator, installs APK, runs it for 5 seconds, checks
     logcat for fatal errors.
- [ ] Update `docs/README.md` to reference Android platform support.

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides the `GupContext`/`RenderContext`
  abstraction that the Android lifecycle adapter drives.
- GUP-039: Context Window Integration ✅ — provides the platform surface → wgpu
  mapping that this story extends to `ANativeWindow`.
- GUP-182: Touch Selection Support ✅ — provides the `InteractionEvent` types
  and `GestureRecognizer` that translated `MotionEvent` data feeds into.
- GUP-013: Event Handling System 📋 — the unified event dispatch system that
  Android touch events must route through; must be settled before the JNI bridge
  event path is finalised.
- GUP-270: iOS Platform Support 📋 — parallel mobile story whose FFI boundary
  and surface lifecycle patterns should be adopted here for consistency.

### Enables Stories

- A future **Mobile Chart Gallery** story would rely on both GUP-270 and GUP-271
  being complete to share cross-platform mobile examples.

## Testing Strategy

- **Unit tests**: Test `MotionEvent` → `InteractionEvent` conversion logic with
  synthetic event data; test lifecycle state machine transitions (created →
  changed → destroyed → created) in isolation using a mock `ANativeWindow`.
- **Integration tests**: In CI, install the example APK on an emulator and
  assert: (a) the app starts, (b) at least one rendered frame is produced, (c)
  logcat contains no `FATAL` or Vulkan validation errors after 5 seconds of
  idle.
- **Visual validation**: Manually verify the example on a physical device (at
  least one `arm64-v8a` device) and screenshot the live chart.
- **Lifecycle stress test**: In CI emulator, rotate the screen three times
  (forcing `surfaceDestroyed` / `surfaceCreated` cycles) and assert no GPU
  resource leaks appear in logcat.
- **Performance**: The example must sustain 60 fps on an API-30 emulator for a
  chart with ≤ 1 000 data points; measure with Android GPU Inspector or
  `systrace`.

## Success Metrics

- [ ] `.aar` library builds reproducibly in CI for `arm64-v8a` and
      `armeabi-v7a`.
- [ ] Example app runs on the Android 30 emulator for 30 seconds with zero
      `FATAL` / Vulkan validation errors in logcat.
- [ ] Screen-rotation stress test (3 rotations) completes without crashes or GPU
      resource leak warnings.
- [ ] A Kotlin developer can embed `GupSurfaceView` in a new project by
      following the `examples/android/README.md` in under 15 minutes.

## Risk Assessment

- **Medium**: Android Vulkan driver quality varies widely across device vendors.
  Some lower-end devices ship with buggy Vulkan drivers, causing rendering
  artefacts or crashes that do not reproduce on the emulator. _Mitigation_:
  Implement the OpenGL ES 3.0 fallback path (via wgpu's GLES backend) early; use
  the Android emulator's software Vulkan implementation for CI to catch
  API-level issues even without hardware.

- **Medium**: The JNI boundary between Kotlin and Rust adds complexity around
  object lifetimes and thread safety. The Android `SurfaceHolder` callbacks fire
  on the UI thread while rendering runs on a dedicated GL/Vulkan thread.
  _Mitigation_: Use a dedicated render thread with a channel-based command queue
  (as is conventional in Android OpenGL ES apps); document the threading model
  clearly in code comments and the README.

- **Low**: `cargo ndk` and Gradle integration can be fiddly to set up
  reproducibly across developer machines and CI. NDK version mismatches cause
  subtle ABI incompatibilities. _Mitigation_: Pin the NDK version in both
  `build.gradle.kts` and the CI workflow; document the exact NDK version
  required in `examples/android/README.md`.

- **Low**: GUP-013 (Event Handling System) is not yet complete. If its
  `InteractionEvent` API changes shape before this story is implemented, the JNI
  bridge event translation code will need updating. _Mitigation_: Coordinate
  with GUP-013 implementer on the settled event types before finalising the JNI
  bridge; stub the translation if needed and iterate.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Example APK builds with `./gradlew assembleDebug` from `examples/android/`
- [ ] CI Android emulator job passes (build + launch + logcat check)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
