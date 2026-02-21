# GUP-151: Surface Configuration Caching

**Status**: 💡 New

## Story Overview

**Title**: Surface Configuration Caching
**Epic**: Phase 1 Initiative 1 - Core GPU Primitives and Selection API
**Priority**: Low
**Story Points**: 2

## Context

After device recovery, surfaces must be recreated by the application. We currently don't cache surface configurations (size, format, scale factor) which forces applications to track and restore these settings. Caching configurations would enable automatic surface recreation with the same settings.

## User Story

**As a** Gup application developer
**I want** surface configurations to be preserved during recovery
**So that** surfaces are automatically restored to their previous settings

## Acceptance Criteria

- [ ] Cache surface size, format, and scale factor before loss
- [ ] Attempt automatic surface recreation after recovery
- [ ] Provide callback for surfaces that need window handle renewal
- [ ] Maintain backward compatibility with manual recreation

## Dependencies

- GUP-048: Context Error Recovery (completed)

## Technical Notes

- Store surface configs in HashMap<SurfaceId, SurfaceConfig>
- Can't store window handles (lifetime issues)
- Need application to provide window handle renewal callback
- Automatic recreation best-effort; fallback to manual if callback not set
