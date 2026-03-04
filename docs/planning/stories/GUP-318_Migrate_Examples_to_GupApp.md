# GUP-318: Migrate Existing Examples to GupApp

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-03-04

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

- [ ] `examples/basic/02_scatter_window.rs` is migrated to use `GupApp` and its
      `main()` body is ≤ 5 lines.
- [ ] At least two other windowed examples are migrated where appropriate.
- [ ] Multi-window and advanced-event examples remain unchanged and are
      documented as intentionally manual.
- [ ] All migrated examples compile and run correctly.
- [ ] A short paragraph in `docs/` or the example doc-comments explains when to
      use `GupApp` vs the manual approach.

## Dependencies

### Prerequisite Stories

- GUP-265: winit Application Shell ✅

## Technical Tasks

- [ ] Identify which examples are suitable for `GupApp` migration.
- [ ] Refactor each suitable example: extract the renderer into an `AppRenderer`
      impl, replace the `ApplicationHandler` boilerplate with
      `GupApp::new(renderer).run()`.
- [ ] Add doc comments explaining the migration and when manual handling is
      preferred.
- [ ] Verify all examples compile: `cargo check --examples`.

## Testing Strategy

- **Compilation**: `cargo check --examples` must pass.
- **Visual**: run each migrated example and confirm correct rendering and
  keyboard shortcuts.

## Risk Assessment

- **Low**: `GupApp` wraps the same pattern already used in the examples, so
  migration is a mechanical refactor.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated in story file and INDEX.md
- [ ] Retrospective added to story document
