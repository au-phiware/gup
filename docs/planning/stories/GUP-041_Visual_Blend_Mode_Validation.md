# GUP-041: Visual Blend Mode Validation

## Story Overview

**Title**: Visual Validation Framework for Blend Modes  
**Epic**: Phase 1 Initiative 2 - Testing and Quality Infrastructure  
**Priority**: Medium  
**Story Points**: 2

## Context

GUP-027 implemented GPU blend state integration with functional tests, but lacks
visual validation. We need to ensure that blend modes produce the expected
visual results, not just that they execute without errors.

## User Story

**As a** graphics developer  
**I want** visual tests that validate blend mode rendering results  
**So that** I can be confident blend modes produce correct visual output across
platforms

## Acceptance Criteria

### AC1: Visual Test Framework

- [ ] Create framework for rendering to offscreen textures
- [ ] Implement pixel-perfect comparison utilities
- [ ] Generate reference images for each blend mode

### AC2: Blend Mode Visual Tests

- [ ] Test each blend mode with known color combinations
- [ ] Validate alpha blending produces expected color mixing
- [ ] Test additive blend mode produces expected brightening
- [ ] Test multiply blend mode produces expected darkening

### AC3: Cross-Platform Validation

- [ ] Reference images work across different GPU vendors
- [ ] Handle minor precision differences in floating-point calculations
- [ ] Platform-specific test variations where needed

## Technical Design

### Visual Test Utilities

```rust
pub struct VisualTestUtils {
    context: RenderContext,
    reference_images: HashMap<String, Vec<u8>>,
}

impl VisualTestUtils {
    pub fn render_blend_test(&mut self,
        bg_color: [f32; 4],
        fg_color: [f32; 4],
        blend_mode: BlendMode
    ) -> Vec<u8> {
        // Render test pattern and return pixel data
    }

    pub fn compare_with_reference(&self,
        actual: &[u8],
        reference_name: &str,
        tolerance: f32
    ) -> bool {
        // Compare with stored reference image
    }
}
```

## Definition of Done

- [ ] Visual test framework renders blend modes to offscreen targets
- [ ] Reference images stored for all supported blend modes
- [ ] Tests validate visual correctness with appropriate tolerance
- [ ] Cross-platform compatibility verified on major GPU vendors
- [ ] Integration with existing test suite

## Notes

This complements GUP-027 by adding visual validation to the functional tests.
Essential for graphics correctness.
