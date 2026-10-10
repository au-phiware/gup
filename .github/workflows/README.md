# CI/CD Performance Testing

This directory contains GitHub Actions workflows for automated performance
testing and regression detection.

## Workflows

### Performance Testing (`performance.yml`)

Runs on every PR and push to `main` to detect performance regressions. Only the
two threshold jobs (`perf_check`, `performance`) run per push; the axis and WASM
axis jobs run on pull requests, weekly and on manual dispatch. The workflow's
header comment gives the reasons (GUP-409).

#### Features

- **Automated Regression Detection**: Compares performance against stored
  baselines
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

Runs comprehensive benchmarks on `main` weekly (Monday 03:00 UTC) and on manual
dispatch. It is too long (316 Criterion benchmarks) to run on every push.

#### Features

- **Comprehensive Benchmarking**: Runs all Criterion benchmarks
- **Long-term Storage**: Keeps benchmark results and a trend data point
  (`scripts/perf_trend.sh record`) as a 90-day artifact

### Tests (`tests.yml`)

Runs `cargo test --workspace -- --test-threads=1` on lavapipe, then the lib and
every `required-features = ["debug"]` integration test with `--features debug`
(`scripts/test_debug_feature.sh`). It is the only workflow that runs the root
crate's whole suite. Wall-clock budgets stay out of it: the timing-budget
binaries the Performance workflow runs by name are `test = false`, and other
tests that assert a budget are `#[ignore]`d with a "wall-clock budget ...
(GUP-398)" reason. `mask ci tests` runs it locally.

### Lint (`lint.yml`)

Runs `mask all-check`, the full local gate, in the Nix dev shell: strict clippy
on every workspace member and target (default and all features), rustfmt,
whitespace, nixfmt/statix, prettier/mdl, mark validation, gallery sync and the
pre-commit hook's scoping tests. The local hook (`mask pre-commit`) scopes its
checks to the staged files; this workflow never does. `mask ci lint` runs it
locally.

### Visual regression (`visual-regression.yml`)

The `visual-regression` job renders the chart-builder goldens, gup-core,
gup-text and the quarantined culling/LOD crate on lavapipe and smoke-runs the
examples. The `browser` job (GUP-408) builds gup-core's `wasm-size/scatter`
harness for wasm32 and renders it in headless Chrome for Testing (a pinned
version) through WebGPU on SwiftShader, Chrome's bundled software adapter. It
fails on `GUP FAIL`, a timeout, a console error, an uncaught exception, a failed
load or a WebGPU/WGSL diagnostic; the step summary records the browser and
adapter, and the render is uploaded as the `browser-smoke` artifact. The same
job then enforces gup-core's WASM size budget (GUP-417, `scripts/wasm_size.sh`):
it fails when the scatter costs more than 400,000 B gz over bare wgpu.
`mask ci visual-regression` runs both jobs locally; `mask wasm-browser` and
`mask wasm-size` run the browser job's two checks alone.

### No workflow scopes by changed files

No workflow has a `paths:` filter, and none reads the pre-commit hook's scoping
rules: a workflow that runs runs all of its checks, whatever changed.

### Every workflow cancels its superseded runs

Every workflow has a `concurrency:` group (`<workflow>-${{ github.ref }}`) with
`cancel-in-progress: true`, so a second push to a branch cancels the first
push's runs instead of queueing beside them (GUP-409: queued jobs piling up
produced "job was not acquired by Runner" failures). Performance adds the event
name to its group so that a push never cancels the weekly benchmark.

### Workflow files are linted

`actionlint` (with shellcheck on every `run:` script) checks every file in this
directory. It runs in `mask all-check` (so in the Lint workflow) and in the
pre-commit hook whenever a workflow file is staged. A malformed workflow does
not fail a run on GitHub: the run never starts, so nothing reports it.

## Toolchain and GPU (all workflows)

- The Rust toolchain is pinned in `rust-toolchain.toml`. Workflows install it
  with `rustup toolchain install` or run in the Nix dev shell, which reads the
  same file. Never use a floating `stable` toolchain in a workflow.
