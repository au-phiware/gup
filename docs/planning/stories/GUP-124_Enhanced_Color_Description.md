# GUP-124: Enhanced Color Description

## Story Overview

**Title**: Enhanced Color Description for Accessibility  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Low  
**Story Points**: 2  
**Status**: 💡 New

## Context

GUP-111 implemented basic color description for accessible mark descriptions using
simple RGB threshold-based approximation (e.g., R>0.8 → "red"). While this works for
common cases, it fails for many real-world colors like orange, brown, pink, purple,
and subtle variations.

Screen reader users benefit from accurate color descriptions that match their
intuitive understanding of colors. A more sophisticated color naming system based
on HSL color space and perceptual color distance would provide better descriptions.

## User Story

**As a** screen reader user  
**I want** accurate human-readable color names for data points  
**So that** I can understand color-based encodings without visual perception

## Acceptance Criteria

### AC1: HSL-Based Color Naming

- [ ] Convert RGBA to HSL color space
- [ ] Use hue, saturation, lightness to determine base color
- [ ] Handle edge cases (grayscale, very dark, very light)
- [ ] Support at least 12 distinct color names (red, orange, yellow, green, cyan, blue, purple, magenta, pink, brown, white, black, gray)

### AC2: Perceptual Accuracy

- [ ] Match common color expectations (orange between red and yellow)
- [ ] Distinguish shades (light blue vs dark blue)
- [ ] Handle desaturated colors (grayish-blue)
- [ ] Validate against common data visualization palettes

### AC3: API Improvements

- [ ] Add `ColorDescriptor` utility struct/module
- [ ] Support both basic and detailed descriptions
- [ ] Allow custom color naming schemes
- [ ] Update `Circle::describe_point()` and other marks to use new system

## Dependencies

### Prerequisite Stories

- GUP-111: Automatic ARIA Generation ✅

### Enables Stories

- Better accessibility for categorical color encodings
- More accurate screen reader descriptions

## Technical Tasks

- [ ] Create `ColorDescriptor` module with RGBA→HSL conversion
- [ ] Implement perceptual color naming based on HSL
- [ ] Add tests for common colors and edge cases
- [ ] Update `AccessibleMark` implementations to use `ColorDescriptor`
- [ ] Add optional detailed description mode (e.g., "light grayish-blue")

## Success Metrics

- 90%+ accuracy on common data visualization colors
- <1ms overhead per color conversion
- Handles all CSS named colors correctly
- Screen reader feedback indicates better comprehension

## Definition of Done

- [ ] HSL-based color naming implemented
- [ ] Tests cover 20+ distinct colors
- [ ] All marks updated to use new system
- [ ] Documentation explains color naming algorithm
- [ ] Performance benchmarks show <1ms overhead
