# GUP-214: Font Atlas Eviction and Memory Management

**Status**: ✅ Complete (2025-08-22) **Priority**: Low **Complexity**: Medium
**Created**: 2025-08-21

## Overview

Add LRU eviction and memory limits to `FontAtlasManager` to prevent unbounded
GPU memory growth when applications use many distinct font families.

## Context

GUP-202 introduced `FontAtlasManager` which lazily creates a `FontAtlas` per
font family. Each atlas allocates a 1024×1024 RGBA GPU texture (~4 MB). In
applications that dynamically render many fonts (e.g., user-supplied data with
font metadata), atlases accumulate without bound.

**Note**: All acceptance criteria were implemented as part of
[GUP-203](GUP-203_Multi_Font_Atlas_Manager.md) (Multi-Font Atlas Manager), which
was the parent story covering the same functionality.

## User Story

As a developer building data visualizations with user-supplied font
specifications, I want the font atlas system to manage its own memory so that
GPU memory doesn't grow unbounded as different fonts are requested.

## Acceptance Criteria

- [x] `FontAtlasManager` supports a configurable maximum atlas count
- [x] Least-recently-used atlases are evicted when the limit is reached
- [x] Evicted atlases are transparently re-created if requested again
- [x] GPU memory usage is bounded by the configured limit
- [x] Default limit is generous enough for typical visualisations (e.g., 16)

_All criteria delivered by GUP-203._

## Dependencies

- GUP-202 ✅ (Font-Aware Text Rendering Pipeline)
- GUP-203 ✅ (Multi-Font Atlas Manager — delivered this functionality)

## Implementation Summary

Subsumed by GUP-203. See that story for implementation details.

---

**Estimated Effort**: 3-5 days **Prerequisites**: GUP-202 ✅ **Blockers**: None
