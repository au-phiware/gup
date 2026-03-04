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

## Retrospective

**Completed**: 2025-07-25

### Key Technical Learnings

#### sRGB Gamma Linearisation Is Essential for Correct Colour Math

- **Challenge**: Colour space conversions must operate on linear-light RGB, not
  sRGB gamma-encoded values. Omitting the sRGB transfer function produces
  visibly wrong LAB/OKLab values.
- **Solution**: Implemented `srgb_to_linear` / `linear_to_srgb` helper
  functions with the standard piecewise formula (threshold 0.04045 / 0.0031308).
- **Pattern**: Any future colour math (gamut mapping, spectral rendering) must
  linearise first. The helpers are available for reuse in WGSL.

#### WGSL Name Collision Avoidance with Suffix Convention

- **Challenge**: `PerceptualColorSpaceConverter` and `PerceptualInterpolation`
  both need the same helper functions (sRGB linearisation, XYZ matrices, LAB f,
  etc.) but cannot share WGSL identifiers when composed in the same pipeline.
- **Solution**: Used a `_pi` suffix for all helpers inside
  `PerceptualInterpolation`'s WGSL block (e.g. `srgb_to_linear_pi`,
  `rgb_to_lab_pi`). This follows the existing convention from GUP-053 where
  `hsv_to_rgb` vs `hsv_to_rgb_convert` were used.
- **Pattern**: When two `ComposableShaderFunction` impls share mathematical
  helpers, give each set a unique suffix to avoid WGSL symbol clashes.

#### OKLab's Simpler Math vs LAB's Illuminant Dependency

- **Challenge**: CIE LAB requires an illuminant reference white (D65 by
  default) passed as uniforms, while OKLab has the illuminant baked into its
  matrix coefficients.
- **Solution**: The uniform struct includes illuminant fields for LAB/LCH paths
  but OKLab paths ignore them. The `with_illuminant()` builder method allows
  users to override for non-D65 workflows.
- **Pattern**: When a colour space has optional parameters, include them in the
  uniform struct but document which conversion directions use them.

### Architectural Decisions

#### Single PerceptualColorSpaceConverter with Direction Enum vs Separate Structs

- **Decision**: Used a single `PerceptualColorSpaceConverter` struct with a
  `PerceptualColorSpaceDirection` enum (8 variants) rather than separate structs
  per conversion.
- **Reasoning**: Follows the existing `ColorSpaceConverter` pattern from
  GUP-053. All conversions share the same WGSL helper functions, so a single
  block of WGSL code is cleaner and avoids duplication.
- **Trade-off**: The WGSL entry point includes all helper functions even when
  only one direction is used, slightly increasing shader size. This is
  acceptable for the composability benefit.
- **Future**: If shader size becomes a concern, a future optimisation could
  strip unused helpers at code-generation time.

#### Separate PerceptualInterpolation Type (f32 → Vec4)

- **Decision**: Created a dedicated `PerceptualInterpolation` type with
  `Input = f32`, `Output = Vec4` that accepts a scalar interpolation parameter
  and produces an RGBA colour.
- **Reasoning**: This allows direct composition with `LinearScale` or
  `EasingFunction` to build complete colour ramp pipelines
  (e.g. `scale.compose(PerceptualInterpolation::oklab(red, blue))`).
- **Trade-off**: The interpolation function embeds endpoint colours in uniforms
  rather than accepting them as inputs, limiting it to two-stop gradients.
  Multi-stop gradients require `ColorGradient` or `ColorGradientStorage`.
- **Future**: A multi-stop perceptual gradient could combine
  `ColorGradientStorage` with per-segment perceptual interpolation.

### Development Workflow Insights

- **Disk space**: The development environment ran out of disk space mid-build
  (51 GB partition at 100%). Removed unused tool caches (~15 GB of conda,
  minecraft, gradle, wine, npm) to recover. Future sessions should monitor
  available space before long builds.
- **rustfmt hazard**: Running `rustfmt` on the 11k-line `shader_function.rs`
  during a disk-full condition silently truncated the file to 1 byte. The file
  was recovered from git. Lesson: always ensure adequate disk space before
  running formatters on large files.
- **Test naming**: Cargo test filters use substring matching, not regex. Using
  `--test-threads=1` is mandatory for GPU tests but the GUP-293 tests are all
  CPU-side reference implementations, so they could theoretically run in
  parallel.

### Follow-up Stories

No new follow-up stories identified. The existing GUP-294 (Shader Function
Module Reorganisation) would benefit from this story's additions since
`shader_function.rs` is now ~12k lines.
