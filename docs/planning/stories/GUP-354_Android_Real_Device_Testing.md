# GUP-354: Android Real Device Testing

## Story Overview

**Initiative**: Mobile **Status**: 📋 Planned **Created**: 2025-07-25

## Context

GUP-271 set up CI with an Android emulator using software Vulkan
(`swiftshader_indirect`). Real Android devices have wildly different GPU
drivers (Adreno, Mali, PowerVR, Xclipse) with varying levels of Vulkan
conformance. This story establishes the real-device testing workflow,
performance baselines, and a GPU vendor compatibility matrix.

This mirrors GUP-274 (iOS Real Device Testing) which does the same for the
iOS platform.

## User Story

> "As a Gup maintainer, I want documented real-device testing procedures
> and a GPU vendor compatibility matrix so that I can catch driver-specific
> rendering bugs before users do."

## Acceptance Criteria

- [ ] A documented workflow for deploying the example APK to a physical
      Android device and capturing logcat output.
- [ ] Performance baselines (frame time, memory usage) on at least two
      physical devices with different GPU vendors.
- [ ] A GPU vendor compatibility matrix documenting tested devices, API
      levels, and any known issues.
- [ ] The GLES fallback path is tested on a device without Vulkan support
      (or with Vulkan disabled).
- [ ] `examples/android/README.md` updated with real-device testing
      instructions.

## Dependencies

### Prerequisite Stories

- GUP-271: Android Platform Support ✅ — provides the Android integration.
- GUP-353: Android Chart Rendering Integration 📋 — need visible rendering
  to measure performance.

## Testing Strategy

- Manual testing on physical devices.
- Performance profiling with Android GPU Inspector or systrace.
- Stress testing with screen rotations and background/foreground cycles.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] Compatibility matrix published in docs/
- [ ] Story status updated to ✅ Complete
