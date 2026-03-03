# GUP-285: High-Resolution GeoJSON Streaming

## Story Overview

**Initiative**: Mark System **Status**: 📋 Planned **Created**: 2025-07-17

## Context

GUP-274 (Map Mark Rendering) introduced `GeoJsonSource::from_str()` which loads
and parses the entire GeoJSON document synchronously on the calling thread. For
low-resolution datasets (Natural Earth 110m, ~20 KB), this is fast and
unproblematic. However, high-resolution boundary datasets (e.g., Natural Earth
10m, admin-level boundaries) can be 10–100 MB. Parsing these in a single call
blocks the render thread and may cause visible frame drops or timeouts.

This story adds a streaming/background parser that can ingest large GeoJSON
files without stalling the render loop.

## User Story

> "As a visualization developer working with high-resolution boundary data, I
> want to load large GeoJSON files without blocking the render thread so that
> the application remains responsive during data loading."

## Acceptance Criteria

- [ ] A `GeoJsonSource::from_reader()` async method accepts an `AsyncRead`
      source and yields `GeoFeature` items as they are parsed.
- [ ] A `GeoJsonSource::from_file_background()` spawns a background task that
      parses the file and sends features to the main thread via a channel.
- [ ] The `GeoPathMark` can accept features incrementally and update its
      tessellated geometry as new features arrive.
- [ ] A 50 MB GeoJSON file loads without dropping below 30 FPS on the render
      thread (measured via frame-time instrumentation).

## Dependencies

### Prerequisite Stories

- GUP-274: Map Mark Rendering ✅ — provides GeoJsonSource and GeoPathMark

## Testing Strategy

- Unit tests: streaming parser produces identical features to synchronous
  parser.
- Integration test: load a synthetic large GeoJSON file in the background,
  verify all features arrive and tessellate correctly.
- Performance test: measure frame times during background load.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated in INDEX.md
