# GUP-187: Flaky Label Performance Test Fix

**Status**: ✅ Complete (2026-02-26) **Priority**: Low **Category**: Testing /
Quality **Estimated Effort**: 0.5 days **Dependencies**: None

## Overview

The `label::positioner::tests::test_performance_500_labels` test has an overly
tight 10ms target that fails intermittently under system load. It currently
asserts that positioning 500 labels completes in under 10ms, but timing tests in
debug builds are unreliable — observed failures show 12ms which is well within
acceptable variation.

## Context

Discovered during GUP-070 (Mark Performance Optimization) when running the full
test suite. The test failed 1 out of ~5 runs, always with the same assertion:
`500 labels took 12ms (target < 10ms)`.

## User Story

**As a** developer running the test suite **I want** all tests to pass reliably
**So that** CI stays green and flaky failures don't mask real regressions

## Acceptance Criteria

- [x] The label performance test does not fail intermittently
- [x] Performance expectations are still validated (not removed entirely)
- [x] Test works reliably in both debug and release profiles

## Technical Tasks

1. Increase the timing threshold to a generous 50ms (debug builds are slow)
2. Alternatively, convert to a benchmark-only check using criterion
3. Add `#[cfg(not(debug_assertions))]` gate for tight timing assertions

## Testing Strategy

- Run the test 100 times in a loop to verify no flaky failures
- Validate that actual performance regressions (e.g. 500ms) would still be
  caught

## Risk Assessment

- **Low risk**: Only changes a timing threshold in a test assertion

## Definition of Done

- [x] Test passes reliably across 100+ consecutive runs
- [x] Performance regression detection is maintained
- [x] No other tests affected

## Implementation Summary

### What Was Implemented

Used `cfg(debug_assertions)` to apply profile-aware performance thresholds:

- **Debug builds**: 50ms threshold (generous, avoids flaky failures)
- **Release builds**: 10ms threshold (tight, catches real regressions)

### Key Files Changed

- `src/label/positioner.rs` — Updated `test_performance_500_labels` with
  conditional thresholds

### Test Results

- 100/100 consecutive test runs passed in debug mode
- 1227 unit tests pass, 0 failures
- All examples compile cleanly

## Retrospective

**Completed**: 2026-02-26

### Key Technical Learnings

#### Profile-Aware Test Thresholds

- **Challenge**: Debug builds run significantly slower than release builds,
  making fixed timing thresholds unreliable — 10ms is generous for release but
  marginal for debug.
- **Solution**: Use `#[cfg(debug_assertions)]` to select thresholds at compile
  time: 50ms for debug, 10ms for release.
- **Pattern**: For any timing-sensitive test, always use profile-conditional
  thresholds rather than a single fixed value. This is more robust than just
  increasing the threshold because it still catches regressions in release
  builds.

### Architectural Decisions

#### Conditional Compilation Over Single Generous Threshold

- **Decision**: Use `cfg(debug_assertions)` with two thresholds instead of a
  single increased threshold.
- **Reasoning**: A single 50ms threshold would still catch gross regressions but
  would miss a 5x slowdown in release builds. Dual thresholds give the best of
  both worlds.
- **Trade-off**: Slightly more verbose test code, but much better regression
  detection fidelity.
- **Future**: This pattern should be applied to the other timing-sensitive tests
  identified in GUP-174.

### Development Workflow Insights

- The fix was surgical — a single file, 14 lines changed, 5 removed. Small
  stories like this are excellent for building confidence in the test suite.
- Running the test 100 times in a loop is a reliable way to validate flaky test
  fixes. All 100 runs completed in ~2 minutes total (using cached build).
- The codebase has 4 other timing-sensitive assertions that could benefit from
  the same pattern (tracked by GUP-174).

### Follow-up Stories

GUP-174 (Flaky Performance Test Stabilization) remains the broader follow-up
that would audit and fix the remaining timing-sensitive tests across the
codebase.
