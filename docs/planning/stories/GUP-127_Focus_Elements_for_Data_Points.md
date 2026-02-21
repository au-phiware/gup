# GUP-127: Focus Elements for Data Points

## Story Overview

**Title**: Focus Elements for Accessible Data Point Navigation  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: High  
**Story Points**: 5  
**Status**: 💡 New

## Context

GUP-111 implemented ARIA tree generation, but screen reader users still cannot
navigate individual data points using keyboard/focus. Marks need corresponding
focusable elements that integrate with the FocusManager from GUP-016 to enable
full keyboard accessibility.

This is critical for WCAG 2.1 AA compliance (Success Criterion 2.1.1: Keyboard)
and enables keyboard-only users to explore data visualizations.

## User Story

**As a** keyboard-only user  
**I want** to navigate between data points using Tab and Arrow keys  
**So that** I can explore visualizations without a mouse

## Acceptance Criteria

### AC1: Focusable Mark Elements

- [ ] Each mark instance creates a focusable element
- [ ] Focus elements positioned at mark centers
- [ ] Invisible by default (visible on focus for debugging)
- [ ] Associated with corresponding ARIA node

### AC2: Integration with FocusManager

- [ ] Register focus elements with FocusManager from GUP-016
- [ ] Support sequential navigation (Tab/Shift+Tab)
- [ ] Support spatial navigation (Arrow keys)
- [ ] Support data dimension navigation
- [ ] Update focus on data changes

### AC3: Focus Visual Feedback

- [ ] Focus ring around focused mark
- [ ] Configurable focus style (color, width, dash pattern)
- [ ] High contrast mode support
- [ ] Animated focus transitions (optional)

## Dependencies

### Prerequisite Stories

- GUP-016: Core Accessibility System ✅
- GUP-111: Automatic ARIA Generation ✅

### Enables Stories

- Full keyboard accessibility
- WCAG 2.1 AA compliance
- Interactive accessible visualizations

## Technical Tasks

- [ ] Create `FocusElement` component for marks
- [ ] Integrate Selection with FocusManager
- [ ] Implement focus element positioning
- [ ] Add focus ring rendering to marks
- [ ] Handle focus updates on data changes
- [ ] Add keyboard event handling
- [ ] Support focus element pooling (large datasets)

## Success Metrics

- All marks focusable via keyboard
- Tab order follows natural data order
- Spatial navigation works intuitively
- <10ms focus transition time
- Focus visible at all zoom levels

## Risk Assessment

### Performance Risk

**Risk**: Creating focus elements for 10K+ points may be slow  
**Mitigation**: Implement focus element pooling, only create for visible/nearby points  
**Fallback**: Limit focus elements to 1000 points max

### Complexity Risk

**Risk**: Coordinate system mismatch between GPU rendering and DOM focus elements  
**Mitigation**: Use viewport transform from GUP-118, test with various scales  
**Fallback**: Disable focus elements for non-standard projections

## Definition of Done

- [ ] All marks have focusable elements
- [ ] FocusManager integration working
- [ ] Keyboard navigation (Tab, Shift+Tab, Arrows) functional
- [ ] Focus visuals rendered correctly
- [ ] Tests validate focus behavior
- [ ] Examples demonstrate keyboard navigation
- [ ] Performance acceptable with 1000+ focus elements
