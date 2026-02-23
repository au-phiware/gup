# Pattern Benchmark CI Integration

**Story**: GUP-162  
**Status**: ✅ Complete  
**Created**: 2025-02-24

## Overview

This document describes the CI/CD integration for pattern rendering benchmarks.
Pattern benchmarks run automatically on pull requests and main branch commits to
detect performance regressions early.

## Architecture

### CI Workflow Jobs

The performance CI workflow (`.github/workflows/performance.yml`) includes:

#### 1. `pattern_benchmarks_pr` - Fast PR Feedback

**Trigger**: All pull requests  
**Purpose**: Quick pattern benchmark comparison against main baseline  
**Duration**: ~15-20 minutes

**What it does**:

- Runs pattern benchmarks with Criterion
- Compares against main branch baseline
- Detects >10% performance regressions
- Posts results as PR comment
- Fails CI if critical regression detected

**Output**:

- PR comment with benchmark summary
- Artifact: `pattern-benchmarks-pr-{PR#}` with full results
- Exit code 1 if regression detected (blocks merge)

#### 2. `benchmark` - Comprehensive Main Branch Benchmarks

**Trigger**: Push to main branch  
**Purpose**: Update baselines and track performance trends  
**Duration**: ~40-45 minutes

**What it does**:

- Runs ALL benchmarks (not just patterns)
- Saves results as new 'main' baseline
- Caches baselines for PR comparisons
- Archives results for trend analysis
- Commits to `benchmark-history` branch

**Output**:

- Artifact: `benchmark-results-{SHA}` (retained 90 days)
- Updated baseline cache for future PRs
- Benchmark history commit

### Baseline Management

#### Automatic Baselines

- **Main baseline**: Updated on every main branch push
- **PR baselines**: Each PR gets named baseline (`pr-{PR#}`)
- **Cache strategy**: Main baselines cached and restored for PR comparisons

#### Manual Baseline Operations

Use `scripts/benchmark_baseline.sh` for manual baseline management:

```bash
# Save current results as baseline
./scripts/benchmark_baseline.sh save main

# Save only pattern benchmarks
./scripts/benchmark_baseline.sh save-pattern feature-xyz

# Compare against baseline
./scripts/benchmark_baseline.sh compare main

# List all baselines
./scripts/benchmark_baseline.sh list

# Delete old baseline
./scripts/benchmark_baseline.sh delete old-baseline

# Clear all baselines
./scripts/benchmark_baseline.sh reset
```

### Regression Detection

#### Detection Criteria

A regression is detected when:

- Criterion reports "Performance has regressed"
- OR benchmark shows >10% increase compared to baseline

#### Criterion Output Parsing

The CI workflow parses Criterion's text output for regression indicators:

- Looks for "Performance has regressed" message
- Checks for percentage changes >10% (e.g., `change: +15.2%`)

#### Regression Response

When regression detected:

