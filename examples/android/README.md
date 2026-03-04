# Example: Android Gup Chart

A minimal Android application that embeds a `GupSurfaceView` to render a
GPU-accelerated live line chart with simulated streaming data.

## Prerequisites

| Tool             | Version         | Notes                                      |
| ---------------- | --------------- | ------------------------------------------ |
| Android SDK      | API 34          | `sdkmanager "platforms;android-34"`        |
| Android NDK      | r27c            | `sdkmanager "ndk;27.2.12479018"`          |
| Rust toolchain   | nightly/stable  | With `aarch64-linux-android` target        |
| `cargo-ndk`      | ≥ 3.5           | `cargo install cargo-ndk`                  |
| JDK              | 17+             | Required by AGP 8.x                        |

## Build Steps

### 1. Install Rust Android targets

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi
```

### 2. Build native libraries

```bash
# From the repository root:
cargo ndk -t arm64-v8a -t armeabi-v7a -o examples/android/app/src/main/jniLibs \
    build --release -p gup-android
```

This produces:

```text
examples/android/app/src/main/jniLibs/
├── arm64-v8a/
│   └── libgup_android.so
└── armeabi-v7a/
    └── libgup_android.so
```

### 3. Build the APK

```bash
cd examples/android
./gradlew assembleDebug
```

The APK is at `app/build/outputs/apk/debug/app-debug.apk`.

### 4. Install and run

```bash
adb install app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n au.com.phiware.gup.example/.ChartActivity
```

## Debugging

### Attach the Rust debugger

```bash
# In a separate terminal:
cargo ndk -t arm64-v8a lldb-server
# Then in Android Studio: Run → Attach Debugger to Android Process
```

### Check logcat for GPU errors

```bash
adb logcat -s "gup-android" "Vulkan" "GLES"
```

## Architecture

```text
ChartActivity (Kotlin)
  └── GupSurfaceView (SurfaceView + SurfaceHolder.Callback)
       ├── surfaceCreated  → JNI → gup_surface_created  (ANativeWindow attach)
       ├── surfaceChanged  → JNI → gup_surface_changed   (swapchain resize)
       ├── surfaceDestroyed→ JNI → gup_surface_destroyed  (GPU cleanup)
       ├── onTouchEvent    → JNI → gup_on_touch_event     (MotionEvent → TouchEvent)
       └── doFrame         → JNI → gup_render_frame       (Choreographer vsync)
```

## NDK Version

This project is tested with NDK **r27c** (`27.2.12479018`). Other versions
may work but are not guaranteed. Pin the NDK version in your
`local.properties`:

```properties
ndk.dir=/path/to/android-ndk-r27c
```
