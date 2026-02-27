# CI Performance Alert System

Automated performance regression detection for the Gup CI/CD pipeline.

## Overview

The alert system runs on every pull request and detects performance regressions
by comparing against configurable thresholds defined in
[`perf-thresholds.toml`](/perf-thresholds.toml). When a benchmark group exceeds
its threshold, the CI build fails and a detailed report is posted as a PR
comment.

## Components

### Threshold Configuration (`perf-thresholds.toml`)

Defines regression and warning thresholds per benchmark group:

```toml
[defaults]
max_regression_percent = 15.0   # Fail CI if regression exceeds this
warning_percent = 10.0          # Warn (don't fail) above this
min_improvement_percent = 5.0   # Report notable improvements

[[group]]
name = "interaction"
description = "Interaction system (point/region/batch queries)"
max_regression_percent = 20.0
warning_percent = 10.0
min_improvement_percent = 5.0
```

Group names match benchmark and test categories. If a benchmark doesn't match
any group, the `[defaults]` section applies.

### Performance Alert Script (`scripts/perf_alert.sh`)

Reads the threshold config, runs tests and benchmarks, and generates reports.

```bash
# Quick check (threshold tests only, skip criterion benchmarks)
./scripts/perf_alert.sh --skip-benchmarks

# Full check with criterion baseline comparison
./scripts/perf_alert.sh --baseline main

# CI mode (fails on regression)
./scripts/perf_alert.sh --fail-on-regression --baseline main

# Save current results as a baseline
./scripts/perf_alert.sh --save-baseline feature-branch --skip-threshold-tests
```

Outputs:

- `performance_report.md` — Markdown report (attached to PR comments)
- `performance_report.json` — Machine-readable results

### Trend Tracking (`scripts/perf_trend.sh`)

Records performance data points over time for historical analysis.

```bash
# Record current benchmark timings
./scripts/perf_trend.sh record

# View trend report (last 10 runs)
./scripts/perf_trend.sh report

# List all data points
./scripts/perf_trend.sh list

# Keep only last 50 data points
./scripts/perf_trend.sh clean 50
```

### Mask Commands

```bash
mask perf-check          # Run threshold tests (CI-friendly)
mask perf-alert          # Run full alert system with report
mask perf-trend-record   # Record a data point
mask perf-trend-report   # Generate trend report
```

## CI Workflow

The GitHub Actions workflow (`.github/workflows/performance.yml`) includes:

| Job                         | Trigger                 | Purpose                                |
| --------------------------- | ----------------------- | -------------------------------------- |
| `perf_check`                | Every PR                | Runs `mask perf-check` threshold tests |
| `performance`               | Every PR + push to main | Runs CI test suite + `perf_alert.sh`   |
| `pattern_benchmarks_pr`     | PR only                 | Pattern-specific criterion benchmarks  |
| `axis_performance`          | Every PR + push         | Axis rendering performance             |
| `benchmark`                 | Push to main only       | Full criterion suite, saves baselines  |
| `cross_platform_comparison` | Manual                  | Multi-platform comparison              |

### Baseline Management

- Criterion baselines are cached **per branch** using `actions/cache@v4`
- The `benchmark` job saves baselines as `main` on every push to main
- PR jobs restore the `main` baseline and compare against it
- Historical data is committed to a `benchmark-history` branch

### PR Comments

Performance reports are posted as PR comments. Subsequent pushes to the same PR
**update** the existing comment instead of creating new ones, keeping the
conversation clean.

## Customising Thresholds

1. Edit [`perf-thresholds.toml`](/perf-thresholds.toml)
2. Add or modify `[[group]]` sections
3. Set `max_regression_percent` for the CI failure threshold
4. Set `warning_percent` for the warning (no-fail) threshold
5. Commit the changes

Groups with more variable timings (e.g., transpilation, spatial index) can have
higher thresholds. Core rendering paths should keep tighter thresholds.
