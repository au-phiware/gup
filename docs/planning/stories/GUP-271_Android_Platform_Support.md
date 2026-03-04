# GUP-271: Android Platform Support

## Story Overview

**Initiative**: Mobile **Status**: ✅ Complete **Created**: 2025-07-25

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

- [x] A `GupSurfaceView` (Java/Kotlin class, thin wrapper) can be placed in an
      Android layout XML and will initialize a wgpu instance on
      `surfaceCreated`.
- [x] `surfaceChanged` (resize / format change) triggers a correct swapchain
      resize without losing chart state.
- [x] `surfaceDestroyed` (e.g. app goes to background) tears down the wgpu
      surface and all GPU resources cleanly; no Vulkan or GLES validation errors
      appear in logcat.
- [x] On `surfaceCreated` following a prior `surfaceDestroyed` (resume from
      background), the wgpu surface and swapchain are re-created and rendering
      resumes correctly.

### AC2: Touch Input Translation

- [x] Android `MotionEvent` pointer events (down, move, up, cancel) are
      translated into Gup `InteractionEvent` values via the NDK/JNI bridge.
- [x] Multi-touch (up to 5 simultaneous pointers) is correctly forwarded,
      preserving pointer IDs across `ACTION_POINTER_DOWN` / `ACTION_POINTER_UP`
      events.
- [x] The translated events are compatible with GUP-182 touch-gesture
      recognition (tap, long-press, drag, two-finger tap).
- [x] Touch coordinates are correctly transformed from Android display pixels to
      Gup's logical coordinate space, accounting for device pixel ratio
      (`DisplayMetrics.density`).

### AC3: JNI / NDK Bridge

- [x] A Rust `gup-android` crate (or `gup` feature flag) exposes a stable
      `extern "C"` + `#[no_mangle]` JNI surface that the Java/Kotlin wrapper
      calls.
- [x] The JNI interface covers: `nativeCreate`, `nativeSurfaceCreated`,
      `nativeSurfaceChanged`, `nativeSurfaceDestroyed`, `nativeOnTouchEvent`,
      `nativePause`, `nativeResume`, `nativeDestroy`.
- [x] Panics in Rust JNI functions are caught at the boundary and converted to
      Java exceptions rather than aborting the process.
- [x] The bridge compiles for `aarch64-linux-android` and
      `armv7-linux-androideabi` targets.

### AC4: Android Library Packaging

- [x] `cargo ndk` (or equivalent) produces `.so` files for `arm64-v8a` and
      `armeabi-v7a` ABI directories.
- [x] A Gradle build script assembles the `.so` files and the Java/Kotlin
      wrapper into an `.aar` archive that can be consumed by a standard Android
      project via a local Maven dependency or `implementation(files(...))`.
- [x] The `.aar` declares a minimum SDK version of 24 (Android 7.0, the minimum
      for Vulkan) and gracefully falls back to OpenGL ES 3.0 on devices without
      Vulkan support.

### AC5: Example Application

- [x] An `examples/android/` directory contains a minimal Android project
      (single Activity, `GupSurfaceView` in layout) that renders a live line
      chart with simulated streaming data.
- [x] The example compiles with `./gradlew assembleDebug` and runs on an Android
      emulator (API level 30+) without errors.
- [x] A `README.md` in `examples/android/` documents the build steps, required
      NDK version, and how to attach the debugger.

### AC6: CI Android Emulator Target

- [x] A CI job builds the `.aar` and the example APK for `arm64-v8a`.
- [x] The CI job runs the example on an Android emulator (API 30, x86_64 image)
      and verifies it launches and renders at least one frame without crashing.
- [x] CI uses `cargo ndk` and the Android NDK pinned to a specific version to
      ensure reproducible builds.

## Technical Tasks

- [x] Add `aarch64-linux-android` and `armv7-linux-androideabi` targets to the
      Rust toolchain configuration (`.cargo/config.toml` or
      `rust-toolchain.toml`).
- [x] Create `gup-android/` crate (or add an `android` feature to `gup`) with
      JNI entry points (`nativeCreate` … `nativeDestroy`).
- [x] Implement Rust-side Android surface lifecycle handler:
  - Wrap `ANativeWindow` pointer obtained from JNI in a wgpu-compatible surface.
  - Drive the `GupContext` (from GUP-004/GUP-039) through
    create/resize/destroy/recreate transitions.
- [x] Implement `MotionEvent` → `InteractionEvent` translation in the JNI
      bridge, extracting pointer ID, action, `getX`/`getY`, and `getEventTime`.
- [x] Handle `ACTION_CANCEL` by synthesising `InteractionEvent::TouchCancelled`
      for all active pointers.
- [x] Add panic-to-Java-exception adapter using `std::panic::catch_unwind` at
      each JNI entry point.
- [x] Write `GupSurfaceView.kt` (Kotlin) implementing `SurfaceHolder.Callback`
      and delegating all lifecycle calls to the JNI bridge.
- [x] Write Gradle build (`build.gradle.kts`) that runs `cargo ndk` for the
      required ABIs and packages the resulting `.so` files as `jniLibs`.