1. CI step continues (doesn't fail immediately)
2. Regression flag set in workflow outputs
3. PR comment includes ⚠️ warning
4. Final step fails with exit code 1 (blocks merge)

This allows the full benchmark suite to complete and generate reports even when
regressions are found.

### PR Comments

#### Comment Format

````text
## 🎨 Pattern Benchmark Results

**PR #123** - Pattern rendering performance analysis

### Summary

Pattern benchmarks compare rendering performance for different pattern types
(Solid, Dots, Lines, Crosshatch) across various data sizes.

**Target**: <5ms overhead for 100K points with patterns

### Results

```text
[Criterion benchmark output excerpt]
````

### ⚠️ Performance Regression Detected

One or more benchmarks show >10% performance degradation compared to the main
branch baseline.

**Action Required**: Review the changes and optimize or document the performance
impact.

---

**Note**: Benchmarks run on software rendering (no GPU). Results may differ on
real hardware.

📊 Full benchmark results available in workflow artifacts.

````

#### Comment Behavior

- **First run**: Creates new comment
- **Subsequent runs**: Updates existing comment (finds comment by marker text)
- **Always posts**: Even on failure, so results are visible

## Performance Targets

### Pattern Benchmarks (<5ms Overhead)

Pattern rendering must add <5ms overhead compared to solid fill rendering:

- **100K points, Solid**: Baseline reference time
- **100K points, Dots**: Baseline + <5ms
- **100K points, Lines**: Baseline + <5ms
- **100K points, Crosshatch**: Baseline + <5ms

### Regression Threshold (>10% Degradation)

CI fails if any benchmark shows >10% slowdown:

- **Example**: If baseline is 10ms, >11ms fails CI
- **Rationale**: 10% allows for measurement noise while catching real issues

## Usage Guide

### For PR Authors

1. **Push changes**: Benchmarks run automatically
2. **Check PR comment**: Review benchmark results
3. **If regression detected**:
   - Review changed code for performance issues
   - Optimize if possible
   - If intentional, document reason in PR description
   - Request reviewer approval for acceptable regression

### For Reviewers

1. **Check PR comment**: Look for ⚠️ regression warning
2. **Review artifacts**: Download full results if needed
3. **Evaluate impact**: Is regression acceptable?
4. **Decision**:
   - Minor (<10%): Usually acceptable
   - Major (>10%): Needs justification or optimization
   - Critical (>20%): Should not merge without significant benefit

### For Maintainers

#### Updating Baselines

After major changes (e.g., wgpu upgrade, hardware change):

```bash
# Clear old baselines
./scripts/benchmark_baseline.sh reset

# Run benchmarks to establish new baseline
cargo bench --all-features -- --save-baseline main

# Commit new baseline
git add target/criterion
git commit -m "chore: reset benchmark baselines after [change]"
````

#### Adjusting Thresholds

Edit `.github/workflows/performance.yml`:

```yaml
# Change regression detection pattern
if grep -q "change:.*+[1-9][0-9]\{1,\}%" benchmark_comparison.txt; then
# Current: detects +10% or more
# For +20%: "change:.*+[12][0-9]\{1,\}%"
# For +5%:  "change:.*+[5-9]%\|+[1-9][0-9]%"
```

#### Troubleshooting

**Benchmarks fail with "no GPU"**:

- Expected on CI runners (use software rendering)
- Validates CPU overhead, not GPU performance

**Cache miss (no baseline found)**:

- First PR after main branch update may miss cache
- Benchmarks still run, but no comparison available
- Wait for main branch benchmark to complete

**False positives**:

- Measurement noise can cause sporadic failures
- Re-run benchmark to confirm
- Consider increasing threshold to 15% if frequent

## Implementation Details

### Files Changed

- `.github/workflows/performance.yml`: Added `pattern_benchmarks_pr` job
- `scripts/benchmark_baseline.sh`: NEW - Manual baseline management
- `docs/CI_BENCHMARK_INTEGRATION.md`: NEW - This documentation

### Key Design Decisions

#### Why Separate PR Job

- **Speed**: Run only pattern benchmarks (~15min) vs all benchmarks (~45min)
- **Focus**: Pattern-specific feedback more relevant to pattern changes
- **Cost**: Reduce CI resource usage

#### Why Block on Regression

- **Prevention**: Stop regressions before merge
- **Awareness**: Forces explicit discussion of performance trade-offs
- **Documentation**: Regression justification becomes part of PR history

#### Why Update Comments

- **Clean**: Avoid comment spam on multiple pushes
- **Latest**: Always see most recent results
- **History**: Old results still in artifacts

### Criterion Integration

#### Baseline Storage

Criterion stores baselines in `target/criterion/{benchmark_group}/{baseline}/`:

- `base/`: Default baseline (unnamed)
- `main/`: Main branch baseline
- `pr-123/`: PR-specific baselines

#### Comparison Format

Criterion output includes:

```text
                        time:   [8.2534 ms 8.2891 ms 8.3273 ms]
                        change: [-1.2345% -0.8234% -0.4123%] (p = 0.03 < 0.05)
                        Performance has improved.
```

CI parses this format to detect:

- Positive percentage = regression (slower)
- "Performance has regressed" message
- Statistical significance (p < 0.05)

## Future Enhancements

### GPU-Capable Runners (GUP-154)

When self-hosted GPU runners available:

- Update matrix to include `self-hosted-nvidia-gpu`
- Run pattern benchmarks on real GPU hardware
- Compare software vs hardware rendering performance

### Trend Visualization (GUP-152)

Integrate with performance trend visualization:

- Track pattern benchmark trends over time
- Identify gradual performance degradation
- Correlate with commits/features

### Adaptive Thresholds

Implement context-aware regression detection:

- Different thresholds per benchmark group
- Account for measurement noise per environment
- Dynamic thresholds based on historical variance

## References

- **Story**:
  [GUP-162](../planning/stories/GUP-162_Pattern_Benchmark_CI_Integration.md)
- **Prerequisites**:
  [GUP-156](../planning/stories/GUP-156_Pattern_Performance_Benchmarking.md)
- **Benchmark Docs**:
  [Pattern Performance Benchmarking](PATTERN_PERFORMANCE_BENCHMARKING.md)
- **Criterion Guide**: <https://bheisler.github.io/criterion.rs/book/>
