# GUP-209: Mark Validation CI Integration

**Status**: ✅ Complete (2025-07-22) **Priority**: Low **Category**: Developer
Experience **Estimated Effort**: 0.5 days **Dependencies**: GUP-071 (Custom Mark
Development Kit)

## Summary

Add a `mask validate-marks` command that runs `MarkValidator` on all registered
mark types as part of the CI pipeline, with configurable failure thresholds.

## Background

GUP-071 delivered `MarkValidator` for validating custom mark implementations.
Currently, validation is run manually or in individual tests. Integrating it
into the `mask` task runner would catch mark regressions automatically as part
of the standard development workflow.

## User Story

As a developer contributing to Gup, I want mark validation to run automatically
as part of the CI checks, so that regressions in mark implementations are caught
before merge.

## Acceptance Criteria

- [x] `mask validate-marks` command runs `MarkValidator` on all built-in mark
      types (Circle, Rectangle, Line, Path, BoxPlot, Text)
- [x] Command exits with non-zero code if any mark fails validation
- [x] Validation report is printed in human-readable format
- [x] Performance profiling results are included in the output
- [x] Integration with `mask all-check` for pre-commit validation

## Technical Tasks

1. Create validation runner script or Rust binary
2. Add `validate-marks` task to `maskfile.md`
3. Wire into `all-check` pipeline
4. Add configurable thresholds for performance checks

## Testing Strategy

- Run command and verify all marks pass
- Verify failure reporting with intentional test failures
- Test performance under CI-like conditions

## Risk Assessment

- **Low risk**: Uses existing validation infrastructure
- **CI timing**: Need to ensure validation completes within reasonable CI
  timeframe

## Definition of Done

- [x] All acceptance criteria met
- [x] `mask validate-marks` runs successfully
- [x] Integrated into `mask all-check`
- [x] Documentation updated

## Implementation Summary

### What was implemented

- **`validate_marks` binary** (`src/bin/validate_marks.rs`): CLI tool that runs
  `MarkValidator` and `MarkProfiler` on all 6 built-in mark types (Circle,
  Rectangle, Line, Path, BoxPlot, Text). Prints a structured, human-readable
  report including per-section validation results, performance classification,
  vertex/index counts, memory usage, and generation timing. Exits with non-zero
  code if any mark fails validation.

- **`mask validate-marks` command** (`maskfile.md`): New mask task that runs the
  binary via `cargo run --bin validate_marks`.

- **`mask all-check` integration** (`maskfile.md`): Added `mask validate-marks`
  as a concurrent check in the `all-check` pipeline, ensuring mark validation
  runs alongside lint, format, and other checks.

- **Integration tests** (`tests/mark_validation_ci_tests.rs`): 5 tests verifying
  all built-in marks pass validation, have no critical issues, meet performance
  thresholds, produce expected report structure, and generate readable
  summaries.

### Key files changed

| File                                | Change                                          |
| ----------------------------------- | ----------------------------------------------- |
| `src/bin/validate_marks.rs`         | New validation runner binary                    |
| `Cargo.toml`                        | Added `[[bin]]` section                         |
| `maskfile.md`                       | Added validate-marks task, wired into all-check |
| `tests/mark_validation_ci_tests.rs` | New integration tests                           |

### Test counts

- 5 new integration tests (all passing)
- All 6 built-in marks pass validation with 0 critical issues and 0 errors