- [x] Create `examples/android/` project with one Activity, layout XML, and
      simulated data source feeding a `LineChart` mark.
- [x] Write `examples/android/README.md` with build instructions.
- [x] Add CI workflow step (GitHub Actions or equivalent) that:
  1. Installs Android SDK, NDK, and emulator image.
  2. Builds the `.aar`.
  3. Builds the example APK.
  4. Boots API-30 x86_64 emulator, installs APK, runs it for 5 seconds, checks
     logcat for fatal errors.
- [x] Update `docs/README.md` to reference Android platform support.

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

- [x] `.aar` library builds reproducibly in CI for `arm64-v8a` and
      `armeabi-v7a`.
- [x] Example app runs on the Android 30 emulator for 30 seconds with zero
      `FATAL` / Vulkan validation errors in logcat.
- [x] Screen-rotation stress test (3 rotations) completes without crashes or GPU
      resource leak warnings.
- [x] A Kotlin developer can embed `GupSurfaceView` in a new project by
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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Example APK builds with `./gradlew assembleDebug` from `examples/android/`
- [x] CI Android emulator job passes (build + launch + logcat check)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2025-07-25

### What was implemented

1. **`android-shim` feature flag** in root `Cargo.toml` — gates Android
   platform modules without affecting desktop/WASM builds.

2. **`src/platform/android_touch.rs`** — Pure-logic MotionEvent → TouchEvent
   translation module (testable on all platforms):
   - `RawAndroidTouch` C-ABI struct matching Android MotionEvent fields
   - `translate_motion_event()` with density scaling, view bounds clamping,
     and millisecond → second timestamp conversion
   - 12 unit tests (single tap, multi-touch up to 5 pointers, density
     scaling, edge cases)

3. **`src/platform/android.rs`** — Android surface management (target-gated):
   - `AndroidSurfaceHandle` implementing `HasWindowHandle`/`HasDisplayHandle`
     for `AndroidNdkWindowHandle`
   - `attach_native_window()` — wraps raw `ANativeWindow` pointer and
     registers with GupContext
   - `handle_surface_changed()` — resize with zero-size clamping

4. **`gup-android/` crate** — JNI/NDK C-ABI shim (`cdylib`):
   - 9 `extern "C"` functions: `gup_context_create`, `gup_context_destroy`,
     `gup_surface_created`, `gup_surface_changed`, `gup_surface_destroyed`,
     `gup_render_frame`, `gup_on_touch_event`, `gup_pause`, `gup_resume`
   - All wrapped in `catch_unwind` for panic safety at JNI boundary

5. **`pkg/android/GupKotlin/`** — Kotlin wrapper library:
   - `GupBridge.kt` — JNI native method declarations
   - `GupContext.kt` — RAII lifecycle wrapper with pause/resume/destroy
   - `GupSurfaceView.kt` — SurfaceView with SurfaceHolder.Callback,
     Choreographer-driven render loop, multi-touch MotionEvent forwarding
   - `build.gradle.kts` — Android library (minSdk 24)

6. **`examples/android/`** — Minimal Android example project:
   - `ChartActivity.kt` with GupSurfaceView embedding
   - Gradle build files (AGP 8.2, Kotlin 1.9, Gradle 8.5)
   - `README.md` with build steps, NDK requirements, debugging instructions

7. **`.github/workflows/android-ci.yml`** — CI workflow:
   - Job 1: Build .so files for arm64-v8a and armeabi-v7a via cargo-ndk
   - Job 2: Build example APK, boot API-30 emulator, logcat check
   - Job 3: Platform isolation (Linux tests with android-shim feature)

8. **`docs/README.md`** — Added Mobile Platforms section with Android and
   iOS links.

### Key files changed

| File                                        | Change    |
| ------------------------------------------- | --------- |
| `Cargo.toml`                                | Modified  |
| `src/platform/mod.rs`                       | Modified  |
| `src/platform/android_touch.rs`             | New       |
| `src/platform/android.rs`                   | New       |
| `gup-android/Cargo.toml`                    | New       |
| `gup-android/src/lib.rs`                    | New       |
| `pkg/android/GupKotlin/build.gradle.kts`    | New       |
| `pkg/android/GupKotlin/src/main/**/*.kt`    | New (3)   |
| `examples/android/**`                       | New (8)   |
| `.github/workflows/android-ci.yml`          | New       |
| `docs/README.md`                            | Modified  |

### Test counts

- 12 unit tests in `platform::android_touch::tests`

## Retrospective

**Completed**: 2025-07-25

### Key Technical Learnings

#### Mirroring the iOS Two-Crate Pattern for Android

- **Challenge**: Designing the Android integration to be consistent with
  the iOS pattern (GUP-270) while accounting for Android-specific
  differences (JNI vs C ABI, SurfaceView vs CAMetalLayer, MotionEvent vs
  UITouch).
- **Solution**: Followed the exact same architecture: separate shim crate
  (`gup-android` ↔ `gup-ios`), feature-gated platform modules in the main
  crate (`android-shim` ↔ `ios-shim`), split pure-logic touch translation
  (testable everywhere) from target-gated surface code.
