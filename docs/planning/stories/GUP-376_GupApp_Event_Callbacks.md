# GUP-376: GupApp Event Callbacks

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-07-17

## Context

GUP-318 migrated three windowed examples to `GupApp`, but many more examples
(e.g. `treemap_window.rs`, `simple_window.rs`, `visual_blend_demo.rs`) could not
be migrated because they handle custom keyboard or mouse events. Currently,
`AppRenderer` only has a `render()` method — there is no way to receive input
events through the `GupApp` shell.

Adding optional event callbacks to `GupApp` would enable migrating a significant
number of additional examples while keeping the API simple for the common case.

## User Story

> "As a visualisation developer, I want to handle keyboard and mouse events
> through `GupApp` without falling back to manual `ApplicationHandler`, so I can
> keep my application code minimal."

## Acceptance Criteria

- [ ] `GupApp` supports an optional `.on_key(callback)` builder method that
      receives key press events.
- [ ] `GupApp` supports an optional `.on_mouse(callback)` builder method that
      receives mouse/cursor events.
- [ ] At least one example that was previously manual is migrated to use the new
      callbacks (e.g. `simple_window.rs` or `treemap_window.rs`).
- [ ] The default behaviour (no callbacks) remains unchanged — existing `GupApp`
      usage is not affected.
- [ ] Built-in keyboard shortcuts (Escape/Q/F/S) still fire even when a custom
      key callback is registered.

## Dependencies

### Prerequisite Stories

- GUP-265: winit Application Shell ✅
- GUP-318: Migrate Existing Examples to GupApp ✅

## Technical Tasks

- [ ] Extend `GupApp` builder with `.on_key()` and `.on_mouse()` methods.
- [ ] Wire callbacks into `GupAppRunner::window_event()`.
- [ ] Migrate `simple_window.rs` (Space to cycle colours) to use `.on_key()`.
- [ ] Add unit tests for the callback wiring.
- [ ] Update `GupApp` module docs with callback examples.

## Testing Strategy

- **Unit**: Builder tests for callback registration.
- **Compilation**: `cargo check --examples` must pass.
- **Visual**: Run migrated example to confirm event handling works.

## Success Metrics

- At least one more example migrated to `GupApp`.
- `GupApp` API remains ≤ 10 builder methods (stays minimal).

## Risk Assessment

- **Low**: Callbacks are additive; no existing API changes.
- **Medium**: Deciding the right callback signature (pass raw `WindowEvent` vs a
  simplified event enum) requires care to keep the API ergonomic.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated in story file and INDEX.md
- [ ] Retrospective added to story document
