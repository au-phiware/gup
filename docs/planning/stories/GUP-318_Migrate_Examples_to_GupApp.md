# GUP-318: Migrate Existing Examples to GupApp

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Completed**:
2025-07-17 **Created**: 2025-03-04

## Context

GUP-265 introduced `GupApp`, a builder that eliminates the 50–100 lines of winit
boilerplate present in every windowed example. However, all existing windowed
examples still implement `ApplicationHandler` manually. Migrating suitable
examples to `GupApp` will:

1. Demonstrate `GupApp`'s versatility across different chart types.
2. Reduce total code in the repository by removing duplicated lifecycle code.
3. Serve as additional integration tests for the shell.

Not every example should be migrated — multi-window examples and those that
demonstrate advanced event handling should remain as reference implementations
of the manual approach.

## User Story

> "As a contributor, I want the example suite to showcase `GupApp` alongside the
> manual `ApplicationHandler` approach, so I can learn both patterns."

## Acceptance Criteria

- [x] `examples/basic/02_scatter_window.rs` is migrated to use `GupApp` and its
      `main()` body is ≤ 5 lines.
- [x] At least two other windowed examples are migrated where appropriate.
- [x] Multi-window and advanced-event examples remain unchanged and are
      documented as intentionally manual.
- [x] All migrated examples compile and run correctly.
- [x] A short paragraph in `docs/` or the example doc-comments explains when to
      use `GupApp` vs the manual approach.

## Dependencies

### Prerequisite Stories

- GUP-265: winit Application Shell ✅

## Technical Tasks

- [x] Identify which examples are suitable for `GupApp` migration.
- [x] Refactor each suitable example: extract the renderer into an `AppRenderer`
      impl, replace the `ApplicationHandler` boilerplate with
      `GupApp::new(renderer).run()`.
- [x] Add doc comments explaining the migration and when manual handling is
      preferred.
- [x] Verify all examples compile: `cargo check --examples`.

## Testing Strategy

- **Compilation**: `cargo check --examples` must pass.
- **Visual**: run each migrated example and confirm correct rendering and
  keyboard shortcuts.

## Risk Assessment

- **Low**: `GupApp` wraps the same pattern already used in the examples, so
  migration is a mechanical refactor.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

### What Was Implemented

Three windowed examples were migrated from manual `ApplicationHandler` to
`GupApp`:

1. **`examples/basic/02_scatter_window.rs`** — `ScatterRenderer` now implements
   `AppRenderer`; main() reduced from 15 lines to 4.
2. **`examples/boxplot_rendering_demo.rs`** — `BoxPlotRenderer` now implements
   `AppRenderer`; prepare/viewport logic moved into render() using
   `frame.device()`, `frame.queue()`, and `frame.surface_size()`.
3. **`examples/multi_pass_mark_demo.rs`** — `LazyMultiPassDemo` wrapper provides
   lazy GPU resource initialisation via `AppRenderer`.

Four examples were annotated as intentionally manual:

- `multi_window_demo.rs` — multi-window
- `windowed_demo.rs` — multi-window
- `treemap_window.rs` — custom keyboard shortcuts (C, A)
- `simple_window.rs` — custom keyboard shortcut (Space)

### Key Files Changed

| File | Change |
|------|--------|
| `src/context.rs` | Added `RenderFrame::surface_size()` |
| `examples/basic/02_scatter_window.rs` | Migrated to GupApp (–160 lines) |
| `examples/boxplot_rendering_demo.rs` | Migrated to GupApp (–170 lines) |
| `examples/multi_pass_mark_demo.rs` | Migrated to GupApp (–160 lines) |
| `examples/multi_window_demo.rs` | Added "intentionally manual" doc comment |
| `examples/windowed_demo.rs` | Added "intentionally manual" doc comment |
| `examples/treemap_window.rs` | Added "intentionally manual" doc comment |
| `examples/simple_window.rs` | Added "intentionally manual" doc comment |

### Test Counts

- All 3015+ library tests pass
- All 4 scatter window example tests pass
- All 4 boxplot example tests pass
- All examples compile (`cargo check --examples`)
