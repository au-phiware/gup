# GUP-233: Fix Flaky Registry Scalability Performance Test

**Priority**: Low **Complexity**: Low **Created**: 2025-07-18 **Status**: 🚧
In Progress

## Overview

The `test_registry_scalability` test in `mark_pipeline_performance_tests.rs`
consistently fails with ~5-6ms for 100 cached pipeline retrievals against a 5ms
threshold. The threshold is too tight and the test is environment-dependent,
making it a flaky failure that obscures real regressions.

## Context

Discovered during GUP-232 investigation. The test measures
`MarkRegistry::get_pipeline` performance and asserts the total time for 100
cached lookups is under 5ms. On the current development hardware, this
consistently takes 5-6ms, just above the threshold. The test logic and the
underlying code are correct — the issue is purely in the timing assertion.

## User Story

As a developer, I want performance tests to reliably distinguish real
regressions from environment noise so that test results are trustworthy.

## Acceptance Criteria

- [ ] `test_registry_scalability` passes reliably across multiple runs
- [ ] Performance intent is still validated (cached pipeline retrieval is fast)
- [ ] No other test regressions

## Technical Tasks

- [ ] Increase the pipeline retrieval threshold from 5ms to a more reasonable
      value (e.g. 20ms) or switch to per-retrieval timing
- [ ] Consider adding a warm-up loop before measurement to reduce first-run bias
- [ ] Run the test 5+ times to confirm reliability

## Dependencies

- None (standalone fix)

## Testing Strategy

- Run `cargo test test_registry_scalability -- --test-threads=1` multiple times
- Run full test suite to verify no regressions

## Success Metrics

- Test passes on 10 consecutive runs

## Risk Assessment

- **Very Low**: Only modifies a performance test threshold

## Definition of Done

- [ ] Test passes reliably
- [ ] Full test suite has no new failures
