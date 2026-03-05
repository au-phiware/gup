# GUP-285: Fix WASM Integration Test Compilation

## Story Overview

**Initiative**: Core Infrastructure **Status**: 📋 Planned **Created**:
2025-07-18

## Context

After GUP-283 fixed the `wasm-pack build` failure, `wasm-pack test --headless
--chrome` still fails because `tests/html_export_integration.rs` uses
`tokio::runtime::Runtime::new()`, which is not available on the
`wasm32-unknown-unknown` target. The tokio runtime requires OS thread
primitives that do not exist in WASM.

## User Story

> "As a Gup developer, I want `wasm-pack test` to compile and run so that
> browser-based integration tests can execute in CI."

## Acceptance Criteria

- [ ] `wasm-pack test --headless --chrome` compiles without errors.
- [ ] Integration tests that require native-only features are gated with
      `#[cfg(not(target_arch = "wasm32"))]`.
- [ ] WASM-compatible tests use `wasm_bindgen_test` where applicable.
- [ ] All native tests (`cargo test -- --test-threads=1`) continue to pass.

## Technical Tasks

- [ ] Audit all integration tests in `tests/` for `tokio::runtime::Runtime`
      usage.
- [ ] Gate native-only tests with `#[cfg(not(target_arch = "wasm32"))]`.
- [ ] Optionally add `wasm_bindgen_test`-based equivalents for key integration
      tests.
- [ ] Verify `wasm-pack test --headless --chrome` compiles cleanly.

## Dependencies

### Prerequisite Stories

- GUP-283 ✅ (WASM build fix)

### Enables Stories

- Full WASM CI pipeline
- Browser-based benchmark stories

## Testing Strategy

- `wasm-pack test --headless --chrome` compiles and runs.
- `cargo test -- --test-threads=1` on native shows no regressions.

## Risk Assessment

- **Low**: Straightforward conditional compilation or test framework change.

## Definition of Done

- [ ] `wasm-pack test --headless --chrome` compiles.
- [ ] All native tests pass.
- [ ] `mask all-fix` exits cleanly.
