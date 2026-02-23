# GUP-151: Surface Configuration Caching

**Status**: ✅ Complete (2025-02-24)

## Story Overview

**Title**: Surface Configuration Caching **Epic**: Phase 1 Initiative 1 - Core
GPU Primitives and Selection API **Priority**: Low **Story Points**: 2

## Context

After device recovery, surfaces must be recreated by the application. We
currently don't cache surface configurations (size, format, scale factor) which
forces applications to track and restore these settings. Caching configurations
would enable automatic surface recreation with the same settings.

## User Story

**As a** Gup application developer **I want** surface configurations to be
preserved during recovery **So that** surfaces are automatically restored to
their previous settings

## Acceptance Criteria

- [x] Cache surface size, format, and scale factor before loss
- [x] Attempt automatic surface recreation after recovery
- [x] Provide callback for surfaces that need window handle renewal
- [x] Maintain backward compatibility with manual recreation

## Dependencies

- GUP-048: Context Error Recovery (completed)

## Technical Notes

- Store surface configs in HashMap<SurfaceId, SurfaceConfig>
- Can't store window handles (lifetime issues)
- Need application to provide window handle renewal callback
- Automatic recreation best-effort; fallback to manual if callback not set

## Implementation Summary

### Files Changed
- `src/context.rs`: Added surface configuration caching infrastructure
- `tests/surface_configuration_caching_tests.rs`: Comprehensive test suite (9 tests)

### Key Features Implemented
1. **CachedSurfaceConfig struct** - Stores surface settings (size, format, present mode, scale factor, etc.)
2. **WindowHandle trait** - Combines HasWindowHandle + HasDisplayHandle + Send + Sync for trait object compatibility
3. **WindowHandleRenewalCallback** - Type alias for callback function that provides window handles during recovery
4. **Automatic caching** - Surface configs automatically cached on add/resize/scale factor changes
5. **cache_surface_config()** - Helper method to cache surface configuration
6. **update_cached_surface_config()** - Updates cache after surface property changes
7. **set_window_handle_renewal_callback()** - Public API to set the renewal callback
8. **Enhanced recreate_surfaces()** - Attempts automatic recreation using cached configs and callback
9. **recreate_single_surface()** - Internal method to recreate one surface with cached settings
10. **Backward compatibility** - Falls back to manual recreation if no callback is set

### Test Coverage
- 9 tests covering all acceptance criteria
- Surface config caching on add/modification
- Window handle renewal callback mechanism
- Automatic recreation during recovery
- Backward compatibility without callback
- Recovery timing verification
- Callback lifetime and ownership
- Multiple recovery attempts

All tests pass successfully.
