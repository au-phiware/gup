# GUP-240: ChromeDriver/Puppeteer CI Integration for WASM Tests

## Story Overview

**Epic**: Phase 2 - Testing Infrastructure **Theme**: CI/CD **Priority**: Low
**Story Points**: 2 **Status**: 💡 New

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
> wasm-pack tests can run in headless Chrome for automated WebAssembly validation."

## Acceptance Criteria

- [ ] ChromeDriver version matches installed Chromium version in nix flake
- [ ] `wasm-pack test --headless --chrome` runs successfully for at least one
      test file
- [ ] CI workflow uses the matching ChromeDriver for WebAssembly tests
- [ ] HTML benchmark runners can be executed in headless mode with result capture

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

- **Nix packaging complexity**: ChromeDriver may need to be packaged or overlayed.
- **CI browser availability**: GitHub Actions may need additional Chrome setup.

## Definition of Done

- [ ] ChromeDriver version alignment verified
- [ ] At least one wasm-pack test passes in headless Chrome
- [ ] CI workflow updated to run headless browser tests
