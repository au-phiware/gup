#!/usr/bin/env node
// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// capture_wasm_benchmarks.js — Load axis_benchmarks.html in headless Chrome
// via Puppeteer and capture the JSON results from window.__gupAxisResults.
//
// Usage:
//   node scripts/capture_wasm_benchmarks.js [output.json]
//
// Environment variables:
//   CHROME_PATH   — Path to the Chromium/Chrome binary (default: auto-detect)
//   BENCH_PORT    — Port for the local HTTP server (default: 8098)
//   BENCH_TIMEOUT — Timeout in milliseconds (default: 60000)
//
// Prerequisites:
//   - npm install (in scripts/ directory) to get puppeteer-core
//   - wasm-pack build --target web --out-dir benches/wasm/pkg --release
//   - Chromium available in PATH or via CHROME_PATH
//
// Exit codes:
//   0 — JSON results captured successfully
//   1 — Benchmarks failed or timed out
//   2 — Setup error (missing WASM package, no Chromium, etc.)

"use strict";

const path = require("path");
const fs = require("fs");
const http = require("http");
const { execSync } = require("child_process");

const PROJECT_ROOT = path.resolve(__dirname, "..");
const BENCH_DIR = path.join(PROJECT_ROOT, "benches", "wasm");
const RESULTS_DIR = path.join(PROJECT_ROOT, "target", "bench-results");
const DEFAULT_OUTPUT = path.join(
  RESULTS_DIR,
  `wasm_axis_puppeteer_${new Date().toISOString().slice(0, 10)}.json`,
);

const OUTPUT_PATH = process.argv[2] || DEFAULT_OUTPUT;
const PORT = parseInt(process.env.BENCH_PORT || "8098", 10);
const TIMEOUT = parseInt(process.env.BENCH_TIMEOUT || "60000", 10);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Find Chromium/Chrome executable. */
function findChromePath() {
  if (process.env.CHROME_PATH) {
    return process.env.CHROME_PATH;
  }
  for (const name of ["chromium", "google-chrome-stable", "google-chrome"]) {
    try {
      const p = execSync(`which ${name}`, { encoding: "utf8" }).trim();
      if (p) return p;
    } catch {
      /* not found, try next */
    }
  }
  return null;
}

/** Start a minimal static HTTP server for benches/wasm/. */
function startServer(dir, port) {
  const MIME = {
    ".html": "text/html",
    ".js": "application/javascript",
    ".wasm": "application/wasm",
    ".json": "application/json",
    ".css": "text/css",
  };

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, `http://localhost:${port}`);
    let filePath = path.join(dir, url.pathname === "/" ? "index.html" : url.pathname);
    filePath = path.normalize(filePath);

    // Security: ensure we stay within the bench directory
    if (!filePath.startsWith(dir)) {
      res.writeHead(403);
      res.end("Forbidden");
      return;
    }

    fs.readFile(filePath, (err, data) => {
      if (err) {
        res.writeHead(404);
        res.end("Not found");
        return;
      }
      const ext = path.extname(filePath);
      res.writeHead(200, {
        "Content-Type": MIME[ext] || "application/octet-stream",
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Embedder-Policy": "require-corp",
      });
      res.end(data);
    });
  });

  return new Promise((resolve, reject) => {
    server.listen(port, "127.0.0.1", () => resolve(server));
    server.on("error", reject);
  });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  // 1. Validate WASM package exists
  const wasmPkg = path.join(BENCH_DIR, "pkg", "gup.js");
  if (!fs.existsSync(wasmPkg)) {
    console.error(
      "❌ WASM package not found at benches/wasm/pkg/gup.js\n" +
        "   Build with: wasm-pack build --target web --out-dir benches/wasm/pkg --release",
    );
    process.exit(2);
  }

  // 2. Find Chrome
  const chromePath = findChromePath();
  if (!chromePath) {
    console.error(
      "❌ Chromium/Chrome not found.\n" +
        "   Set CHROME_PATH or ensure chromium is in PATH.",
    );
    process.exit(2);
  }
  console.log(`Using Chrome: ${chromePath}`);

  // 3. Load puppeteer-core
  let puppeteer;
  try {
    puppeteer = require("puppeteer-core");
  } catch {
    console.error(
      "❌ puppeteer-core not installed.\n" +
        "   Run: cd scripts && npm install",
    );
    process.exit(2);
  }

  // 4. Start HTTP server
  const server = await startServer(BENCH_DIR, PORT);
  console.log(`HTTP server listening on http://127.0.0.1:${PORT}`);

  let browser;
  try {
    // 5. Launch headless Chrome
    browser = await puppeteer.launch({
      executablePath: chromePath,
      headless: true,
      args: [
        "--no-sandbox",
        "--disable-setuid-sandbox",
        "--disable-gpu",
        "--disable-dev-shm-usage",
      ],
    });

    const page = await browser.newPage();

    // Log console messages for debugging
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        console.error(`  [browser] ${msg.text()}`);
      }
    });

    // Log page errors
    page.on("pageerror", (err) => {
      console.error(`  [page error] ${err.message}`);
    });

    // 6. Navigate to benchmark page with ?autorun
    const url = `http://127.0.0.1:${PORT}/axis_benchmarks.html?autorun`;
    console.log(`Navigating to ${url}`);
    await page.goto(url, { waitUntil: "networkidle0", timeout: TIMEOUT });

    // 7. Wait for window.__gupAxisResults to be populated
    console.log("Waiting for benchmark results...");
    const results = await page.waitForFunction(
      () => window.__gupAxisResults,
      { timeout: TIMEOUT },
    );

    const data = await results.jsonValue();

    // 8. Validate results
    if (!data || !data.results || !Array.isArray(data.results)) {
      console.error("❌ Invalid results structure:", JSON.stringify(data).slice(0, 200));
      process.exit(1);
    }

    console.log(`✅ Captured ${data.results.length} benchmark results`);
    console.log(`   Platform: ${data.platform}`);
    console.log(`   Timestamp: ${data.timestamp}`);

    // 9. Write JSON output
    fs.mkdirSync(path.dirname(OUTPUT_PATH), { recursive: true });
    const json = JSON.stringify(data, null, 2);
    fs.writeFileSync(OUTPUT_PATH, json + "\n");
    console.log(`Results saved to ${OUTPUT_PATH}`);

    // 10. Print summary table
    const budget = 2.0;
    let violations = 0;
    console.log("");
    console.log("Benchmark".padEnd(35) + "Median (ms)".padStart(12) + "  Status");
    console.log("-".repeat(55));
    for (const r of data.results) {
      const pass = r.median_ms < budget;
      if (!pass) violations++;
      const status = pass ? "✅" : "❌";
      console.log(
        r.name.padEnd(35) +
          r.median_ms.toFixed(4).padStart(12) +
          `  ${status}`,
      );
    }
    console.log("");
    if (violations > 0) {
      console.log(`⚠️  ${violations} benchmark(s) exceed ${budget}ms budget`);
    } else {
      console.log(`✅ All ${data.results.length} benchmarks within ${budget}ms budget`);
    }
  } catch (err) {
    console.error(`❌ Error: ${err.message}`);
    process.exit(1);
  } finally {
    if (browser) await browser.close();
    server.close();
  }
}

main();
