# GUP-269B: WASM Module Integration for HTML Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-07-18

## Context

GUP-269 (HTML Export) implemented `WasmStrategy::Inline(PathBuf)` which requires
the user to manually specify the path to a `.wasm` file. In practice, the WASM
artifact is produced by `wasm-pack` into a predictable output directory
(`pkg/<name>_bg.wasm`). Automatically locating and embedding the correct WASM
binary would streamline the export workflow.

Additionally, the WASM module loaded by the HTML page currently receives no
initialisation data from the embedded JSON. Wiring the JavaScript bootstrap to
read `#gup-chart-data`, pass it to the WASM `_start` or `init` function, and
have the Rust WASM code parse and render it would close the loop for a fully
interactive export.

## User Story

> "As a visualization developer, I want the HTML exporter to automatically find
> my WASM build output and generate a page where the embedded WASM module reads
> the chart data from the JSON block, so that I get a fully interactive chart
> with a single export call."

## Acceptance Criteria

- [x] `WasmStrategy::Inline` (without a path argument) auto-discovers the
      `.wasm` artifact from the `wasm-pack` output directory, or a new
      `WasmStrategy::Auto` variant is introduced.
- [x] The JavaScript bootstrap passes the `#gup-chart-data` JSON to the WASM
      module's init function.
- [x] The Gup WASM entry point parses the JSON into a `ChartSnapshot` (or
      `ChartBundle`) and renders the chart onto the canvas.
- [x] An integration test verifies the round-trip: export HTML → extract JSON →
      parse in Rust (simulating what the WASM module would do).
- [x] Documentation covers the auto-discovery mechanism and fallback paths.

## Technical Tasks

- [x] Implement auto-discovery of `pkg/*.wasm` files or accept a workspace root.
- [x] Update the JavaScript bootstrap template to read the JSON block and pass
      it to the WASM instance.
- [x] Implement or extend the WASM entry point (`#[wasm_bindgen(start)]`) to
      accept chart data from JavaScript.
- [x] Write integration tests.
- [x] Update the `html_export` example to demonstrate auto-discovery.

## Dependencies

### Prerequisite Stories

- GUP-269: HTML Export ✅ — provides the HtmlExporter and template foundation.
- GUP-172: WebAssembly Performance Benchmarks ✅ — established WASM build
  pipeline.
- GUP-269A: Data Serialisation in HTML Export 📋 — provides the data embedding.

## Testing Strategy

- **Unit tests**: Verify auto-discovery logic finds the WASM file.
- **Integration tests**: Full export and JSON extraction round-trip.
- **Manual test**: Open exported HTML in Chromium, verify interactive rendering.

## Risk Assessment

- **Medium**: Auto-discovery assumes a `wasm-pack` layout. Custom build
  pipelines may put the WASM file elsewhere. Mitigation: fall back to explicit
  path.
- **Medium**: Passing data from JavaScript to WASM requires agreeing on a
  serialisation format. Using JSON via `serde_json` is the natural choice.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete
- [ ] Retrospective added

## Implementation Summary

**Completed**: 2025-07-19

### What Was Implemented

1. **`WasmStrategy::Auto(Option<PathBuf>)`** — New enum variant that
   auto-discovers the `*_bg.wasm` artifact from the `wasm-pack` output
   directory (`pkg/`). Searches from the current directory or a specified
   workspace root.

2. **`discover_wasm_artifact()` public function** — Reusable discovery
   logic with comprehensive error messages for missing `pkg/`, no WASM
   files, or ambiguous matches (multiple `*_bg.wasm` files).

3. **Updated JavaScript bootstrap templates** — Both inline and URL
   strategies now read the `#gup-chart-data` JSON block and store it as
   `window.__GUP_CHART_DATA__` before WASM instantiation.

4. **`render_from_bundle` WASM API** — New `#[wasm_bindgen]` export that
   accepts a canvas ID and JSON string, parses it as `ChartBundle` or
   `ChartSnapshot`, extracts data, and renders via `render_scatter`.

5. **`parse_bundle_json` native helper** — Same parsing logic available
   on native targets for testing the round-trip pipeline without a browser.

6. **Module documentation** — Comprehensive docs covering WASM strategies,
   auto-discovery mechanism, JS↔WASM data passing, and fallback paths.

### Key Files Changed

| File | Change |
|------|--------|
| `src/export/html/mod.rs` | `WasmStrategy::Auto`, `discover_wasm_artifact()`, docs |
| `src/export/html/template.rs` | JS templates read chart data JSON |
| `src/export/mod.rs` | Re-export `discover_wasm_artifact` |
| `src/wasm_api.rs` | `render_from_bundle`, `parse_bundle_json` |
| `tests/html_export_integration.rs` | 4 new integration tests |
| `examples/html_export.rs` | Auto strategy example (commented) |

### Test Counts

- **Unit tests**: 10 new (5 auto-discovery, 2 JS template, 4 parse_bundle_json, -1 dedup)
- **Integration tests**: 4 new (WASM round-trip, config-only round-trip, JS reads chart data, Auto strategy)
- **Total new tests**: 14
