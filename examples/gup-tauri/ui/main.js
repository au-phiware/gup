// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Frontend entry point for the gup-tauri example.
//
// Flow:
// 1. Check for WebGPU availability.
// 2. Load the Gup WASM package.
// 3. Invoke the Tauri backend command to get scatter data.
// 4. Pass the data to the WASM `render_scatter` function.
// 5. On "Refresh Data" click, re-invoke and re-render.

const { invoke } = window.__TAURI__.core;

// --- Gup WASM module (loaded dynamically) ---
let gupWasm = null;

/**
 * Initialise the Gup WASM module.
 *
 * The WASM package is expected at `./pkg/gup.js` (produced by
 * `wasm-pack build --target web`).
 *
 * @returns {Promise<void>}
 */
async function loadGupWasm() {
  const mod = await import("./pkg/gup.js");
  await mod.default(); // wasm_bindgen init
  gupWasm = mod;
}

/**
 * Fetch scatter data from the Tauri Rust backend.
 *
 * @param {boolean} randomised - If true, request randomised data.
 * @returns {Promise<Array<{x: number, y: number}>>}
 */
async function fetchData(randomised = false) {
  const command = randomised
    ? "get_scatter_data_randomised"
    : "get_scatter_data";
  return await invoke(command);
}

/**
 * Render scatter data to the canvas using the Gup WASM API.
 *
 * @param {Array<{x: number, y: number}>} data
 */
async function renderChart(data) {
  if (!gupWasm) {
    throw new Error("WASM module not loaded");
  }
  const json = JSON.stringify(data);
  await gupWasm.render_scatter("chart-canvas", json);
}

/**
 * Update the status message.
 *
 * @param {string} msg
 */
function setStatus(msg) {
  document.getElementById("status").textContent = msg;
}

// --- Main ---

async function main() {
  // 1. Check WebGPU
  if (!navigator.gpu) {
    document.getElementById("no-webgpu").classList.remove("hidden");
    setStatus("WebGPU unavailable");
    return;
  }

  try {
    // 2. Load WASM
    setStatus("Loading WASM…");
    await loadGupWasm();

    // 3. Fetch initial data from Rust backend
    setStatus("Fetching data…");
    const data = await fetchData(false);

    // 4. Render
    setStatus("Rendering…");
    await renderChart(data);
    setStatus(`Rendered ${data.length} points`);
  } catch (err) {
    console.error("Initialisation failed:", err);
    setStatus(`Error: ${err.message || err}`);
  }

  // 5. Wire up refresh button
  document.getElementById("btn-refresh").addEventListener("click", async () => {
    try {
      setStatus("Refreshing…");
      const data = await fetchData(true);
      await renderChart(data);
      setStatus(`Rendered ${data.length} points (refreshed)`);
    } catch (err) {
      console.error("Refresh failed:", err);
      setStatus(`Error: ${err.message || err}`);
    }
  });
}

main();
