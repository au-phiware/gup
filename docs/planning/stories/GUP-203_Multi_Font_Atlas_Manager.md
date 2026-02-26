# GUP-203: Multi-Font Atlas Manager

**Status**: ✅ Complete (2025-08-22) **Priority**: Low **Complexity**: Medium
**Created**: 2025-08-20

## Overview

Implement a font atlas manager that maintains multiple `FontAtlas` instances for
different fonts and enables efficient switching between them during rendering.

## Context

GUP-106 enabled loading fonts from the system by name, and GUP-202 connected
this to the rendering pipeline via `TextStyle.font_family`. However, managing
multiple font atlases efficiently — including GPU memory limits, atlas eviction,
and texture binding optimization — requires a dedicated manager.

## User Story

As a developer rendering complex dashboards with many text elements, I want the
font system to efficiently manage multiple font atlases so that GPU memory is
used wisely and rendering remains performant even with many different fonts.

## Acceptance Criteria

- [x] `FontAtlasManager` struct that caches and manages multiple `FontAtlas`
      instances
- [x] LRU eviction policy for font atlases when GPU memory limit is reached
- [x] Configurable maximum number of cached atlases
- [x] Shared GPU texture arrays or atlases where possible for efficiency
- [x] Telemetry: atlas count, memory usage, cache hit rate

## Technical Tasks

1. Design `FontAtlasManager` with LRU cache of font atlases
2. Implement GPU memory budget tracking
3. Add atlas eviction and recreation logic
4. Consider shared texture atlas for small font sizes
5. Add monitoring/debug output for atlas management

## Dependencies

- GUP-106 ✅ (System Font Loading)
- GUP-202 ✅ (Font-Aware Text Rendering Pipeline)

## Testing Strategy

- Unit tests for atlas caching and eviction
- Memory budget enforcement tests
- Performance tests with many different fonts

## Risk Assessment

- **Low**: This is an optimization story; the basic multi-font capability works
  without it (just less efficiently).

## Definition of Done

- [x] FontAtlasManager manages multiple atlases efficiently
- [x] LRU eviction works under memory pressure
- [x] All existing tests pass
- [x] Documentation updated

## Implementation Summary

Enhanced the existing `FontAtlasManager` (from GUP-202) with LRU eviction,
capacity configuration, alias deduplication, and telemetry.

### Key Files Changed

- **`src/text/font.rs`**: Enhanced `FontAtlasManager` with LRU eviction, added
  `FontAtlasManagerConfig`, `FontAtlasStats`, and `BYTES_PER_ATLAS`. New public
  API: `with_config()`, `stats()`, `reset_stats()`, `config()`,
  `memory_used_bytes()`.
- **`src/text/atlas.rs`**: Promoted `FontAtlas::from_resolved()` to `pub(crate)`
  for internal use by the manager.

### New Types

| Type                     | Purpose                                             |
| ------------------------ | --------------------------------------------------- |
| `FontAtlasManagerConfig` | Configurable max_atlases and memory_budget_bytes    |
| `FontAtlasStats`         | Telemetry: atlas count, memory, hit rate, evictions |
| `BYTES_PER_ATLAS`        | Public constant for memory budget calculations      |

### Features

- **LRU eviction**: Least-recently-used non-default atlases evicted when
  `max_atlases` or `memory_budget_bytes` exceeded.
- **Alias deduplication**: Different font family names resolving to the same
  system font share one GPU atlas (e.g., "Helvetica" → "DejaVu Sans").
- **Default protection**: The embedded default atlas (`__default__`) is never
  evicted.

### Test Count

43 tests in `text::font` module (16 new tests added for LRU eviction, memory
budget, telemetry, aliasing, and configuration).

---

**Estimated Effort**: 1-2 weeks **Prerequisites**: GUP-202 **Blockers**: None
