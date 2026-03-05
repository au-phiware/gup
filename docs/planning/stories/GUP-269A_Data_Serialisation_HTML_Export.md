# GUP-269A: Data Serialisation in HTML Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-07-18 **Completed**: 2025-07-19

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

- [x] When `T: Serialize`, the `HtmlExporter` includes the `Selection`'s data
      items in the embedded JSON block.
- [x] A `ChartBundle<T>` (or extended `ChartSnapshot`) struct pairs the config
      snapshot with a `Vec<serde_json::Value>` (or `Vec<T>`) of data points.
- [x] Round-trip test: serialise a `ChartBundle` to JSON, deserialise it, and
      verify the data matches the original.
- [x] When `T` does not implement `Serialize`, the export still works but the
      JSON block contains only the config snapshot (backward compatible).
- [x] Documentation explains the data embedding and any size implications.

## Technical Tasks

- [x] Design `ChartBundle<T>` or extend `ChartSnapshot` with an optional
      `data: Option<serde_json::Value>` field.
- [x] Conditionally serialise data when `T: Serialize` (may require a separate
      code path or trait bound on the export method).
- [x] Update `HtmlExporter::render` to populate the data field.
- [x] Write unit tests for data round-trip.
- [x] Update the `html_export` example to demonstrate data embedding.

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

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete
- [x] Retrospective added

## Implementation Summary

### What was implemented

- **`ChartBundle` struct** (`src/export/html/snapshot.rs`): Pairs a
  `ChartSnapshot` config with an optional `Vec<serde_json::Value>` data array.
  Uses `#[serde(skip_serializing_if)]` to omit the `data` key when absent.
- **`HtmlExporter::render_with_data`** (`src/export/html/mod.rs`): New method
  with `T: Serialize` bound that serialises Selection data items into a
  `ChartBundle` JSON block.
- **`HtmlExporter::export_with_data`**: File-writing convenience wrapper.
- **`ComposedChart::export_html_with_data`** (`src/chart_builder.rs`):
  Convenience method on `ComposedChart` with `T: Serialize` bound.
- **Full backward compatibility**: Existing `render`/`export`/`export_html`
  methods unchanged; they produce plain `ChartSnapshot` JSON.
- **Module documentation** updated with data embedding explanation and size
  guidance.

### Key files changed

| File                                    | Change                                    |
| --------------------------------------- | ----------------------------------------- |
| `src/export/html/snapshot.rs`           | Added `ChartBundle` struct + 3 unit tests |
| `src/export/html/mod.rs`               | Added `render_with_data`, `export_with_data`, updated docs |
| `src/export/mod.rs`                     | Re-exported `ChartBundle`                 |
| `src/chart_builder.rs`                  | Added `export_html_with_data` convenience method |
| `tests/html_export_integration.rs`      | 5 new integration tests for data embedding |
| `examples/html_export.rs`              | Added `Serialize` derive, data export demo |

### Test counts

- **3 new unit tests**: `bundle_round_trip_with_data`, `bundle_config_only_omits_data`, `bundle_backward_compat_from_snapshot_json`
- **5 new integration tests**: `html_export_with_data_contains_bundle`, `html_export_with_data_round_trip`, `html_export_without_data_has_no_data_field`, `html_export_with_data_file_write`, `html_export_convenience_with_data`
- **Total**: 8 new tests; all 3,218+ project tests pass

## Retrospective

**Completed**: 2025-07-19

### Key Technical Learnings

#### Conditional Serialisation with Separate Methods

- **Challenge**: The story asked for data to be included "when `T: Serialize`"
  and omitted otherwise, ideally from the same export path. Rust doesn't
  support runtime trait detection or stable specialisation.
- **Solution**: Added parallel methods (`render_with_data` / `export_with_data`)
  that carry the extra `T: Serialize` bound. The existing `render` / `export`
  methods remain unchanged with their original bounds.
- **Pattern**: When a feature requires an additional trait bound, prefer adding
  a new method (or method set) rather than trying to make a single method
  conditionally behave differently. This is cleaner and more discoverable.

#### Type-Erased Data with `serde_json::Value`

- **Challenge**: The `ChartBundle` needs to store data from any `T: Serialize`
  without being generic itself (since it's deserialised on the WASM side
  without knowing `T` at compile time).
- **Solution**: Serialise each `T` to `serde_json::Value` via
  `serde_json::to_value()` and store as `Vec<serde_json::Value>`. The WASM
  consumer can deserialise into whatever type it expects.
- **Pattern**: Use `serde_json::Value` as a type-erased interchange format
  when the producer and consumer don't share compile-time type information.

### Architectural Decisions

#### Separate `ChartBundle` vs Extending `ChartSnapshot`

- **Decision**: Created a new `ChartBundle` struct that wraps `ChartSnapshot`
  rather than adding an `Option<Vec<Value>>` field to `ChartSnapshot`.
- **Reasoning**: `ChartSnapshot` is a clean DTO that mirrors `ChartConfig`
  fields. Adding a data field would conflate configuration with data. The
  wrapper approach keeps concerns separated and makes the JSON structure
  self-documenting (`{"config": {...}, "data": [...]}`).
- **Trade-off**: Consumers now need to handle two possible JSON formats
  (plain `ChartSnapshot` from old exports, or `ChartBundle` from new ones).
- **Future**: The `ChartBundle` structure is extensible — additional metadata
  (e.g., schema version, mark type hints) can be added as sibling fields
  to `config` and `data`.

#### Non-Generic `ChartBundle`

- **Decision**: `ChartBundle` uses `Vec<serde_json::Value>` rather than
  being generic as `ChartBundle<T>`.
- **Reasoning**: The bundle is meant to be round-tripped through JSON.
  Making it generic over `T` would require the deserialising side to know
  `T` at compile time, defeating the purpose of a self-contained HTML file.
  The type-erased approach lets the WASM module parse data dynamically.
- **Trade-off**: No compile-time type safety on the data array. Consumers
  must handle potential deserialisation errors gracefully.
- **Future**: A typed `ChartBundle<T>` could be offered as a convenience
  for Rust-to-Rust scenarios, but the primary use case (HTML export) benefits
  from type erasure.

### Development Workflow Insights

- The story was cleanly scoped with a well-defined prerequisite (GUP-269).
  The existing `HtmlExporter` and `ChartSnapshot` infrastructure made
  extension straightforward.
- Adding the `Serialize` bound only on the new methods preserved full
  backward compatibility — no existing code needed to change.
- The integration test infrastructure (helper functions, JSON extraction)
  from GUP-269 was easily extended for the new tests.
- Implementation was compact: ~120 lines of new library code, ~200 lines
  of new tests, across 4 incremental commits.

### Follow-up Stories

No new stories identified. GUP-269B (WASM Module Integration) is now
unblocked and is the natural next step in the HTML export initiative.
