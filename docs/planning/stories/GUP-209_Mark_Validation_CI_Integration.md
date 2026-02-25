# GUP-209: Mark Validation CI Integration

**Status**: 📋 Planned **Priority**: Low **Category**: Developer Experience
**Estimated Effort**: 0.5 days **Dependencies**: GUP-071 (Custom Mark
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

- [ ] `mask validate-marks` command runs `MarkValidator` on all built-in mark
      types (Circle, Rectangle, Line, Path, BoxPlot, Text)
- [ ] Command exits with non-zero code if any mark fails validation
- [ ] Validation report is printed in human-readable format
- [ ] Performance profiling results are included in the output
- [ ] Integration with `mask all-check` for pre-commit validation

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

- [ ] All acceptance criteria met
- [ ] `mask validate-marks` runs successfully
- [ ] Integrated into `mask all-check`
- [ ] Documentation updated
