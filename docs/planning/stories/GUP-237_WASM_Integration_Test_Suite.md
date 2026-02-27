# GUP-237: WASM Integration Test Suite

**Priority**: Low **Complexity**: Medium **Created**: 2026-02-27 **Status**: ✅
Complete

## Overview

Create a browser-based integration test suite that loads the wasm-pack output
and verifies core Gup functionality at runtime. GUP-231 ensured the library
compiles for WASM but does not validate runtime behaviour.

## Context

With GUP-231 complete, the library builds for `wasm32-unknown-unknown` and
`wasm-pack build --target web` produces a loadable package. However, there is no
automated verification that the WASM module initialises correctly, creates a GPU
context, or renders marks in the browser. A headless browser test suite would
close this gap.

## User Story

As a library maintainer, I want automated browser tests that verify the WASM
package works at runtime so that I can catch regressions beyond compilation
failures.

## Acceptance Criteria

- [x] A minimal HTML test harness loads the wasm-pack output
- [x] Tests verify GPU adapter/device creation succeeds
- [x] Tests verify at least one mark type renders without errors
- [x] Tests run in a headless browser (Chromium with WebGPU flags)
- [x] CI integration: tests execute as part of the WASM workflow

## Technical Tasks

- [x] Create `tests/wasm/` directory with HTML test page
- [x] Write Rust `#[wasm_bindgen_test]` tests for GPU initialisation
- [x] Write a basic rendering smoke test
- [x] Add headless browser launch to CI workflow
- [x] Document how to run WASM tests locally

## Dependencies

- **Requires**: GUP-231 (WASM Build Platform Gating) ✅

## Testing Strategy

- `wasm-pack test --headless --chrome` for automated headless browser tests
- Manual verification with `wasm-pack test --chrome` for interactive debugging

## Risk Assessment

- **Medium**: WebGPU headless browser support varies across CI environments
- **Low**: Test harness is straightforward once the WASM build works

## Definition of Done

- [x] `wasm-pack test --headless --chrome` passes
- [x] CI runs WASM tests on every PR
- [x] At least 3 integration tests covering init, context, and rendering

## Implementation Summary

### What Was Implemented

A three-tier WASM integration test suite verifying runtime behaviour in the
browser, plus infrastructure fixes enabling the tests to run.

### Key Changes

1. **`tests/wasm_integration.rs`** — 6 CPU-only tests: module loading, Circle
   and Rectangle vertex generation, CircleInstance bytemuck round-trip,
   MarkRegistry operations, shader source validation.

2. **`tests/wasm_gpu_integration.rs`** — 3 GPU tests using `run_in_browser`:
   WebGPU adapter request, device creation with buffer allocation, end-to-end
   Circle mark render to an off-screen 64×64 texture (storage buffer, vertex
   buffer, index buffer, bind group, pipeline, render pass).

3. **`tests/wasm/index.html`** — Standalone HTML test harness that loads the
   `wasm-pack build` output from `pkg/` and verifies module initialisation, GPU
   adapter/device creation, and a minimal clear render pass via the WebGPU
   JavaScript API.

4. **`src/lib.rs`** — Gated `#[wasm_bindgen(start)]` behind the `wasm-start`
   feature flag. The previous `cfg(not(test))` guard didn't work for integration
   tests because the library is compiled as a dependency (not in test mode), so
   the `main` symbol conflicted with the wasm-bindgen test harness entry point.

5. **`Cargo.toml`** — Added `wasm-start` feature flag.

6. **`.github/workflows/wasm.yml`** — Added WASM test compilation check and
   best-effort `wasm-pack test --headless --chrome` step. Updated
   `wasm-pack build` to pass `--features wasm-start`.

### Test Results

- **WASM CPU tests**: 6 passed (headless Chrome)
- **WASM GPU tests**: 3 passed (headless Chrome with WebGPU)
- **Native tests**: 2,500+ passed (0 failures)
- All examples compile
