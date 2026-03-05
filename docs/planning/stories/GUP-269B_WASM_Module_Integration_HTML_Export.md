# GUP-269B: WASM Module Integration for HTML Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
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

- [ ] `WasmStrategy::Inline` (without a path argument) auto-discovers the
      `.wasm` artifact from the `wasm-pack` output directory, or a new
      `WasmStrategy::Auto` variant is introduced.
- [ ] The JavaScript bootstrap passes the `#gup-chart-data` JSON to the WASM
      module's init function.
- [ ] The Gup WASM entry point parses the JSON into a `ChartSnapshot` (or
      `ChartBundle`) and renders the chart onto the canvas.
- [ ] An integration test verifies the round-trip: export HTML → extract JSON →
      parse in Rust (simulating what the WASM module would do).
- [ ] Documentation covers the auto-discovery mechanism and fallback paths.

## Technical Tasks

- [ ] Implement auto-discovery of `pkg/*.wasm` files or accept a workspace root.
- [ ] Update the JavaScript bootstrap template to read the JSON block and pass
      it to the WASM instance.
- [ ] Implement or extend the WASM entry point (`#[wasm_bindgen(start)]`) to
      accept chart data from JavaScript.
- [ ] Write integration tests.
- [ ] Update the `html_export` example to demonstrate auto-discovery.

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

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete
- [ ] Retrospective added
