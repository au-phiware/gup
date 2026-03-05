# GUP-269A: Data Serialisation in HTML Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
2025-07-18

## Context

GUP-269 (HTML Export) introduced the `HtmlExporter` which embeds a
`ChartSnapshot` JSON block containing chart configuration (dimensions, margins,
title, axis/grid toggles). However, the actual data values (`T` instances bound
to the chart's `Selection`) are not included. This means the embedded WASM
module cannot fully reconstruct the chart from the JSON alone — it would need a
separate data feed.

Extending `ChartSnapshot` (or introducing a `ChartBundle` wrapper) to include
serialised data points would make the HTML export truly self-contained: the WASM
module could parse the embedded JSON and render the complete chart without any
external data source.

## User Story

> "As a visualization developer, I want the HTML export to embed my chart data
> in the JSON block so that the WASM module can render the complete chart from a
> single file without a separate data API."

## Acceptance Criteria

- [ ] When `T: Serialize`, the `HtmlExporter` includes the `Selection`'s data
      items in the embedded JSON block.
- [ ] A `ChartBundle<T>` (or extended `ChartSnapshot`) struct pairs the config
      snapshot with a `Vec<serde_json::Value>` (or `Vec<T>`) of data points.
- [ ] Round-trip test: serialise a `ChartBundle` to JSON, deserialise it, and
      verify the data matches the original.
- [ ] When `T` does not implement `Serialize`, the export still works but the
      JSON block contains only the config snapshot (backward compatible).
- [ ] Documentation explains the data embedding and any size implications.

## Technical Tasks

- [ ] Design `ChartBundle<T>` or extend `ChartSnapshot` with an optional
      `data: Option<serde_json::Value>` field.
- [ ] Conditionally serialise data when `T: Serialize` (may require a separate
      code path or trait bound on the export method).
- [ ] Update `HtmlExporter::render` to populate the data field.
- [ ] Write unit tests for data round-trip.
- [ ] Update the `html_export` example to demonstrate data embedding.

## Dependencies

### Prerequisite Stories

- GUP-269: HTML Export ✅ — provides the HtmlExporter and ChartSnapshot
  foundation.

### Enables Stories

- GUP-269B: WASM Module Integration for HTML Export — the WASM module would read
  data from the embedded JSON.

## Testing Strategy

- **Unit tests**: Verify data serialisation round-trip with known data types.
- **Integration tests**: Export HTML with data, extract JSON, verify data
  presence and correctness.

## Risk Assessment

- **Low**: Adding `T: Serialize` as a bound only on the export path preserves
  backward compatibility for charts that never export.
- **Medium**: Large datasets could produce very large HTML files. May need a
  compression or truncation strategy.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete
- [ ] Retrospective added
