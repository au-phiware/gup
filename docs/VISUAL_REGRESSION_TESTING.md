# Visual Regression Testing for Patterns

This document explains how to maintain and use the visual regression testing
system for pattern rendering.

## Overview

Visual regression tests capture screenshots of rendered patterns and compare
them with reference images. This catches visual bugs that unit tests might miss,
such as:

- Spacing changes
- Color blend issues
- Aliasing artifacts
- Pattern alignment problems

## Running Tests

To run all visual regression tests:

```bash
cargo test --test pattern_visual_regression_tests -- --test-threads=1
```

Note: `--test-threads=1` is required because GPU resources cannot be accessed
safely from multiple threads.

## Test Structure

### Test Infrastructure

- `tests/visual_regression_utils.rs` - Core visual testing utilities
  - `VisualTestRenderer` - Renders patterns to offscreen textures
  - `VisualTestConfig` - Configuration for tolerance, paths, etc.
  - `ComparisonResult` - Results of image comparison
- `tests/visual_test_pattern_shader.wgsl` - Shader for rendering test patterns
- `tests/pattern_visual_regression_tests.rs` - Actual test cases

### Directory Structure

- `tests/visual_references/` - Reference images (committed to git)
- `target/visual_test_outputs/` - Current test output (not committed)
- `target/visual_test_diffs/` - Visual diffs when tests fail (not committed)

## Adding New Visual Tests

To add a new pattern visual test:

1. **Create the test function**:

```rust
#[tokio::test]
async fn test_visual_my_new_pattern() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    // Create your pattern
    let pattern = Pattern::Lines {
        spacing: 10.0,
        angle: std::f32::consts::PI / 6.0,
    };

    // Render it
    render_pattern_test(&renderer, &pattern, Color::BLACK, Color::WHITE).await?;

    // Compare with reference
    let result = renderer.capture_and_compare("my_new_pattern").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}
```

2. **Run the test** to generate the reference image:

```bash
cargo test --test pattern_visual_regression_tests test_visual_my_new_pattern -- --test-threads=1
```

3. **Review the reference image** at
   `tests/visual_references/my_new_pattern.png`

4. **Commit the reference image** to git:

```bash
git add tests/visual_references/my_new_pattern.png
```

## Updating Reference Images

If you intentionally change pattern rendering (e.g., improve quality, fix bugs),
you need to update reference images:

### Option 1: Update All References

```bash
# Delete all reference images
rm tests/visual_references/*.png

# Re-run all tests to regenerate
cargo test --test pattern_visual_regression_tests -- --test-threads=1

# Review all changes
git diff tests/visual_references/

# Commit if correct
git add tests/visual_references/*.png
git commit -m "Update pattern visual references after [change description]"
```

### Option 2: Update Specific Test

```bash
# Delete specific reference
rm tests/visual_references/dots_16px.png

# Re-run that test
cargo test --test pattern_visual_regression_tests test_visual_dots_pattern -- --test-threads=1

# Review and commit
git add tests/visual_references/dots_16px.png
```

## Understanding Test Failures

When a test fails, you'll see output like:

```
FAIL - 2.50% pixels differ (max diff: 10.25%), 12000 / 480000 pixels
```

This means:

- 2.50% of pixels differed from the reference
- The maximum difference in any pixel was 10.25% (per channel)
- 12,000 out of 480,000 total pixels were different

### Investigating Failures

1. **Check the diff image**:
   - Located at `target/visual_test_diffs/[test_name].png`
   - Red pixels show where differences occurred
   - Gray pixels show where images matched

2. **Check the output image**:
   - Located at `target/visual_test_outputs/[test_name].png`
   - This is what the current code rendered

3. **Compare with reference**:
   - Reference is at `tests/visual_references/[test_name].png`
   - Use an image viewer to spot differences

### Common Causes of Failures

1. **Intentional changes**: You modified rendering code. Update references.

2. **GPU driver differences**: Different GPUs may produce slightly different
   results. Adjust tolerance in `VisualTestConfig` if needed.

3. **Floating point precision**: Very small differences (<1%) are often rounding
   artifacts. The default 2% tolerance should handle these.

4. **Actual bugs**: Visual artifacts you didn't intend. Fix the code!

## Configuring Test Tolerance

Default configuration:

```rust
VisualTestConfig {
    width: 800,
    height: 600,
    pixel_tolerance: 0.02,        // 2% per channel (~5/255)
    pixel_diff_threshold: 0.01,   // 1% of pixels can differ
    // ... paths ...
}
```

To use custom tolerance for a specific test:

```rust
let mut config = VisualTestConfig::default();
config.pixel_tolerance = 0.05;  // Allow 5% channel difference
config.pixel_diff_threshold = 0.02;  // Allow 2% pixel differences

let renderer = VisualTestRenderer::new(config).await?;
```

## CI Integration

Visual regression tests run in CI with the same commands:

```yaml
- name: Run visual regression tests
  run: cargo test --test pattern_visual_regression_tests -- --test-threads=1
```

**Important**: Reference images must be committed to git for CI to work. If
tests fail in CI, it means your code produces different output than the
committed references.

## Best Practices

1. **Keep reference images small**: Use reasonable resolutions (800x600 default)

2. **Test specific scenarios**: Each test should focus on one pattern type or
   edge case

3. **Meaningful test names**: Use descriptive names that indicate what's being
   tested

4. **Review changes carefully**: Always inspect reference images before
   committing

5. **Update docs**: If you add new tests or change behavior, update this
   document

## Troubleshooting

### Tests fail with "Buffer mapping failed"

- Make sure you're using `--test-threads=1`
- Check that you have GPU access (may fail in some CI environments)

### All tests create reference images

- Reference images were deleted or not committed
- Run once to generate, then commit the references

### Differences between machines

- Different GPUs may produce slightly different results
- Increase tolerance or ensure same GPU type in CI
- Use software rendering for perfect reproducibility (add to config)

### Out of GPU memory

- Reduce test resolution in `VisualTestConfig`
- Run fewer tests at once
- Ensure proper cleanup in tests

## Technical Details

### Texture-to-Buffer Copying

The implementation handles wgpu's `COPY_BYTES_PER_ROW_ALIGNMENT` requirement
(256 bytes) by padding rows during copy and removing padding when creating
images.

### Comparison Algorithm

- Per-pixel comparison using L-infinity distance (maximum channel difference)
- Configurable per-channel tolerance (0-1 range)
- Counts pixels that exceed tolerance
- Generates visual diff showing failed pixels in red

### Render Pipeline

Tests use a simple full-screen quad shader that applies pattern functions
directly. This isolates pattern rendering from other rendering systems.
