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

## Retrospective

**Completed**: 2026-02-27

### Key Technical Learnings

#### TOML Parsing in Bash

- **Challenge**: Parsing a TOML file with nested arrays of tables (`[[group]]`)
  from pure bash without external dependencies like `yq` or `tomlq`.
- **Solution**: Used `awk` with a two-pass approach — first pass extracts
  group-specific values, second pass falls back to `[defaults]` section.
- **Pattern**: For CI scripts that need to read config files, awk-based parsing
  of simple TOML is sufficient. For more complex TOML, consider requiring Python
  in the CI environment.

#### Filtering Compiler Noise from Test Output

- **Challenge**: Test output captured for reports contained hundreds of lines of
  compiler warnings (from gup-macros dead code), drowning out the actual test
  results.
- **Solution**: Used `grep -E '(^test |^running |^ok |^FAILED|^test result)'` to
  extract only test framework output lines.
- **Pattern**: Always filter cargo test output before including it in reports.
  Compiler output goes to stderr, but `2>&1` merging makes this messy.

#### JSON Format for Trend Data

- **Challenge**: Initially wrote JSON with all test results on a single line,
  which broke line-oriented tools (grep -c, grep -A1) used to extract per-test
  values.
- **Solution**: Format each test entry on its own line in the JSON output.
- **Pattern**: When writing JSON that will be parsed by both proper JSON parsers
  and line-oriented shell tools, use one-entry-per-line formatting.

### Architectural Decisions

#### Shell Scripts Over Rust Binary

- **Decision**: Implemented the alert system and trend tracking as bash scripts
  rather than a Rust binary.
- **Reasoning**: CI scripts need to be lightweight, fast to iterate on, and easy
  to modify without recompilation. The threshold config parsing is simple enough
  for awk. The existing `benchmark_baseline.sh` set the precedent.
- **Trade-off**: Less type safety and harder to test than Rust. Mitigated with
  dedicated script test suite.
- **Future**: If the alert system grows more complex (e.g., statistical analysis
  of trends), consider migrating to a Rust helper binary.

#### TOML for Threshold Config

- **Decision**: Used TOML format for `perf-thresholds.toml` rather than JSON or
  YAML.
- **Reasoning**: Consistent with the Rust ecosystem (Cargo.toml). Human-
  readable comments. Simple enough to parse with awk.
- **Trade-off**: Slightly more complex bash parsing than JSON. But TOML's
  comment support is valuable for documenting threshold choices.

#### Update-in-Place PR Comments

- **Decision**: PR performance comments update existing bot comments rather than
  creating new ones.
- **Reasoning**: Repeated pushes to a PR would otherwise create a wall of
  performance comments. Updating in-place keeps the conversation clean.
- **Trade-off**: Loses history of intermediate results (only latest is visible).
  Mitigated by artifact uploads which preserve all reports.

### Development Workflow Insights

- The story was primarily infrastructure/CI work rather than Rust code changes,
  which made the usual `cargo test` / `mask all-fix` loop less central. Most
  validation was done by running the scripts directly.
- The pre-existing 3 test failures in `mark::renderer::tests` are unrelated to
  this story and should be addressed separately.
- Upgrading `actions/upload-artifact` and `actions/cache` from v3 to v4 was a
  natural improvement while touching the workflow file.

### Follow-up Stories

1. **GUP-232: Fix Pre-existing Mark Renderer Metric Test Failures** — 3 tests in
   `mark::renderer::tests` fail consistently (`test_non_tracked_render_*`,
   `test_render_marks_tracked_*`). These appear to be related to draw call
   metric tracking and should be investigated separately.
