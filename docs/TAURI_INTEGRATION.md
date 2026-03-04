# Tauri Integration Guide

Embed GPU-accelerated Gup charts in a
[Tauri](https://v2.tauri.app/) desktop application. The Rust backend
produces data, serialises it to JSON over Tauri's IPC bridge, and a
WASM-compiled Gup chart running in the WebView consumes and renders it
with WebGPU.

## Prerequisites

| Tool | Version | Install |
| --- | --- | --- |
| Rust | ≥ 1.77 | <https://rustup.rs/> |
| Node.js | ≥ 18 | <https://nodejs.org/> |
| Tauri CLI | 2.x | `cargo install tauri-cli` |
| wasm-pack | ≥ 0.12 | `cargo install wasm-pack` |

You also need the platform-specific system libraries required by Tauri.
See the
[Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)
for details:

- **Linux**: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`,
  `librsvg2-dev`, `patchelf`
- **macOS**: Xcode command-line tools
- **Windows**: Microsoft Visual Studio C++ Build Tools, WebView2

## Quick Start (Using the Example)

The repository ships a complete working example at
`examples/gup-tauri/`. To run it:

```bash
# 1. Build the Gup WASM package (from repository root)
wasm-pack build --target web --out-dir examples/gup-tauri/ui/pkg

# 2. Move into the example directory
cd examples/gup-tauri

# 3. Install npm dependencies (only needed once)
npm install

# 4. Launch the Tauri dev server
cargo tauri dev
```

A native window should open displaying a GPU-accelerated scatter plot.
Click **Refresh Data** to request new data from the Rust backend and
re-render the chart.

## Building Your Own Tauri + Gup App

### Step 1: Create a Tauri project

```bash
cargo tauri init
```

Choose the defaults. This creates a `src-tauri/` directory with
`Cargo.toml`, `tauri.conf.json`, and a `src/main.rs` skeleton.

### Step 2: Build the Gup WASM package

From the Gup repository root:

```bash
wasm-pack build --target web --out-dir /path/to/your-app/ui/pkg
```

This produces a JavaScript module (`gup.js`) and a WASM binary
(`gup_bg.wasm`) that expose the `render_scatter` function.

### Step 3: Add a Tauri command

In your `src-tauri/src/main.rs`, expose a command that returns chart
data:

```rust
use serde::Serialize;

#[derive(Serialize)]
struct ScatterPoint {
    x: f32,
    y: f32,
}

#[tauri::command]
fn get_scatter_data() -> Vec<ScatterPoint> {
    (0..30)
        .map(|i| {
            let t = i as f32 / 29.0;
            ScatterPoint {
                x: t * 100.0,
                y: 20.0 + 60.0 * t + 15.0 * (t * 6.28).sin(),
            }
        })
        .collect()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_scatter_data])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Step 4: Wire the frontend

In your `ui/index.html`:

```html
<canvas id="chart-canvas" width="800" height="500"></canvas>
<script type="module" src="main.js"></script>
```

In your `ui/main.js`:

```js
const { invoke } = window.__TAURI__.core;
import init, { render_scatter } from "./pkg/gup.js";

async function main() {
  // Check WebGPU availability
  if (!navigator.gpu) {
    document.body.textContent = "WebGPU is not available in this WebView.";
    return;
  }

  // Initialise the WASM module
  await init();

  // Fetch data from the Rust backend
  const data = await invoke("get_scatter_data");

  // Render the scatter plot
  await render_scatter("chart-canvas", JSON.stringify(data));
}

main();
```

### Step 5: Configure Tauri

In `src-tauri/tauri.conf.json`, ensure the frontend directory points to
your `ui/` folder:

```json
{
  "build": {
    "frontendDist": "../ui"
  },
  "app": {
    "withGlobalTauri": true
  }
}
```

### Step 6: Run

```bash
cd your-app
cargo tauri dev
```

## Live Data Updates

To update the chart without a page reload, re-invoke the command and
call `render_scatter` again:

```js
async function refresh() {
  const data = await invoke("get_scatter_data");
  await render_scatter("chart-canvas", JSON.stringify(data));
}

document.getElementById("btn-refresh").addEventListener("click", refresh);
```

The `render_scatter` function caches the GPU device and render pipeline
per canvas element, so subsequent calls only rebuild the data buffers —
no canvas or GPU resource leaks occur.

## WASM API Reference

### `render_scatter(canvas_id, data_json)`

Renders a scatter plot to an HTML `<canvas>` element.

**Parameters:**

| Name | Type | Description |
| --- | --- | --- |
| `canvas_id` | `string` | DOM `id` of the target `<canvas>` element |
| `data_json` | `string` | JSON array of `{x, y}` objects |

**Returns:** `Promise<void>`

**Errors:**

- Canvas element not found
- WebGPU not available
- Malformed JSON or empty data array

## Known Limitations

### WebGPU Availability

WebGPU support in OS-native WebViews varies by platform:

| Platform | WebView Engine | WebGPU Status |
| --- | --- | --- |
| Linux | WebKitGTK | Requires ≥ 2.42; may need `WEBKIT_DISABLE_COMPOSITING_MODE=1` |
| macOS | WebKit | Supported on macOS 14+ |
| Windows | WebView2 (Chromium) | Supported with recent Edge/Chromium updates |

The example application checks `navigator.gpu` at startup and displays
a fallback banner when WebGPU is unavailable.

### Tauri Version

This guide targets **Tauri 2.x** exclusively. The IPC and capability
APIs differ significantly from Tauri 1.x.

### Chart Builder Limitation

The current WASM API renders circles using Gup's low-level mark
rendering system (GPU-instanced circles with WGSL shaders). The
high-level `ScatterPlotBuilder` chart builder API configures scales and
axes but does not yet render data marks directly. A future story will
unify the builder output with the mark rendering pipeline.

## Troubleshooting

**"Canvas element not found"** — Ensure the `<canvas>` element's `id`
matches the string passed to `render_scatter`.

**"No suitable GPU adapter"** — The WebView does not support WebGPU.
Check the platform table above and update your system WebView.

**"Failed to create surface"** — The WASM module could not create a
WebGPU surface from the canvas. This may happen on older browsers or
when hardware acceleration is disabled.

**Blank canvas with no errors** — Verify that `wasm-pack build` was run
with `--target web` and that the `pkg/` directory is served correctly by
Tauri's `frontendDist` setting.
