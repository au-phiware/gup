# GUP-293: LAB/OKLab Perceptual Color Space Shader Functions

## Story Overview

**Title**: LAB/OKLab Perceptual Color Space Shader Functions **Epic**: Phase 1
Initiative 2 - Unified Shader Function System **Priority**: Low **Story
Points**: 5 **Status**: ✅ Complete (2025-07-25)

## Context

GUP-053 implemented RGB↔HSV color space conversion as a composable shader
function but deferred perceptual color spaces (LAB, OKLab) due to their
additional complexity (XYZ intermediate space, illuminant references).
Perceptual color spaces are valuable for data visualization because they provide
perceptually uniform color gradients — differences in color correspond linearly
to differences in data values.

## User Story

**As a** visualization developer **I want** perceptual color space conversions
on the GPU **So that** I can create perceptually uniform color scales that
accurately represent data differences

## Acceptance Criteria

- [x] RGB↔XYZ↔LAB conversion as composable shader functions
- [x] RGB↔OKLab conversion (modern perceptual color space)
- [x] LCH (Lightness-Chroma-Hue) cylindrical form of LAB
- [x] Perceptual color interpolation function (interpolate in LAB/OKLab space)
- [x] D65 illuminant as default with configurable illuminant
- [x] Unit tests validating conversion accuracy against known values

## Technical Tasks

1. Implement XYZ color space as intermediate (RGB→XYZ→LAB)
2. Implement LAB conversion with D65 illuminant reference
3. Implement OKLab conversion (simpler math, no illuminant needed)
4. Implement LCH cylindrical coordinates from LAB
5. Create perceptual interpolation function that converts to LAB, interpolates,
   converts back to RGB
6. Write comprehensive tests with known color conversion values

## Dependencies

- GUP-053: Advanced Shader Function Library ✅ (provides ColorSpaceConverter
  pattern)

## Testing Strategy

- Unit tests with known color conversion values (e.g., sRGB white → LAB [100, 0,
  0])
- Round-trip tests (RGB → LAB → RGB should be identity within float tolerance)
- Perceptual uniformity validation tests

## Risk Assessment

- **Medium**: WGSL floating-point precision may cause small deviations from
  reference implementations
- **Low**: OKLab is mathematically simpler than LAB and may be preferred

## Definition of Done

- [x] All color space conversions implemented and tested
- [x] Perceptual interpolation function works in composition chains
- [x] Documentation with color science references
- [x] Performance validation for real-time use

## Implementation Summary

### Key Files Changed

- **`src/shader_function.rs`**: Added ~690 lines of implementation:
  - `PerceptualColorSpaceConverter` struct with 8 conversion directions
    (RGB↔XYZ, RGB↔LAB, RGB↔OKLab, RGB↔LCH)
  - `PerceptualColorSpaceConverterUniforms` with configurable D65 illuminant
  - `PerceptualInterpolation` struct for perceptually uniform color blending
    in LAB, OKLab, or LCH spaces
  - `PerceptualInterpolationUniforms` with two endpoint colours and space
    selector
  - WGSL shader code for all conversions including sRGB linearisation,
    XYZ matrix transforms, LAB f/f_inv functions, OKLab matrices,
    LCH cylindrical conversion, and shortest-arc hue interpolation
  - 35 unit tests: direction mapping, uniform alignment, WGSL content
    validation, known CIE reference values (white, black, red), round-trip
    tests for all 5 conversion paths, perceptual uniformity spot-checks,
    and composability verification
- **`src/prelude.rs`**: Exported 4 new public types

### Test Counts

- 35 new tests added
- 2813 total tests pass (4 ignored)
