# GUP-309: Viewport-Aware Eviction Policy

## Story Overview

**Initiative**: Advanced Scale **Status**: 💡 New **Created**: 2025-07-17

## Context

The `StreamingLodManager` (GUP-258) evicts data points in strict insertion order
(`OldestFirst`). For panning/zooming use cases, this may evict points that are
currently visible while retaining off-screen data. A viewport-aware eviction
policy would prioritise retaining points within or near the current viewport.

## User Story

As a developer building an interactive streaming map, I want the LOD manager to
evict off-screen points first so that the visible region always has the highest
data density.

## Acceptance Criteria

- [ ] A `NearestViewport` eviction strategy is added to `EvictionPolicy`.
- [ ] `StreamingLodManager::poll()` accepts an optional viewport parameter to
      drive viewport-aware eviction.
- [ ] When the budget is exceeded, points furthest from the viewport centre are
      evicted before closer points.
- [ ] A unit test verifies that two points equidistant from the viewport centre
      are evicted in insertion order (tie-breaking).
- [ ] A unit test verifies that an off-screen point is evicted before an on-screen
      point even if the on-screen point is older.

## Dependencies

- GUP-258 ✅ (StreamingLodManager, EvictionPolicy)

## Testing Strategy

- Unit tests for viewport distance calculations.
- Integration test: pan viewport, verify eviction preference.

## Risk Assessment

- **Low**: Requires per-point distance calculation during eviction, which is
  O(N) in the worst case. Mitigation: maintain a sorted/indexed structure for
  spatial eviction queries.

## Definition of Done

- [ ] All acceptance criteria satisfied
- [ ] Tests pass: `cargo test -- --test-threads=1`
- [ ] Lint clean: `mask all-fix`
