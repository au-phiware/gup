# GUP-237: WASM Integration Test Suite

**Priority**: Low **Complexity**: Medium **Created**: 2026-02-27 **Status**:
📋 Planned

## Overview

Create a browser-based integration test suite that loads the wasm-pack output
and verifies core Gup functionality at runtime. GUP-231 ensured the library
compiles for WASM but does not validate runtime behaviour.

## Context

With GUP-231 complete, the library builds for `wasm32-unknown-unknown` and
`wasm-pack build --target web` produces a loadable package. However, there is
no automated verification that the WASM module initialises correctly, creates a
GPU context, or renders marks in the browser. A headless browser test suite
would close this gap.

## User Story

As a library maintainer, I want automated browser tests that verify the WASM
package works at runtime so that I can catch regressions beyond compilation
failures.

## Acceptance Criteria

- [ ] A minimal HTML test harness loads the wasm-pack output
- [ ] Tests verify GPU adapter/device creation succeeds
- [ ] Tests verify at least one mark type renders without errors
- [ ] Tests run in a headless browser (Chromium with WebGPU flags)
- [ ] CI integration: tests execute as part of the WASM workflow

## Technical Tasks

- [ ] Create `tests/wasm/` directory with HTML test page
- [ ] Write Rust `#[wasm_bindgen_test]` tests for GPU initialisation
- [ ] Write a basic rendering smoke test
- [ ] Add headless browser launch to CI workflow
- [ ] Document how to run WASM tests locally

## Dependencies

- **Requires**: GUP-231 (WASM Build Platform Gating) ✅

## Testing Strategy

- `wasm-pack test --headless --chrome` for automated headless browser tests
- Manual verification with `wasm-pack test --chrome` for interactive debugging

## Risk Assessment

- **Medium**: WebGPU headless browser support varies across CI environments
- **Low**: Test harness is straightforward once the WASM build works

## Definition of Done

- [ ] `wasm-pack test --headless --chrome` passes
- [ ] CI runs WASM tests on every PR
- [ ] At least 3 integration tests covering init, context, and rendering
