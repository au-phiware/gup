# GUP-127: Focus Elements for Data Points

## Story Overview

**Title**: Focus Elements for Accessible Data Point Navigation  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: High  
**Story Points**: 5  
**Status**: 🚧 In Progress  
**Started**: 2025-02-22

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

## Implementation Status

**Status**: ⚠️ **Blocked by Missing Selection Type**

### What Was Implemented

1. **`focus_elements.rs`** - Mark focus helper system
   - `FocusElementConfig` for configuration
   - `MarkFocusHelper` for converting mark positions to focusable elements
   - Automatic registration with FocusManager
   - Performance limits (max 1000 elements)
   - Full unit test coverage

2. **`focus_ring.rs`** - GPU-accelerated focus ring renderer
   - `FocusRingRenderer` with instanced rendering
   - `FocusRingStyle` with default, high contrast, and animated variants
   - WCAG AAA compliant high contrast mode
   - Animation support
   - Multi-select focus ring support
   - Full unit test coverage

3. **Documentation**
   - Usage guide: `docs/FOCUS_ELEMENTS_GUIDE.md`
   - Comprehensive examples
   - Integration patterns
   - Accessibility features documented

### What Is Blocked

**Critical Blocker**: The codebase references a `Selection<T, M>` type in `crate::selection` that has never been implemented. This type was referenced in GUP-111 as if it existed, but `src/selection.rs` is an empty file.

This blocks:
- ✗ Integration tests (cannot create Selection instances)
- ✗ Working examples (cannot compile due to Selection references throughout codebase)
- ✗ Full AC validation (ACs assume Selection integration)

### Partial Completion Assessment

**Acceptance Criteria**:
- AC1 (Focusable Mark Elements): ✅ API implemented, ⚠️ cannot demonstrate due to blocker
- AC2 (FocusManager Integration): ✅ Implemented, ⚠️ cannot test due to blocker
- AC3 (Focus Visual Feedback): ✅ Fully implemented and tested

**Technical Tasks**:
- ✅ Create `FocusElement` component for marks - Done
- ⚠️ Integrate Selection with FocusManager - Blocked (Selection doesn't exist)
- ✅ Implement focus element positioning - Done
- ✅ Add focus ring rendering to marks - Done
- ⚠️ Handle focus updates on data changes - Blocked (requires Selection)
- ✅ Add keyboard event handling - Already exists in FocusManager
- ✅ Support focus element pooling - Implemented via max_elements config

### Files Created

- `src/accessibility/focus_elements.rs` (187 lines)
- `src/accessibility/focus_ring.rs` (389 lines)
- `docs/FOCUS_ELEMENTS_GUIDE.md` (250 lines)

### Tests

Unit tests pass for both modules:
```bash
cargo test accessibility::focus_elements
cargo test accessibility::focus_ring
```

Integration tests blocked by Selection type not existing.

### Next Steps

**Option 1**: Complete GUP-002 (Core Selection Type) first, then return to this story

**Option 2**: Create a minimal Selection stub just for testing/examples

**Option 3**: Mark story as "Partially Complete - Blocked" and document the working components

**Recommendation**: Mark as blocked and create follow-up story to implement Selection type properly.
