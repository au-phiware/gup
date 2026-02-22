# CI/CD Performance Testing

This directory contains GitHub Actions workflows for automated performance testing and regression detection.

## Workflows

### Performance Testing (`performance.yml`)

Runs on every PR and push to `main` to detect performance regressions.

#### Features

- **Automated Regression Detection**: Compares performance against stored baselines
- **PR Comments**: Posts detailed performance reports as PR comments
- **Artifact Storage**: Stores performance reports for 30 days
- **Configurable Thresholds**: Default 20% increase triggers regression warnings
- **Severity Levels**: Critical, High, Medium, Low regression classification

#### Performance Report Contents

- Test execution times
- Memory usage
- Comparison against baselines
- Regression detection status
- Historical trend data

### Benchmark Suite (`performance.yml` - benchmark job)

Runs comprehensive benchmarks on commits to `main` branch.

#### Features

- **Comprehensive Benchmarking**: Runs all Criterion benchmarks
- **Long-term Storage**: Keeps benchmark results for 90 days
- **Trend Analysis**: Maintains historical benchmark data

## Usage

### Running Performance Tests Locally

```bash
# Run the performance test suite
cargo test --test performance_ci_tests -- --test-threads=1

# Update baselines after confirming changes are intentional
UPDATE_BASELINES=1 cargo test --test performance_ci_tests -- --test-threads=1
```

### Creating New Performance Tests

Add tests to `tests/performance_ci_tests.rs`:

```rust
async fn test_my_feature(_ctx: &mut GpuDebugContext) -> gup::GupResult<PerformanceSnapshot> {
    let start = std::time::Instant::now();
    
    // Your test code here
    my_feature_to_test();
    
    let elapsed = start.elapsed();
    
    Ok(PerformanceSnapshot::new(
        elapsed.as_secs_f32() * 1000.0,
        memory_usage_bytes,
    ))
}

// Add to test suite
let test_suite = PerformanceTestSuite::new("My Test Suite")
    .add_test("my_feature", "category", |ctx| {
        Box::pin(test_my_feature(ctx))
    });
```

### Baseline Management

Baselines are stored in `baselines/performance/` and version-controlled.

#### Updating Baselines

When performance changes are intentional (e.g., optimizations), update baselines:

1. Review the performance report to confirm changes are expected
2. Run locally: `UPDATE_BASELINES=1 cargo test --test performance_ci_tests`
3. Commit the updated baseline files
4. Include rationale in commit message

#### Baseline File Structure

```
baselines/performance/
├── rendering/
│   ├── basic_rendering.json
│   └── large_dataset_rendering.json
├── compilation/
│   └── shader_compilation.json
└── gpu_transfer/
    └── buffer_upload.json
```

### Configuration

Adjust CI configuration in tests:

```rust
fn create_ci_config() -> CiConfig {
    CiConfig {
        baseline_dir: PathBuf::from("baselines/performance"),
        fail_on_regression: true,  // Fail CI on regressions
        max_suite_duration_secs: 300,  // 5-minute timeout
        thresholds: PerformanceThresholds {
            regression_threshold_percent: 20.0,  // 20% increase threshold
            ..Default::default()
        },
    }
}
```

## Performance Thresholds

Default thresholds:

- **Frame Time**: 16.67ms (60 FPS target)
- **Query Time**: 1000μs (1ms interaction target)
- **Memory Usage**: 1GB limit
- **Regression Threshold**: 20% increase

### Severity Levels

- **Low**: < 20% increase (warning only)
- **Medium**: 20-40% increase (fails CI if `fail_on_regression: true`)
- **High**: 40-60% increase (requires investigation)
- **Critical**: > 60% increase (immediate attention required)

## Interpreting Results

### Successful Run

```
✅ No performance regressions detected

📈 Individual Test Results:
  ✅ basic_rendering - 5.07ms (1024KB)
  ✅ large_dataset_rendering - 15.04ms (10240KB)
  ✅ shader_compilation - 8.07ms (512KB)
  ✅ buffer_upload - 3.12ms (5120KB)
```

### Regression Detected

```
⚠️ Performance Regressions Detected:

| Test | Severity | Frame Time Δ | Memory Δ |
|------|----------|--------------|----------|
| large_dataset_rendering | High | +45.2% | +12.3% |
```

**Action Items:**

1. Review what changed in the code
2. Determine if the regression is justified
3. If justified, update baselines and document why
4. If not, investigate and fix the performance issue

## Artifacts

Performance reports are uploaded as CI artifacts:

- **performance-report-{sha}.json**: Machine-readable report
- **performance-report-{sha}.md**: Human-readable Markdown report
- **benchmark-results-{sha}/**: Criterion benchmark results

Access artifacts from the Actions tab in GitHub.

## Best Practices

1. **Run tests locally before pushing**: Catch regressions early
2. **Use meaningful test names**: Easy to identify in reports
3. **Group related tests**: Use categories to organize tests
4. **Keep tests fast**: Target < 5 minutes for full suite
5. **Document baseline updates**: Always explain why performance changed
6. **Monitor trends**: Look at historical data for patterns

## Troubleshooting

### Tests Failing Due to System Load

GPU tests can be sensitive to system load. If tests fail intermittently:

1. Re-run the workflow
2. Check if other processes were consuming GPU resources
3. Consider adjusting thresholds slightly

### Baselines Out of Sync

If baselines seem wrong:

1. Check when they were last updated
2. Review recent changes that might have affected performance
3. Re-establish baselines on a clean system

### CI Workflow Not Triggering

Ensure:

1. Workflow file is in `.github/workflows/`
2. YAML syntax is valid
3. Branch protection rules aren't blocking

## Future Enhancements

Potential additions (see follow-up stories):

- WebGPU timestamp query integration (GUP-080)
- Advanced debug data visualization (GUP-081)
- Web-based profiling dashboard (GUP-086)
- Historical trend analysis and charts
- Automated performance regression bisection
- Integration with performance monitoring services
