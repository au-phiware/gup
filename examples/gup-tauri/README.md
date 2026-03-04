# gup-tauri

A self-contained Tauri 2.x desktop application that embeds a Gup WebGPU scatter
plot inside a native WebView. The Rust backend generates data and feeds it to
the chart over Tauri's typed IPC bridge.

## Prerequisites

- [Rust](https://rustup.rs/) ≥ 1.77
- [Node.js](https://nodejs.org/) ≥ 18
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)
  (`cargo install tauri-cli`)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/) (`cargo install wasm-pack`)
- Platform-specific dependencies (see
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

## Quick Start

```bash
# 1. Build the Gup WASM package (from the repository root)
wasm-pack build --target web --out-dir examples/gup-tauri/ui/pkg

# 2. Install frontend dependencies (optional, for Tauri CLI)
cd examples/gup-tauri
npm install

# 3. Run the development server
cargo tauri dev

# 4. Or build a release binary
cargo tauri build
```

## Project Structure

```text
gup-tauri/
├── src-tauri/               # Rust backend (Tauri application)
│   ├── Cargo.toml           # Tauri + serde dependencies
│   ├── tauri.conf.json      # Tauri configuration
│   ├── capabilities/        # Tauri v2 capability permissions
│   │   └── default.json
│   └── src/
│       └── main.rs          # Tauri commands (get_scatter_data, etc.)
├── ui/                      # Frontend (loaded into the WebView)
│   ├── index.html           # Main HTML page with <canvas> element
│   ├── main.js              # JS: invokes Tauri commands, calls WASM API
│   └── styles.css           # Basic styling
├── package.json             # npm metadata (for Tauri CLI integration)
└── README.md                # This file
```

## How It Works

1. The **Rust backend** (`src-tauri/src/main.rs`) exposes a Tauri command
   `get_scatter_data` that returns a JSON array of `{x, y}` objects.
2. The **frontend** (`ui/main.js`) calls `invoke("get_scatter_data")` via
   Tauri's IPC bridge and receives the data.
3. The frontend loads the Gup WASM package (`ui/pkg/gup.js`) and calls
   `render_scatter("chart-canvas", dataJson)` which initialises WebGPU and
   renders the scatter plot to the `<canvas>` element.
4. A "Refresh Data" button re-invokes the command with different parameters and
   re-renders the chart without a page reload.

## Known Limitations

- **WebGPU availability**: WebKitGTK on Linux requires ≥ 2.42; older WebView
  backends may not support WebGPU. The app shows a fallback message when
  `navigator.gpu` is undefined.
- **Not a workspace member**: This example is self-contained and not part of the
  main Gup Cargo workspace. It has its own `Cargo.toml` that depends on `tauri`.