- The runners have no GPU. Workflows install Mesa's lavapipe (software Vulkan),
  or set `GUP_SOFTWARE_GPU=1` for the Nix dev shell. The browser job uses
  Chrome's SwiftShader instead (`scripts/browser_smoke.mjs` forces it).
- `mask ci` (or `mask ci <workflow>`) runs the push-to-main workflows locally on
  lavapipe.

## Usage

### Running Performance Tests Locally

```bash
# Run the performance test suite
cargo test --features debug --test performance_ci_tests -- --test-threads=1

# Update baselines after confirming changes are intentional
UPDATE_BASELINES=1 cargo test --features debug --test performance_ci_tests -- --test-threads=1
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

When performance changes are intentional (e.g., optimizations), update
baselines:

1. Review the performance report to confirm changes are expected
2. Run locally:
   `UPDATE_BASELINES=1 cargo test --features debug --test performance_ci_tests`
3. Commit the updated baseline files
4. Include rationale in commit message

#### Baseline File Structure

Multi-platform baselines are organized by platform:

```text
baselines/performance/
├── default/                    # Software rendering (default)
│   ├── rendering/
│   │   ├── basic_rendering.json
│   │   └── large_dataset_rendering.json
│   └── compilation/
│       └── shader_compilation.json
├── nvidia_rtx_3080/           # NVIDIA GPU platform
│   ├── rendering/
│   │   └── ...
│   └── compilation/
│       └── ...
└── amd_rx_6800/              # AMD GPU platform
    └── ...
```

Each platform has its own baseline set to account for hardware-specific
performance characteristics.

## Multi-Platform Testing

The CI workflow supports testing across multiple GPU platforms to detect
platform-specific performance regressions.

### Platform Detection

The system automatically detects the GPU platform using wgpu adapter info:

- **NVIDIA**: RTX 3000/4000 series
- **AMD**: RX 6000/7000 series
- **Intel**: Arc A-series
- **Software**: CPU fallback renderer (default)

Platform information is included in performance reports and used to organize
baselines.

### Enabling Multi-Platform Testing

Multi-platform testing requires self-hosted GitHub Actions runners with specific
GPU hardware:

1. **Set up self-hosted runners**:
   - Configure runners with different GPU vendors
   - Tag runners with appropriate labels (e.g., `self-hosted-nvidia-gpu`)

2. **Update workflow matrix**:
   - Uncomment GPU platform entries in `.github/workflows/performance.yml`
   - Update runner labels to match your infrastructure

3. **Trigger workflow**:
   ```bash
   # Manual trigger with multi-platform enabled
   gh workflow run performance.yml -f enable_multi_platform=true
   ```

### Cross-Platform Comparison

When multiple platforms are tested, a comparison report is automatically
generated showing:

- Performance on each platform
- Platform-specific variations
- Hardware that performs best/worst for each test

Example comparison:

| Test                    | NVIDIA RTX 3080 | AMD RX 6800 | Intel Arc A770 | Software |
| ----------------------- | --------------- | ----------- | -------------- | -------- |
| basic_rendering         | 5.1ms           | 5.8ms       | 6.2ms          | 45ms     |
| large_dataset_rendering | 15.0ms          | 16.2ms      | 17.5ms         | 180ms    |

### Platform-Specific Baselines

Each platform maintains independent baselines:

```rust
// Baselines are automatically loaded based on detected platform
let runner = CiPerformanceRunner::new(debug_context, config)
    .with_platform_info(platform_info);  // Auto-detects GPU
```

### Infrastructure Requirements

For full multi-platform testing:

- **NVIDIA Runner**: Linux machine with RTX 3070+ or equivalent
- **AMD Runner**: Linux machine with RX 6000+ series
- **Intel Runner**: Linux machine with Arc A-series
- **Software Runner**: Standard GitHub-hosted runner (no GPU)

Alternatively, use cloud GPU instances for occasional multi-platform validation.

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
