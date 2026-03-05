# GUP-352: Interactive Tutorial Examples

## Story Overview

**Initiative**: Documentation **Status**: ✅ Complete **Created**: 2025-07-15
**Completed**: 2025-07-26

## Context

The tutorials delivered in GUP-281 reference existing examples from the GUP-103
suite. These examples often demonstrate more features than a single tutorial
covers, which can be overwhelming for a reader who just finished a tutorial and
wants to see exactly what they built rendered in a window. Dedicated tutorial
examples that match the tutorial code verbatim would provide a smoother learning
experience.

## User Story

> "As a developer following a Gup tutorial, I want to run a single command and
> see exactly the chart described in the tutorial, without extra features or
> complexity that I haven't learned yet."

## Acceptance Criteria

- [x] A `examples/tutorials/` directory exists with one example per tutorial
      (where visual output is meaningful).
- [x] Each tutorial example is a self-contained windowed program that renders
      the tutorial's "Full Example" chart.
- [x] Tutorial documents are updated to reference these dedicated examples.
- [x] Each example includes a quit mechanism (ESC or window close).

## Technical Tasks

- [x] Create `examples/tutorials/` directory.
- [x] Implement `tutorial01_scatter.rs` — windowed scatter chart from
      Tutorial 1.
- [x] Implement `tutorial04_interactions.rs` — interactive chart from
      Tutorial 4.
- [x] Implement `tutorial05_streaming.rs` — streaming chart from Tutorial 5.
- [x] Implement `tutorial06_custom_marks.rs` — custom mark chart from
      Tutorial 6.
- [x] Update tutorial documents with links to the new examples.
- [x] Update screenshot sources in `docs/tutorials/README.md`.

## Dependencies

### Prerequisite Stories

- GUP-281: Tutorial and Guide Suite ✅ — The tutorials whose examples to create.
- GUP-103: Comprehensive Chart Examples Suite ✅ — Pattern source for windowed
  examples.

## Testing Strategy

- Each example compiles via `cargo check --examples`.
- Each example runs and renders visible output when launched.

## Success Metrics

- [x] At least 4 tutorial examples exist and compile.
- [x] Screenshots from tutorial examples match the tutorials.

## Risk Assessment

- **Low**: Tutorial examples may duplicate code from existing examples. Keep
  them minimal and focused on the tutorial content.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass
- [x] Story status updated to ✅ Complete

## Implementation Summary

### What Was Implemented

Four dedicated tutorial examples in `examples/tutorials/`:

1. **`tutorial01_scatter.rs`** — Windowed scatter chart using GupApp, rendering
   the five-point data set from Tutorial 1 with GPU-accelerated circles.
2. **`tutorial04_interactions.rs`** — Interactive scatter chart with click and
   hover handlers using the event system, rendering the three-point data set
   from Tutorial 4's Full Example.
3. **`tutorial05_streaming.rs`** — Headless streaming example with a
   `SineWaveSource` implementing `StreamingDataSource`, wired to
   `StreamingScatterPlot` as in Tutorial 5.
4. **`tutorial06_custom_marks.rs`** — Headless custom mark example defining an
   `Arrow` mark with `#[derive(Mark)]` and validating it, matching Tutorial 6's
   Full Derive Example.

### Key Files Changed

- `examples/tutorials/tutorial01_scatter.rs` (new)
- `examples/tutorials/tutorial04_interactions.rs` (new)
- `examples/tutorials/tutorial05_streaming.rs` (new)
- `examples/tutorials/tutorial06_custom_marks.rs` (new)
- `Cargo.toml` — registered 4 new examples
- `docs/tutorials/01_getting_started.md` — added tutorial example link
- `docs/tutorials/04_interactions.md` — added tutorial example link
- `docs/tutorials/05_streaming_data.md` — added tutorial example link
- `docs/tutorials/06_custom_marks.md` — added tutorial example link
- `docs/tutorials/README.md` — updated screenshot source commands

### Test Counts

- 13 unit tests across all four tutorial examples (all passing)
- Full test suite: 3015+ tests pass with 0 failures
