# GUP-243: Puppeteer HTML Benchmark CI Runner

## Story Overview

**Epic**: Phase 2 - Testing Infrastructure **Theme**: CI/CD **Priority**: Low
**Story Points**: 3 **Status**: ✅ Complete **Completed**: 2026-02-28

## Overview

Add Puppeteer (or Playwright) to the development environment and CI workflow to
capture JSON output from the HTML benchmark runners in `benches/wasm/`. GUP-240
aligned ChromeDriver and Chromium versions for wasm-pack tests, but the HTML
benchmark runners (`axis_benchmarks.html?autorun`) store results in
`window.__gupAxisResults` and need a browser automation tool to extract that
data in CI.

## Context

The `benches/wasm/axis_benchmarks.html` page supports an `?autorun` URL
parameter and stores results in `window.__gupAxisResults`. Currently there is no
automation to load this page in headless Chrome, wait for completion, and
extract the JSON. Puppeteer or Playwright can bridge this gap, enabling real
browser timing data collection in CI.

## User Story

> "As a CI pipeline, I want to automatically run the HTML benchmark page in
> headless Chrome and capture the JSON results so that performance regressions
> in the WebAssembly path are detected."

## Acceptance Criteria

- [x] Puppeteer or Playwright is available in the nix devShell
- [x] A script loads `axis_benchmarks.html?autorun` in headless Chrome
- [x] The script waits for `window.__gupAxisResults` and writes JSON to a file
- [x] CI workflow includes the benchmark capture step
- [x] Captured JSON is uploaded as a CI artifact

## Technical Tasks

1. Add `puppeteer` (or `playwright`) to the nix flake
2. Create a `scripts/capture_wasm_benchmarks.js` helper
3. Integrate into `performance.yml` after the wasm-pack build step
4. Upload captured JSON as a GitHub Actions artifact

## Dependencies

- **GUP-240**: ChromeDriver/Puppeteer CI Integration ✅

## Testing Strategy

- Run the capture script locally and verify JSON output matches
  `window.__gupAxisResults` structure
- Verify CI artifact contains valid JSON with all 8 benchmark results

## Success Metrics

- CI produces a JSON artifact with real browser timing data
- Script completes within 60 seconds

## Risk Assessment

- **Nix packaging**: Puppeteer bundles its own Chromium by default; may need
  configuration to use the system Chromium instead.
- **CI headless rendering**: GitHub Actions runners lack GPU; WebGPU benchmarks
  may fail or report degraded times.

## Definition of Done

- [x] Capture script works locally with nix devShell
- [x] CI workflow produces JSON artifact
- [x] Script handles errors gracefully (timeout, missing WASM module)

## Implementation Summary

### What Was Implemented

1. **Nix flake updates** (`flake.nix`):
   - Added `nodejs` to devShell buildInputs for running Puppeteer scripts
   - Added Node.js version display to shell hook for verification

2. **Puppeteer capture script** (`scripts/capture_wasm_benchmarks.js`):
   - Uses `puppeteer-core` (no bundled Chromium; uses system Chromium from nix)
   - Starts a built-in HTTP server for the `benches/wasm/` directory
   - Navigates to `axis_benchmarks.html?autorun` in headless Chrome
   - Waits for `window.__gupAxisResults` with configurable timeout
   - Validates result structure and writes pretty-printed JSON to output file
   - Prints summary table with budget comparison
   - Handles errors gracefully: missing WASM package (exit 2), no Chrome
     (exit 2), timeout (exit 1), invalid results (exit 1)
   - Supports `CHROME_PATH`, `BENCH_PORT`, and `BENCH_TIMEOUT` env vars

3. **Node.js dependency management** (`scripts/package.json`,
   `scripts/package-lock.json`):
   - Added `puppeteer-core` v24.x as project dependency
   - `scripts/node_modules/` added to `.gitignore`

4. **CI workflow updates** (`.github/workflows/performance.yml`):
   - Added "Install Puppeteer dependencies" step (`npm ci` in scripts/)
   - Added "Capture HTML benchmark results via Puppeteer" step with
     `continue-on-error: true`
   - Updated artifact upload to include
     `target/bench-results/wasm_axis_benchmarks.json`

### Key Files Changed

- `flake.nix` — Added nodejs, shell hook version display
- `scripts/capture_wasm_benchmarks.js` — New: Puppeteer benchmark capture script
- `scripts/package.json` — New: puppeteer-core dependency
- `scripts/package-lock.json` — New: lockfile for reproducible installs
- `.github/workflows/performance.yml` — Added Puppeteer capture steps
- `.gitignore` — Added scripts/node_modules/

### Test Results

- Capture script tested locally: captures all 8 benchmark results
- JSON output validated: correct structure with all fields
- Script completes in ~15 seconds locally
- All 1900+ Rust tests continue to pass
- All examples compile
- `mask all-fix` passes cleanly
