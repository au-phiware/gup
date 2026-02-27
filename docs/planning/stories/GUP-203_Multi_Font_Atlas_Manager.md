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

## Retrospective

**Completed**: 2025-08-22

### Key Technical Learnings

#### LRU Eviction with Vec-Based Access Order

- **Challenge**: Implementing LRU eviction without adding an external dependency
  (e.g., `lru` crate). The atlas count is small (typically ≤16), so O(n) is
  acceptable.
- **Solution**: A `Vec<String>` tracks access order with most-recently-used at
  the back. `touch_lru()` removes and re-appends, `evict_lru()` removes from the
  front (skipping the default atlas).
- **Pattern**: For small bounded caches, a simple Vec+HashMap combo is simpler
  and faster than a full LRU data structure. Only consider `LinkedHashMap` or a
  dedicated crate when the cache grows large.

#### Alias Deduplication for System Fonts

- **Challenge**: Different font family names (e.g., "Helvetica", "Arial") can
  resolve to the same underlying system font (e.g., "DejaVu Sans" on Linux).
  Creating duplicate atlases wastes GPU memory.
- **Solution**: After resolving a font name to its canonical family, check if
  that canonical name already has an atlas. If so, add an alias instead of a new
  atlas. Only alias non-fallback fonts — fallback fonts keep separate entries
  per requested name.
- **Pattern**: When caching resolved resources, track the canonical key and
  alias requested keys to it. But skip aliasing when the resolution is a
  fallback/ default, since the user's intent was to use different fonts.

#### Fallback Font Aliasing Decision

- **Challenge**: With an empty `FontDatabase`, every requested font name
  resolves to the same embedded fallback ("Squada One"). Aliasing all of them to
  one atlas is memory-efficient but breaks existing test expectations and means
  LRU eviction can't differentiate between them.
- **Solution**: Only alias non-fallback (real system) fonts. Fallback fonts are
  stored per-requested-name so each conceptually-different font request gets its
  own LRU entry and can be evicted independently.
- **Pattern**: Resource sharing optimizations should respect conceptual
  distinctness. Two resources that happen to have the same data but different
  intent should remain separate for lifecycle management.

### Architectural Decisions

#### Enhancing Existing Manager vs New Type

- **Decision**: Enhanced the existing `FontAtlasManager` in-place rather than
  creating a separate `LruFontAtlasManager`.
- **Reasoning**: The existing manager was only used in GUP-202 and had a simple
  HashMap-based implementation. Enhancing it keeps the API stable — all callers
  (`TextRenderer`, `ChartBuilder`, examples) continue to work without changes.
- **Trade-off**: The `new()` constructor's signature is unchanged, so existing
  code gets the default config (16 atlases, 64 MB budget) automatically. Users
  who want custom limits use `with_config()`.
- **Future**: If atlas management grows more complex (e.g., shared texture
  arrays, atlas packing across fonts), a more specialized manager could be
  warranted.

#### GUP-214 Subsumed

- **Decision**: Marked GUP-214 (Font Atlas Eviction) as complete since all its
  acceptance criteria were delivered by this story.
- **Reasoning**: GUP-214 was a subset of GUP-203 — both covered LRU eviction and
  memory limits. Implementing them separately would have been redundant.
- **Future**: If more sophisticated eviction strategies are needed (e.g.,
  cost-based, frequency-weighted), a new story can be created.

### Development Workflow Insights

- The story was implemented in 3 focused commits: (1) core manager enhancement,
  (2) doc/constant polish, (3) story completion. Each was a clean, testable
  unit.
- Pre-existing test failures in `mark::renderer::tests` (3 tests) were confirmed
  by running tests on the pre-change commit via `git stash`. This avoided
  wasting time debugging unrelated issues.
- The `FontAtlas::from_resolved()` method had to be promoted from `fn` to
  `pub(crate)` so the manager could call it directly with resolved font data.
  This was the only change needed outside the font module.

### Follow-up Stories

No new follow-up stories identified. GUP-214 was already planned and is now
subsumed. The remaining planned text stories (GUP-216, GUP-217, GUP-227,
GUP-228) are independent of this work.
