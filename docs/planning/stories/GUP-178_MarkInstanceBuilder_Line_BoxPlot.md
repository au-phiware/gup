# GUP-178: MarkInstanceBuilder for Line and BoxPlot

**Status**: 🚧 In Progress

## Story Overview

**Title**: Extend MarkInstanceBuilder to Line and BoxPlot marks **Epic**: Phase
1 Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**: 3

## Context

GUP-168 implemented `MarkInstanceBuilder` for `Circle` and `Rectangle` marks,
enabling declarative attribute binding via `attr()` and
`prepare_render_bound()`. The `Line` and `BoxPlot` marks still require the
manual `prepare_render(mapper)` path.

This story extends `MarkInstanceBuilder` to the remaining mark types for
complete coverage.

## User Story

**As a** library user **I want** to use declarative `attr()` bindings with Line
and BoxPlot marks **So that** I have a consistent API across all mark types

## Acceptance Criteria

- [ ] `MarkInstanceBuilder` implemented for `Line` mark
- [ ] `MarkInstanceBuilder` implemented for `BoxPlot` mark
- [ ] Attribute name aliases consistent with Circle and Rectangle
- [ ] BoxPlot-specific attributes (min, q1, median, q3, max, etc.) supported
- [ ] GPU integration tests for Line and BoxPlot with `prepare_render_bound()`
- [ ] Documentation updated

## Dependencies

- **Requires**: GUP-168 (Selection Attribute Binding Pipeline) ✅
- **Requires**: GUP-067 (Rectangle and Line Mark Types) ✅
- **Requires**: GUP-166 (Unified BoxPlot Mark Renderer) ✅

## Testing Strategy

- Unit tests for Line and BoxPlot instance builders
- GPU integration tests for prepare_render_bound with both marks
- Test attribute aliases consistency

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Existing tests still pass
- [ ] `mask all-fix` clean
