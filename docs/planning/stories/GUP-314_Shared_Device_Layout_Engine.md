# GUP-314: Shared Device Layout Engine

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-20

## Context

GUP-311 introduced the interactive graph rendering example, which creates two
separate GPU contexts: one `RenderContext` for the layout engine and one
`GupContext` for windowed rendering. On integrated GPUs with limited resources,
this wastes memory and prevents buffer sharing.

## User Story

> "As a developer, I want the layout engine to share the same wgpu device as my
> rendering context so I can avoid allocating a second GPU context."

## Acceptance Criteria

- [ ] `LayoutEngine` can be created from a `GupContext` (not just
      `RenderContext`)
- [ ] Layout compute and rendering share one `wgpu::Device`
- [ ] The interactive graph example uses a single GPU context
- [ ] No regression in layout correctness or performance

## Technical Tasks

1. Add `LayoutEngine::from_gup_context(ctx: &GupContext)` constructor
2. Refactor `LayoutEngine::new()` to accept a trait or enum covering both context
   types
3. Update `interactive_graph.rs` to use the shared context
4. Verify all layout integration tests still pass

## Dependencies

### Prerequisite Stories

- GUP-311: Interactive Force-Directed Graph Rendering ✅

## Testing Strategy

- All existing layout tests must pass
- New test: create engine from GupContext (headless)
- Visual regression: interactive graph example still renders correctly

## Risk Assessment

- **Low**: The engine only needs `Device` and `Queue`, which both context types
  provide. The refactor is straightforward.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint clean
- [ ] Retrospective added