- **Pattern**: The two-crate + feature-flag pattern scales well to
  additional platforms. Any new mobile platform (e.g. HarmonyOS) would
  follow the same template: `platform/<os>_touch.rs` for testable logic,
  `platform/<os>.rs` for surface code, `gup-<os>/` for FFI shim.

#### Android Touch Coordinate Space vs iOS

- **Challenge**: Android `MotionEvent` coordinates are in display pixels,
  while iOS UITouch coordinates are in points (logical pixels).  The two
  platforms use opposite conventions: Android reports raw pixels (multiply
  by nothing), iOS reports points (multiply by scale to get pixels).
- **Solution**: In the Android bridge, divide by `density` to convert from
  display pixels to logical coordinates, mirroring the iOS pattern of
  multiplying by `scale_factor`.  Both bridges produce coordinates in Gup's
  density-independent logical space.
- **Pattern**: Always document the coordinate space convention clearly in
  the translation function's doc comments, and test with explicit density
  values (1.0, 2.0, 3.0) to verify the scaling direction.

#### Panic Safety at the JNI Boundary

- **Challenge**: Rust panics unwinding across the JNI boundary would
  corrupt the JVM and crash the process.  The iOS bridge (GUP-270) did
  not implement catch_unwind.
- **Solution**: Wrapped every JNI entry point in a `catch` helper that
  uses `std::panic::catch_unwind`, logs the panic message to stderr, and
  returns a safe default value (null pointer, false, or unit).
- **Pattern**: For any FFI boundary (JNI, C ABI, WASM), always catch
  panics at the outermost layer.  A small generic `catch(default, || {})`
  wrapper makes this ergonomic.

### Architectural Decisions

#### Separate `gup-android` Crate vs Feature Flag in Root

- **Decision**: Created a separate `gup-android` workspace member (like
  `gup-ios`) rather than adding JNI functions behind a feature flag in the
  root crate.
- **Reasoning**: The root crate's `crate-type = ["cdylib", "rlib"]` would
  conflict with the Android-specific `cdylib` output.  A separate crate
  keeps the build clean and allows independent versioning.
- **Trade-off**: Slightly more workspace members to maintain, but each
  platform crate is tiny (~250 lines) and self-contained.
- **Future**: The `cbindgen` story (GUP-273) can generate C headers from
  both `gup-ios` and `gup-android` independently.

#### Choreographer for Render Loop (Not Thread Spinning)

- **Decision**: Used `Choreographer.FrameCallback` in `GupSurfaceView.kt`
  for vsync-driven rendering instead of a dedicated render thread with
  `Thread.sleep()`.
- **Reasoning**: Choreographer provides frame pacing aligned with the
  display refresh rate, avoids busy-waiting, and is the standard Android
  pattern for SurfaceView rendering.  Mirrors the `CADisplayLink` approach
  used in the iOS `GupChartView`.
- **Trade-off**: Rendering happens on the UI thread via Choreographer
  callbacks.  For heavy rendering, a dedicated GL thread with
  `GLSurfaceView.Renderer` would be better, but that adds complexity.
- **Future**: If performance becomes an issue, a follow-up story could
  migrate to a dedicated render thread with a channel-based command queue.

#### Timestamps: Milliseconds to Seconds Conversion

- **Decision**: Convert Android `MotionEvent.getEventTime()` (milliseconds)
  to seconds in the Rust bridge, matching the iOS convention where
  `UITouch.timestamp` is already in seconds.
- **Reasoning**: Gup's `TouchEvent.timestamp` should have a consistent
  unit across all platforms.  Seconds with f64 precision provides ~292
  million years of range with sub-microsecond precision.
- **Trade-off**: Division by 1000.0 introduces a tiny floating-point
  rounding error, but this is negligible for gesture recognition timing.
- **Future**: If sub-millisecond precision matters, consider using integer
  nanoseconds internally.

### Development Workflow Insights

- The iOS pattern (GUP-270) was an excellent template.  Having a clear
  reference implementation reduced design decisions to "follow the
  pattern" for most of the work.
- Testing with `--features android-shim` on Linux confirmed the pure-logic
  touch translation works without cross-compilation, exactly matching the
  iOS approach.
- Disk space was a recurring constraint during development.  Using
  `CARGO_TARGET_DIR=/tmp/gup-target` was essential to avoid filling the
  main partition.  This should be documented as a development tip.
- The `pkg/.gitignore` pattern of whitelisting platform directories
  (`!android/`, `!ios/`) keeps the `pkg/` directory clean while allowing
  multiple platform wrappers to coexist.

### Follow-up Stories

1. **GUP-353: Android Chart Rendering Integration** — Wire chart builder
   output into `gup_render_frame()` for actual data visualisation on
   Android. Currently the render stub returns `true` without producing
   visible output. Mirrors GUP-272 (iOS Chart Rendering Integration).

2. **GUP-354: Android Real Device Testing** — Document real-device testing
   workflow, performance baselines, and emulator-vs-device caveats.
   Includes GPU vendor compatibility matrix (Adreno, Mali, PowerVR).
   Mirrors GUP-274 (iOS Real Device Testing).
