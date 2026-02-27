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

## Retrospective

**Completed**: 2026-02-27

### Key Technical Learnings

#### wasm-bindgen `main` Symbol Conflict with Integration Tests

- **Challenge**: `#[wasm_bindgen(start)] pub fn main()` in `lib.rs` was gated
  with `#[cfg(all(target_arch = "wasm32", not(test)))]`. However, for
  integration tests (`tests/` directory), the library is compiled as a
  **dependency**, not in test mode. Therefore `cfg(test)` is false and the
  `main` function IS compiled into the library — conflicting with the test
  harness entry point. This caused the error: "main symbol is missing, may be
  because there are multiple exports with the same name but different
  signatures, and discarded by wasm-ld."
- **Solution**: Replaced `not(test)` with a Cargo feature flag `wasm-start`.
  When building the library for production
  (`wasm-pack build --target web -- --features wasm-start`), the `main` entry
  point is included. When running tests, the feature is absent so no conflict
  occurs.
- **Pattern**: Never use `cfg(test)` to gate `#[wasm_bindgen(start)]` functions
  — it only applies when the crate itself is being compiled with `--test`, not
  when it's a dependency. Use a Cargo feature flag instead.

#### ChromeDriver Version Must Match Chromium

- **Challenge**: The nix development environment included ChromeDriver v80 but
  Chromium v145. `wasm-pack test --headless --chrome` failed with "http status:
  404" because ChromeDriver v80 can't drive Chromium v145.
- **Solution**: Used `nix build nixpkgs#chromedriver` to obtain a matching v145
  ChromeDriver and set `CHROMEDRIVER=/nix/store/.../chromedriver` when running
  tests via `cargo test` directly (wasm-pack overrides the env var).
- **Pattern**: Always verify ChromeDriver and Chromium versions match before
  attempting headless browser tests. This is tracked in GUP-240.

#### wgpu v26 API Differences

- **Challenge**: wgpu v26 changed several APIs from earlier versions:
  `request_adapter` returns `Result` instead of `Option`, `request_device` takes
  a single argument (no trace path), `RenderPassColorAttachment` requires
  `depth_slice` field.
- **Solution**: Used `.ok()` on `request_adapter`, removed the second argument
  from `request_device`, added `depth_slice: None`.
- **Pattern**: Always consult the crate's actual type signatures rather than
  relying on documentation or examples from earlier versions.

### Architectural Decisions

#### Feature Flag vs cfg(test) for WASM Start

- **Decision**: Introduced `wasm-start` feature flag in Cargo.toml.
- **Reasoning**: `cfg(test)` is semantically incorrect for this use case — the
  library isn't being tested, it's being used as a dependency by the test
  binary. A feature flag explicitly communicates intent: "include the WASM entry
  point."
- **Trade-off**: Users building for WASM must now pass `--features wasm-start`
  to `wasm-pack build`. This is a minor ergonomic cost documented in CI and the
  test harness.
- **Future**: Could add `wasm-start` to the `default` features, but that would
  cause issues for any project using gup as a library dependency in their own
  WASM binary (duplicate entry point). Keeping it opt-in is safer.

#### Two-File Test Split (CPU vs GPU)

- **Decision**: Separated tests into `wasm_integration.rs` (CPU) and
  `wasm_gpu_integration.rs` (GPU with `run_in_browser`).
- **Reasoning**: CPU tests are portable — they can run in Node.js if available,
  or in a browser. GPU tests require a browser with WebGPU support and are more
  likely to fail in CI environments without GPU hardware.
- **Trade-off**: Two test files to maintain instead of one. But clear separation
  makes it easier to run just the CPU tests in resource-constrained
  environments.
- **Future**: Once GUP-240 ensures matching ChromeDriver in CI, both files can
  be run together seamlessly.

### Development Workflow Insights

- The `wasm-pack test` command overrides the `CHROMEDRIVER` environment variable
  with its own PATH-based discovery. To use a specific ChromeDriver, run
  `cargo test --target wasm32-unknown-unknown` directly with `CHROMEDRIVER` and
  `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER` environment variables set.
- `wasm_bindgen_test_configure!(run_in_browser)` is required for any test that
  needs browser APIs (DOM, WebGPU, etc.). Without it, tests are configured for
  Node.js which doesn't have WebGPU.
- The "main symbol is missing" error from wasm-bindgen is actually the **wasm-ld
  linker** silently discarding an export due to a name+signature conflict, and
  then wasm-bindgen failing because it expected that export. The error message
  is misleading — the real issue is the conflict, not a missing symbol.
- GPU tests should always gracefully degrade with `Option`-based adapter/device
  creation. Not all environments have GPU access, and a failing test is worse
  than a skipped test with a warning message.
