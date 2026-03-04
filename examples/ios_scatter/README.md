# iOS Scatter Plot Example

A minimal iOS application that embeds a Gup scatter plot in a native UIKit view,
demonstrating the full iOS embedding pipeline: Metal surface creation, GPU-
accelerated rendering, and touch-based mark selection.

## Prerequisites

- macOS 13+ with Xcode 15+
- Rust toolchain with the iOS targets installed:

  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim
  ```

- The `cargo-lipo` or `cargo-xcode` helper (optional but recommended)

## Building the Rust Library

From the repository root, build the `gup-ios` static library for the iOS
Simulator:

```bash
cargo build -p gup-ios --target aarch64-apple-ios-sim --release
```

For a real device:

```bash
cargo build -p gup-ios --target aarch64-apple-ios --release
```

The resulting `libgup_ios.a` is at:

```text
target/aarch64-apple-ios-sim/release/libgup_ios.a   # Simulator
target/aarch64-apple-ios/release/libgup_ios.a         # Device
```

## Building the iOS App

1. Open `examples/ios_scatter/IosScatter.xcodeproj` in Xcode (or create a new
   Single View App and follow the manual steps below).

2. Add `libgup_ios.a` to **Build Phases → Link Binary With Libraries**.

3. Add the GupSwift package as a local dependency: **File → Add Package
   Dependencies → Add Local…** → select `pkg/ios/GupSwift/`.

4. In the app target's **Build Settings** set:
   - **Other Linker Flags**: `-lresolv` (required by some Rust dependencies)
   - **Library Search Paths**: path to the directory containing `libgup_ios.a`

5. Build & Run on a Simulator (iPhone 15 or newer recommended).

## Manual Integration (no .xcodeproj)

If you prefer to integrate manually into an existing project:

```swift
import GupSwift

class ScatterViewController: UIViewController {
    private var chartView: GupChartView!

    override func viewDidLoad() {
        super.viewDidLoad()
        let ctx = try! GupContext()
        chartView = GupChartView(context: ctx)
        chartView.frame = view.bounds
        chartView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(chartView)
    }
}
```

Or in SwiftUI:

```swift
import GupSwift
import SwiftUI

struct ContentView: View {
    let context = try! GupContext()

    var body: some View {
        GupChart(context: context)
            .ignoresSafeArea()
    }
}
```

## What the Example Demonstrates

- **Metal surface creation** via `CAMetalLayer`-backed `UIView`
- **CADisplayLink-paced rendering** at the display refresh rate
- **10 000 random scatter-plot points** rendered via GPU
- **Touch selection**: tap a point to highlight it (touch → hit-test →
  selection)
- **Orientation handling**: rotate the device/simulator and the chart resizes
  without re-creating the GPU context

## Simulator vs Device

The iOS Simulator uses a software Metal implementation that is slower than real
hardware. Frame rates on Simulator may be lower than on a physical iPhone 12+.
For accurate performance measurements, always test on a real device.

## Troubleshooting

| Symptom                                              | Fix                                                                                        |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Linker error: `undefined symbol _gup_context_create` | Ensure `libgup_ios.a` is linked and Library Search Paths are correct.                      |
| Black screen                                         | Check Xcode console for wgpu/Metal validation errors.                                      |
| Crash on launch                                      | Verify the Rust library was built for the correct target (sim vs device, arm64 vs x86_64). |
| Low FPS on Simulator                                 | Expected — Simulator uses software Metal. Test on device for real perf.                    |
