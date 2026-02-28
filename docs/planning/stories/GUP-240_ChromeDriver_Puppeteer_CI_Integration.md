# GUP-240: ChromeDriver/Puppeteer CI Integration for WASM Tests

## Story Overview

**Epic**: Phase 2 - Testing Infrastructure **Theme**: CI/CD **Priority**: Low
**Story Points**: 2 **Status**: ✅ Complete **Completed**: 2026-02-28

## Overview

Add matching ChromeDriver (or Puppeteer) to the nix development environment and
CI workflows so that `wasm-pack test --headless --chrome` works for running
WebAssembly benchmarks in a real browser. Currently, the nix flake includes
Chromium 145 but ChromeDriver 80, which prevents automated headless browser
testing.

## Context

GUP-226 (WebAssembly Axis Performance Validation) ported the axis benchmarks to
wasm32 but couldn't run them in headless Chrome due to the ChromeDriver version
mismatch. The `wasm-bindgen-test-runner` requires ChromeDriver to match the
installed Chromium version. A matching ChromeDriver would also enable automated
execution of the HTML benchmark runners in `benches/wasm/`.

## User Story

> "As a CI pipeline, I want matching ChromeDriver and Chromium versions so that
> wasm-pack tests can run in headless Chrome for automated WebAssembly
> validation."

## Acceptance Criteria

- [x] ChromeDriver version matches installed Chromium version in nix flake
- [x] `wasm-pack test --headless --chrome` runs successfully for at least one
      test file
- [x] CI workflow uses the matching ChromeDriver for WebAssembly tests
- [x] HTML benchmark runners can be executed in headless mode with result
      capture

## Technical Tasks

1. Update `flake.nix` to include matching chromedriver package
2. Verify `wasm-pack test --headless --chrome` works locally
3. Update `performance.yml` to uncomment headless Chrome testing step
4. Optionally add Puppeteer/Playwright as alternative browser automation

## Dependencies

- **GUP-226**: WebAssembly Axis Performance Validation ✅
- **GUP-237**: WASM Integration Test Suite (for fixing broken wasm32 test code)

## Testing Strategy

- `wasm-pack test --headless --chrome -- --test wasm_axis_performance`
- Run `benches/wasm/axis_benchmarks.html?autorun` in headless Chrome and capture
  JSON output

## Success Metrics

- ChromeDriver version matches Chromium version
- At least one wasm-pack headless test passes
- CI produces real browser timing data

## Risk Assessment

- **Nix packaging complexity**: ChromeDriver may need to be packaged or
  overlayed.
- **CI browser availability**: GitHub Actions may need additional Chrome setup.

## Definition of Done

- [x] ChromeDriver version alignment verified
- [x] At least one wasm-pack test passes in headless Chrome
- [x] CI workflow updated to run headless browser tests

## Implementation Summary

### What Was Implemented

1. **Nix flake updates** (`flake.nix`, `flake.lock`):
   - Added `chromedriver` package to devShell buildInputs
   - Updated `nixpkgs` flake input to get matching versions (Chromium
     145.0.7632.109 and ChromeDriver 145.0.7632.109)
   - Updated `rust-overlay` flake input to latest
   - Fixed deprecated `xorg.*` package references (now top-level `libx11`,
     `libxcursor`, etc.)
   - Added ChromeDriver version display to shell hook

2. **WASM test compatibility fixes**:
   - Added `#[cfg(not(target_arch = "wasm32"))]` to `plugins.rs` test module
     (uses Send+Sync bounds unavailable on wasm32)
   - Added same gate to `integration_ecosystem_tests.rs`
   - Added `wasm_bindgen_test_configure!(run_in_browser)` to
     `wasm_axis_performance.rs`
   - Fixed duplicated doc comment in `wasm_axis_performance.rs`

3. **CI workflow updates**:
   - Uncommented headless Chrome step in `performance.yml` for axis benchmarks
   - Added axis performance test step to `wasm.yml`
   - Updated GUP-240 references in comments to reflect resolution
   - Both use `continue-on-error` for graceful degradation on runners without
     GPU

### Key Files Changed

- `flake.nix` — Added chromedriver, fixed deprecated packages
- `flake.lock` — Updated nixpkgs and rust-overlay
- `src/plugins.rs` — Added wasm32 cfg gate to test module
- `tests/integration_ecosystem_tests.rs` — Added wasm32 cfg gate
- `tests/wasm_integration.rs` — Updated doc comment
- `tests/wasm_axis_performance.rs` — Added run_in_browser, fixed docs
- `.github/workflows/performance.yml` — Enabled headless Chrome step
- `.github/workflows/wasm.yml` — Added axis performance test step

### Test Results

- 6 wasm integration tests pass in headless Chrome
- 5 wasm axis performance tests pass in headless Chrome
- All 1879+ native tests continue to pass
- All examples compile
