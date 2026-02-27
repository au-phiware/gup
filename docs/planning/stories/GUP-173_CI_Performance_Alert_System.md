# GUP-173: CI Performance Alert System

**Priority**: Low **Complexity**: Medium **Created**: 2025-08-06 **Status**: ✅
Complete (2026-02-27)

## Overview

Implement an automated alerting system that detects performance regressions in
CI/CD pipelines. When benchmark results exceed configured thresholds, the system
should fail the build and provide actionable diagnostics.

## Context

GUP-077 established threshold-based regression tests (`mask perf-check`) and
criterion benchmarks with baseline comparison (`benchmark_baseline.sh`). This
story connects these tools to CI/CD infrastructure for automated monitoring.

## User Story

As a contributor, I want to be notified automatically when my changes cause
performance regressions, so that I can fix them before merging.

## Acceptance Criteria

- [x] CI pipeline runs `mask perf-check` on every PR
- [x] Criterion baselines are saved per branch for comparison
- [x] Performance regression reports attached to PR comments
- [x] Configurable threshold percentages per benchmark group
- [x] Historical performance trend tracking

## Technical Tasks

- [x] Create GitHub Actions workflow for performance checks
- [x] Implement baseline management per git branch
- [x] Create PR comment formatter for benchmark comparison results
- [x] Add configurable threshold configuration file
- [x] Set up benchmark result storage for trend analysis

## Dependencies

- **Requires**: GUP-077 (benchmark suite and regression tests)
- **Related**: GUP-082 (Debug Tool Integration with CI/CD)

## Testing Strategy

- Validate CI workflow with intentional performance regression
- Verify alerting triggers correctly on threshold violation

## Risk Assessment

- **Low**: CI configuration complexity
- **Medium**: GPU availability in CI runners (may need self-hosted)

## Definition of Done

- [x] CI workflow configured and passing
- [x] Regression detection working on test PR
- [x] Documentation for threshold configuration

## Implementation Summary

### What Was Implemented

1. **Configurable threshold configuration** (`perf-thresholds.toml`): Defines
   per-group regression/warning/improvement thresholds for 11 benchmark groups,
   with fallback defaults.

2. **Performance alert script** (`scripts/perf_alert.sh`): Reads the threshold
   config, runs `mask perf-check` and criterion benchmarks, compares results
   against configurable per-group thresholds, and generates both Markdown and
   JSON reports.

3. **Performance trend tracking** (`scripts/perf_trend.sh`): Records benchmark
   data points over time, generates trend reports with per-test timing trends,
   supports cleanup of old data.

4. **Updated CI workflow** (`.github/workflows/performance.yml`):
   - Added `perf_check` job that runs `mask perf-check` on every PR
   - Integrated `perf_alert.sh` for unified regression detection
   - Per-branch criterion baseline caching via `actions/cache@v4`
   - PR comments update in-place (no spam)
   - Trend recording on main branch pushes

5. **Maskfile commands**: `mask perf-alert`, `mask perf-trend-record`,
   `mask perf-trend-report`

6. **Documentation** (`docs/CI_PERFORMANCE_ALERTS.md`): Comprehensive guide
   covering configuration, script usage, CI jobs, and baseline management.

7. **Script tests** (`scripts/test_perf_scripts.sh`): 10 tests validating config
   parsing, report generation, and trend script behaviour.

### Key Files Changed

| File                                | Change                          |
| ----------------------------------- | ------------------------------- |
| `perf-thresholds.toml`              | New — threshold configuration   |
| `scripts/perf_alert.sh`             | New — CI alert script           |
| `scripts/perf_trend.sh`             | New — trend tracking            |
| `scripts/test_perf_scripts.sh`      | New — script tests              |
| `.github/workflows/performance.yml` | Updated — unified workflow      |
| `maskfile.md`                       | Updated — new commands          |
| `docs/CI_PERFORMANCE_ALERTS.md`     | New — documentation             |
| `.gitignore`                        | Updated — benchmark history dir |

### Test Results

- 10/10 script tests pass
- 1743/1746 Rust tests pass (3 pre-existing failures in mark renderer metrics)
- All examples compile
- `mask all-fix` clean
