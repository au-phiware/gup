# GUP-233: Winit Touch Event Integration

**Status**: 📋 Planned **Priority**: Low **Effort**: 2 **Dependencies**: GUP-182
(Touch Selection Support)

## Overview

Add a convenience `From<winit::event::Touch>` conversion for
`TouchEvent`/`TouchPhase` and update the `interactive_selection_demo` example to
handle `WindowEvent::Touch` events through the `TouchSelectionAdapter`, providing
an end-to-end touch selection workflow on desktop touch screens.

## Context

GUP-182 delivered the `TouchSelectionAdapter` with its own windowing-agnostic
`TouchEvent` and `TouchPhase` types. The demo example currently only processes
`WindowEvent::CursorMoved` and `WindowEvent::MouseInput`. Adding winit touch
event mapping completes the integration and provides a working reference for
users.

## User Story

As a developer integrating Gup with winit, I want a zero-boilerplate way to
convert winit touch events into Gup touch events so that I can enable touch
selection without writing manual mapping code.

## Acceptance Criteria

1. `From<winit::event::Touch>` and `From<winit::event::TouchPhase>` impls
   convert winit touch events to Gup types
2. The `interactive_selection_demo` handles `WindowEvent::Touch` events
3. Conversions are behind a `winit` feature flag (or always available if winit is
   already a required dependency)
4. Documentation and doc-tests cover the conversion

## Technical Tasks

- [ ] Implement `From<winit::event::Touch>` for `TouchEvent`
- [ ] Implement `From<winit::event::TouchPhase>` for `TouchPhase`
- [ ] Update `interactive_selection_demo.rs` to process touch events
- [ ] Add unit tests for conversions
- [ ] Add doc examples

## Testing Strategy

- Unit tests for `From` conversions
- Manual testing on a touch-enabled device or emulator

## Risk Assessment

- **Low**: Straightforward type mapping; winit is already a dependency

## Definition of Done

- [ ] `From` impls exist and compile
- [ ] Demo processes touch events
- [ ] All tests pass
