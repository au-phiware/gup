# GUP-243: Puppeteer HTML Benchmark CI Runner

## Story Overview

**Epic**: Phase 2 - Testing Infrastructure **Theme**: CI/CD **Priority**: Low
**Story Points**: 3 **Status**: 📋 Planned

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

- [ ] Puppeteer or Playwright is available in the nix devShell
- [ ] A script loads `axis_benchmarks.html?autorun` in headless Chrome
- [ ] The script waits for `window.__gupAxisResults` and writes JSON to a file
- [ ] CI workflow includes the benchmark capture step
- [ ] Captured JSON is uploaded as a CI artifact

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

- [ ] Capture script works locally with nix devShell
- [ ] CI workflow produces JSON artifact
- [ ] Script handles errors gracefully (timeout, missing WASM module)
