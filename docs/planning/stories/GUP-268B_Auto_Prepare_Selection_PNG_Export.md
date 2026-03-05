# GUP-268B: Auto-Prepare Selection in PNG Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 💡 New **Created**: 2025-07-19

## Context

GUP-268A wired Selection data mark rendering through the PNG export path. However,
the caller must manually call `prepare_render` (with a mapper closure) or set up
attribute bindings and call `prepare_render_bound` before calling `render_to_png`.
If the caller forgets this step, the export succeeds but produces an image with
no data marks — only axes and grid.

This story adds a convenience API that auto-prepares the Selection as part of
the export call, reducing the chance of user error and simplifying the export
workflow.

## User Story

> "As a visualisation developer, I want `render_to_png` to automatically prepare
> the Selection's GPU resources when I've already defined attribute bindings, so
> that I don't need to manually call `prepare_render` before every export."

## Acceptance Criteria

- [ ] A new method (e.g., `export_png_with_mapper`) accepts a mapper closure and
      prepares the Selection as part of the export.
- [ ] When attribute bindings are set, `render_to_png` auto-calls
      `prepare_render_bound` if the Selection is not already render-ready.
- [ ] Existing API remains backward-compatible.
- [ ] Tests verify auto-preparation produces correct output.

## Technical Tasks

- [ ] Add `export_png_with_mapper` method on `ComposedChart` that accepts a
      mapper closure, calls `prepare_render`, then delegates to `render_to_png`.
- [ ] Optionally add auto-preparation logic in `render_to_rgba` that checks for
      attribute bindings and calls `prepare_render_bound` (requires adding
      `M: MarkInstanceBuilder` bound to the method).
- [ ] Add tests for the new convenience methods.

## Dependencies

### Prerequisite Stories

- GUP-268A: Data Mark PNG Export ✅ — provides the core mark rendering in export.

## Testing Strategy

- Unit tests for the new convenience methods.
- Integration tests verifying auto-preparation produces non-white pixels.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
