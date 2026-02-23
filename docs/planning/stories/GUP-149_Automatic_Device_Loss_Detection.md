# GUP-149: Automatic Device Loss Detection

**Status**: ✅ Complete (2025-02-22)

## Story Overview

**Title**: Automatic Device Loss Detection **Epic**: Phase 1 Initiative 1 - Core
GPU Primitives and Selection API **Priority**: Low **Story Points**: 2

## Context

Currently, applications must manually call `mark_device_lost()` when GPU
operations fail. This requires awareness of error handling and explicit recovery
triggering. Automatic detection would wrap queue operations to detect failures
and trigger recovery automatically.

## User Story

**As a** Gup application developer **I want** GPU device loss to be detected
automatically **So that** I don't need to manually monitor for failures and
trigger recovery

## Acceptance Criteria

- [x] Wrap queue.submit() to detect operation failures
- [x] Automatically call mark_device_lost() on detected failures
- [x] Maintain backward compatibility with manual detection
- [x] Add configuration option to enable/disable automatic detection

## Dependencies

- GUP-048: Context Error Recovery (completed)

## Technical Notes

- Wrap wgpu queue operations with error detection
- Check for specific error types that indicate device loss
- Default to automatic detection enabled
- Provide opt-out for applications that want manual control

## Implementation Summary

**Completed**: 2025-02-22

### Files Changed

- `src/context.rs`: Added automatic detection configuration and surface error wrapping
- `tests/automatic_device_loss_detection_tests.rs`: Comprehensive test suite (8 tests)

### Key Features Implemented

1. **Configuration Option** - Added `automatic_device_loss_detection: bool` to `GupOptions` (default: `true`)
2. **Surface Error Detection** - Wrapped `get_current_texture()` calls in `begin_frame()` and `begin_frame_for()` methods
3. **Automatic Recovery Triggering** - Automatically calls `mark_device_lost()` when surface errors occur
4. **Backward Compatibility** - Manual detection via `mark_device_lost()` still works when automatic detection is disabled
5. **Zero Runtime Overhead** - Only checks configuration flag when errors occur (not on success path)

### Technical Approach

The implementation focuses on surface acquisition errors (from `get_current_texture()`) as the primary failure point:

- Surface errors like `Lost`, `Outdated`, or `Timeout` typically indicate GPU issues
- The `begin_frame()` and `begin_frame_for()` methods already have mutable access to context
- When an error occurs and automatic detection is enabled, `mark_device_lost()` is called before returning the error
- The application still receives the error and can handle it, but the recovery system is now aware

### Test Coverage

8 comprehensive tests covering:

- Default configuration (enabled by default)
- Ability to disable automatic detection
- Configuration storage and persistence
- Backward compatibility with manual detection
- Integration with existing recovery system
- Behavior through recovery cycles

All tests pass with 100% success rate.
