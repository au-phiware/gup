# GUP-187: Flaky Label Performance Test Fix

**Status**: ✅ Complete (2025-08-10) **Priority**: Low **Category**: Testing /
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
