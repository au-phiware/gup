# GUP-174: Flaky Performance Test Stabilization

**Priority**: Low **Complexity**: Low **Created**: 2025-08-06 **Status**: 🚧 In
Progress

## Overview

Review and stabilize timing-sensitive performance tests across the codebase that
fail intermittently in debug builds due to tight thresholds or system load
variability.

## Context

During GUP-077 implementation, the pre-existing
`label::positioner::tests::test_performance_500_labels` test was observed
failing with 12ms vs 10ms threshold — a marginal timing issue in debug builds.
Similar flaky patterns may exist in other performance tests.

## User Story

As a developer, I want all tests to pass reliably so that test failures indicate
real bugs rather than environmental noise.

## Acceptance Criteria

- [ ] Audit all performance-threshold tests in the codebase
- [ ] Adjust thresholds to account for debug-build overhead
- [ ] Add documentation noting which tests have timing sensitivity
- [ ] Ensure `cargo test -- --test-threads=1` passes reliably in debug mode

## Technical Tasks

- [ ] Grep for timing assertions across all test files
- [ ] Categorize by current reliability (always pass, sometimes fail, often
      fail)
- [ ] Adjust thresholds or add `#[ignore]` annotations with comments
- [ ] Consider environment-detection for adaptive thresholds

## Dependencies

- **Related**: GUP-077 (established performance testing patterns)

## Testing Strategy

- Run full test suite 10 times in debug mode to verify reliability
- Confirm no regressions in test coverage

## Risk Assessment

- **Low**: Simple threshold adjustments

## Definition of Done

- [ ] All performance tests pass reliably in debug builds
- [ ] Thresholds documented with rationale
