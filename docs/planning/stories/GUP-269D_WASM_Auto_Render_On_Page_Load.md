# GUP-269D: WASM Auto-Render on Page Load

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-07-19

## Context

GUP-269B added `render_from_bundle` as a WASM API that accepts JSON and
renders a chart. However, the current `#[wasm_bindgen(start)]` function
only sets up logging and panic hooks — it does not attempt to render.

For exported HTML pages to be fully interactive without additional JS
beyond the bootstrap, the WASM module should detect the presence of
`#gup-chart-data` in the DOM and automatically render the chart on load.
This requires an async spawn since WebGPU initialisation is async and
`#[wasm_bindgen(start)]` is synchronous.

## User Story

> "As a visualization developer, I want the exported HTML page to
> automatically render the chart when the WASM module loads, without
> needing any additional JavaScript beyond the bootstrap."

## Acceptance Criteria

- [ ] The `#[wasm_bindgen(start)]` function (or a post-init hook)
      detects `#gup-chart-data` in the DOM.
- [ ] If chart data is present, it spawns an async task to parse the
      JSON and render onto `#gup-canvas`.
- [ ] The auto-render is opt-in (feature-gated or conditional) to avoid
      unexpected rendering in non-export contexts.
- [ ] An exported HTML page with inline WASM renders interactively
      without any changes to the JS bootstrap.
- [ ] Tests verify the auto-render detection logic.

## Technical Tasks

- [ ] Enhance `#[wasm_bindgen(start)]` to check for `#gup-chart-data`.
- [ ] Use `wasm_bindgen_futures::spawn_local` for async WebGPU init.
- [ ] Add feature gate (e.g., `wasm-auto-render`) to control behaviour.
- [ ] Write tests for the detection logic (native side).
- [ ] Document the auto-render feature.

## Dependencies

### Prerequisite Stories

- GUP-269B: WASM Module Integration ✅ — provides `render_from_bundle`.
- GUP-269C: ES Module WASM Loading Strategy 📋 — may influence the
  loading mechanism (optional dependency).

## Testing Strategy

- **Unit tests**: Verify detection logic (DOM element presence check).
- **Manual test**: Open an exported HTML file in Chromium and verify the
  chart renders automatically.

## Risk Assessment

- **Medium**: Auto-rendering on load may interfere with applications
  that load the WASM module for other purposes (e.g., a Tauri app with
  custom rendering). Mitigated by feature gating.
- **Low**: `wasm_bindgen_futures::spawn_local` is well-tested.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete
- [ ] Retrospective added
