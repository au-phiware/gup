# GUP-160: Pattern Visual Regression Tests

## Story Overview

**Title**: Implement Screenshot-Based Visual Regression Testing for Patterns  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Low  
**Story Points**: 5  
**Status**: 🚧 In Progress

## Context

Pattern rendering has functional tests but no visual validation. Since patterns
are visual by nature, automated screenshot comparison would catch visual
regressions that unit tests might miss (spacing issues, alignment problems,
aliasing artifacts).

## User Story

**As a** Gup maintainer  
**I want** automated visual regression tests for patterns  
**So that** I can detect visual quality issues before they reach users

## Acceptance Criteria

### AC1: Infrastructure

- [ ] Headless rendering infrastructure for tests
- [ ] Screenshot capture mechanism
- [ ] Image comparison algorithm with tolerance
- [ ] Reference image storage and versioning
- [ ] Test failure reporting with visual diffs

### AC2: Pattern Test Coverage

- [ ] Visual tests for each mark type (Circle, Rectangle, Line, BoxPlot)
- [ ] Visual tests for each pattern type (Dots, Lines, Crosshatch)
- [ ] Tests for different pattern spacings
- [ ] Tests for different pattern angles
- [ ] Tests for edge cases (small marks, large patterns)

### AC3: CI Integration

- [ ] Tests run in CI environment
- [ ] Reference images stored in version control or artifact storage
- [ ] Test results include visual diff images
- [ ] Clear pass/fail criteria based on acceptable visual difference

## Dependencies

### Prerequisite Stories

- GUP-113: Pattern-Based Rendering Implementation ✅
- GUP-157: Multi-Mark Pattern Support ✅

### External Dependencies

- Headless rendering capability (wgpu with offscreen surfaces)
- Image comparison library (e.g., image-compare, pixelmatch)
- CI environment with GPU support or software rendering

## Technical Tasks

- [ ] Set up headless rendering for tests
- [ ] Implement screenshot capture utility
- [ ] Choose and integrate image comparison library
- [ ] Create reference screenshots for all pattern/mark combinations
- [ ] Write visual regression test harness
- [ ] Integrate with cargo test infrastructure
- [ ] Add CI configuration for visual tests
- [ ] Document how to update reference images

## Success Metrics

- Visual tests detect spacing changes (>2px difference)
- Visual tests detect color/blend issues
- Test execution time <30 seconds for full suite
- <5% false positive rate (spurious failures)

## Definition of Done

- [ ] Visual regression test infrastructure implemented
- [ ] Tests for all mark types and pattern types
- [ ] Reference images committed
- [ ] Tests run in CI
- [ ] Documentation for maintaining tests
- [ ] False positive rate measured and acceptable

## Risk Assessment

**Technical Risks**:

- Headless rendering may behave differently than windowed rendering
- GPU differences across machines may cause pixel differences
- Image comparison thresholds may be hard to tune
- Reference image maintenance overhead

**Mitigation**:

- Use software rendering for consistency
- Allow configurable comparison tolerance
- Start with small reference image set
- Document image update process clearly
