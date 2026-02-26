# GUP-185: Multi-Pass Mark Examples

**Status**: 🚧 In Progress **Priority**: Low **Category**: Examples / Documentation
**Estimated Effort**: 1 day **Dependencies**: GUP-069 (Advanced Mark Rendering
Features)

## Overview

Create example marks that use multi-pass rendering to validate the multi-pass
API with visual output. These examples serve as both documentation and
integration tests.

## Context

GUP-069 introduced `MultiPassConfig`, `MultiPassRenderer`, and
`MarkInfoImpl::create_render_pipeline_for_pass()` for multi-pass rendering.
While the API is fully tested, no visual examples exist yet demonstrating the
feature in action.

## User Story

**As a** visualization developer **I want** to see working examples of
multi-pass mark rendering **So that** I understand how to implement custom marks
with fill + outline, shadow effects, and similar multi-layer visuals.

## Acceptance Criteria

- [ ] Stroked circle example: fill pass + outline pass
- [ ] Drop-shadow example: shadow pass + main pass
- [ ] Example renders correctly and demonstrates visual layering
- [ ] README/doc comments explain the multi-pass pattern

## Technical Tasks

1. Create a `StrokedCircle` mark that uses two-pass rendering
2. Create example that renders stroked circles with configurable outline width
3. Create a shadow effect example using multi-pass with offset
4. Add documentation showing the multi-pass configuration pattern

## Testing Strategy

- Visual validation of rendered output
- Ensure examples compile and run without errors

## Success Metrics

- Examples render correctly with visible multi-pass effects
- Examples serve as documentation for the multi-pass API

## Risk Assessment

- **Low risk**: builds on tested infrastructure
- GPU `PolygonMode::Line` requires `NON_FILL_POLYGON_MODE` feature which may not
  be available on all GPUs — examples should handle gracefully

## Definition of Done

- [ ] Examples compile and run
- [ ] Visual output demonstrates multi-pass rendering
- [ ] Documentation explains the pattern
