# GUP-305: Release-Mode Performance Validation CI Job

## Story Overview

**Title**: Release-Mode Performance Validation CI Job **Priority**: Medium
**Story Points**: 3 **Status**: 💡 New

## Context

GUP-014 established comprehensive performance validation tests with
profile-aware targets. In debug mode, thresholds are relaxed (e.g. 100ms frame
budget vs 16.67ms) to allow CI to pass. However, the actual Phase 1 targets
(100K points at 60 FPS, <1ms interaction) can only be validated in release
builds.

A dedicated CI job running `cargo test --release` for the performance validation
suite would catch regressions that debug-mode tests miss.

## User Story

**As a** Gup developer **I want** a CI job that validates Phase 1 performance
targets in release mode **So that** I can be confident the library meets its
performance promises on every commit

## Acceptance Criteria

- [ ] CI pipeline includes a release-mode performance validation step
- [ ] `PerformanceTargets::phase1()` thresholds are met in release builds
- [ ] Release performance results are recorded as baselines
- [ ] Regressions are reported with clear delta information

## Technical Tasks

- [ ] Add a CI workflow step:
      `cargo test --release --test performance_validation_tests -- --test-threads=1`
- [ ] Configure release-mode baseline storage
- [ ] Add opt-in large-dataset tests (`--ignored`) for thorough validation
- [ ] Document the release performance validation workflow

## Dependencies

### Prerequisite Stories

- GUP-014 ✅ — Performance Validation and Optimization

## Testing Strategy

- Run the existing `performance_validation_tests` in release mode
- Verify Phase 1 targets pass without relaxation
- Compare debug vs release results

## Definition of Done

- [ ] Release-mode CI step added and green
- [ ] Phase 1 targets validated in release builds
- [ ] Baseline storage configured for release profile
- [ ] Documentation updated
