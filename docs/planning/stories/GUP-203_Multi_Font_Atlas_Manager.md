# GUP-203: Multi-Font Atlas Manager

**Status**: 🚧 In Progress **Priority**: Low **Complexity**: Medium **Created**:
2025-08-20

## Overview

Implement a font atlas manager that maintains multiple `FontAtlas` instances for
different fonts and enables efficient switching between them during rendering.

## Context

GUP-106 enabled loading fonts from the system by name, and GUP-202 would connect
this to the rendering pipeline via `TextStyle.font_family`. However, managing
multiple font atlases efficiently — including GPU memory limits, atlas eviction,
and texture binding optimization — requires a dedicated manager.

## User Story

As a developer rendering complex dashboards with many text elements, I want the
font system to efficiently manage multiple font atlases so that GPU memory is
used wisely and rendering remains performant even with many different fonts.

## Acceptance Criteria

- [ ] `FontAtlasManager` struct that caches and manages multiple `FontAtlas`
      instances
- [ ] LRU eviction policy for font atlases when GPU memory limit is reached
- [ ] Configurable maximum number of cached atlases
- [ ] Shared GPU texture arrays or atlases where possible for efficiency
- [ ] Telemetry: atlas count, memory usage, cache hit rate

## Technical Tasks

1. Design `FontAtlasManager` with LRU cache of font atlases
2. Implement GPU memory budget tracking
3. Add atlas eviction and recreation logic
4. Consider shared texture atlas for small font sizes
5. Add monitoring/debug output for atlas management

## Dependencies

- GUP-106 ✅ (System Font Loading)
- GUP-202 📋 (Font-Aware Text Rendering Pipeline)

## Testing Strategy

- Unit tests for atlas caching and eviction
- Memory budget enforcement tests
- Performance tests with many different fonts

## Risk Assessment

- **Low**: This is an optimization story; the basic multi-font capability works
  without it (just less efficiently).

## Definition of Done

- [ ] FontAtlasManager manages multiple atlases efficiently
- [ ] LRU eviction works under memory pressure
- [ ] All existing tests pass
- [ ] Documentation updated

---

**Estimated Effort**: 1-2 weeks **Prerequisites**: GUP-202 **Blockers**: None
