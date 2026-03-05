# GUP-269C: ES Module WASM Loading Strategy

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-07-19

## Context

GUP-269B introduced `WasmStrategy::Auto` and `render_from_bundle` for the
HTML export pipeline, but the JavaScript bootstrap still uses raw
`WebAssembly.instantiate()` for both inline and URL strategies. This
works for basic WASM modules but cannot call wasm-bindgen exported
functions like `render_from_bundle` directly — wasm-bindgen modules
require their generated JS glue code for proper function invocation.

Adding an ES module loading strategy that uses `import()` to load the
wasm-bindgen JS wrapper would enable direct JS→WASM function calls,
completing the interactive rendering pipeline end-to-end in the browser.

## User Story

> "As a visualization developer, I want the HTML export to load the WASM
> module via ES module import so that the exported page can call
> `render_from_bundle` directly and render the chart interactively."

## Acceptance Criteria

- [ ] A new `WasmStrategy::Module(String)` variant (or similar) is added
      that generates a JavaScript `import()` call for the wasm-bindgen
      JS wrapper URL.
- [ ] The generated JS calls `init()` and then `render_from_bundle()`
      with the embedded chart data.
- [ ] The exported HTML renders interactively when the JS wrapper and
      WASM file are served alongside it.
- [ ] Integration tests verify the generated JS contains the correct
      `import()` call and function invocations.
- [ ] Documentation covers when to use Module vs URL vs Inline strategies.

## Technical Tasks

- [ ] Add `WasmStrategy::Module(String)` variant to the enum.
- [ ] Implement `module_wasm_script()` template function.
- [ ] Add `<script type="module">` handling in the HTML template.
- [ ] Write unit and integration tests.
- [ ] Update documentation and examples.

## Dependencies

### Prerequisite Stories

- GUP-269B: WASM Module Integration ✅ — provides `render_from_bundle`
  and `WasmStrategy::Auto`.

## Testing Strategy

- **Unit tests**: Verify generated JS contains `import()` and function
  calls.
- **Integration tests**: Full HTML export with Module strategy.
- **Manual test**: Serve the HTML with wasm-pack output and verify in
  Chromium.

## Risk Assessment

- **Low**: ES module `import()` is well-supported in modern browsers.
- **Medium**: CORS restrictions may require the JS wrapper and WASM to be
  served from the same origin. Document this requirement.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete
- [ ] Retrospective added
