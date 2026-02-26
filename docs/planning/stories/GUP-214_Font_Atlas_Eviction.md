# GUP-214: Font Atlas Eviction and Memory Management

**Status**: 📋 Planned **Priority**: Low **Complexity**: Medium **Created**:
2025-08-21

## Overview

Add LRU eviction and memory limits to `FontAtlasManager` to prevent unbounded
GPU memory growth when applications use many distinct font families.

## Context

GUP-202 introduced `FontAtlasManager` which lazily creates a `FontAtlas` per
font family. Each atlas allocates a 1024×1024 RGBA GPU texture (~4 MB). In
applications that dynamically render many fonts (e.g., user-supplied data with
font metadata), atlases accumulate without bound.

## User Story

As a developer building data visualizations with user-supplied font
specifications, I want the font atlas system to manage its own memory so that
GPU memory doesn't grow unbounded as different fonts are requested.

## Acceptance Criteria

- [ ] `FontAtlasManager` supports a configurable maximum atlas count
- [ ] Least-recently-used atlases are evicted when the limit is reached
- [ ] Evicted atlases are transparently re-created if requested again
- [ ] GPU memory usage is bounded by the configured limit
- [ ] Default limit is generous enough for typical visualisations (e.g., 16)

## Technical Tasks

1. Add `max_atlases` configuration to `FontAtlasManager`
2. Track last-use timestamps or access order for LRU eviction
3. Implement eviction logic in `get_or_create()`
4. Add tests for eviction behavior

## Dependencies

- GUP-202 ✅ (Font-Aware Text Rendering Pipeline)

## Testing Strategy

- Unit tests for LRU eviction ordering
- GPU integration tests verifying atlas count stays within limits
- Memory usage measurement tests

## Risk Assessment

- **Low**: Eviction is a straightforward LRU cache pattern. The main risk is
  thrashing if the limit is too low for the workload.

## Definition of Done

- [ ] Atlas eviction works correctly under the configured limit
- [ ] All existing tests pass
- [ ] Documentation updated with memory management guidance
- [ ] Example or test demonstrates eviction behavior

---

**Estimated Effort**: 3-5 days **Prerequisites**: GUP-202 ✅ **Blockers**: None
