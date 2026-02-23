# GUP-114: macOS NSAccessibility Integration

## Story Overview

**Title**: macOS NSAccessibility Integration  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Medium  
**Story Points**: 8  
**Status**: 🚧 In Progress  
**Started**: 2025-01-25

## Context

GUP-112 implemented the architecture for platform-specific accessibility bridges
with a stub implementation for macOS NSAccessibility. This story completes the
macOS implementation by integrating Objective-C bindings and implementing the
full NSAccessibility protocol.

macOS provides the NSAccessibility protocol for assistive technologies.
VoiceOver, macOS's built-in screen reader, relies on NSAccessibility to
understand and interact with applications.

## User Story

**As a** macOS user with visual impairments using VoiceOver  
**I want** Gup visualizations to be fully accessible through NSAccessibility  
**So that** I can explore data visualizations with my screen reader

## Acceptance Criteria

### AC1: NSAccessibility Protocol Implementation

- [ ] Integrate objc2 crate for Objective-C bindings
- [ ] Implement NSAccessibility element hierarchy
- [ ] Support all required NSAccessibility attributes
- [ ] Handle NSAccessibility actions

### AC2: VoiceOver Integration

- [ ] Announcements via NSAccessibilityPostNotification
- [ ] Focus management with NSAccessibilityFocusedUIElement
- [ ] Semantic role mapping to NSAccessibility roles
- [ ] Support for rotor navigation

### AC3: Native Cocoa Integration

- [ ] Integrate with winit/raw-window-handle for NSWindow
- [ ] Create NSAccessibility element tree for visualization
- [ ] Update accessibility tree on data changes
- [ ] Proper memory management with ARC

## Dependencies

### Prerequisite Stories

- GUP-016: Core Accessibility System ✅
- GUP-112: Platform-Specific Accessibility Integration ✅

### Enables Stories

- Production-ready macOS accessibility
- VoiceOver compatibility
- macOS app store compliance

## Technical Tasks

- [ ] Add objc2 dependency to Cargo.toml
- [ ] Create Objective-C bridge layer
- [ ] Implement NSAccessibility element wrapper
- [ ] Map AriaRole to NSAccessibility roles
- [ ] Implement NSAccessibility attribute methods
- [ ] Add VoiceOver announcement support
- [ ] Create macOS-specific integration tests
- [ ] Document NSAccessibility mapping

## Testing Strategy

- Manual testing with VoiceOver enabled
- Test with macOS Accessibility Inspector
- Verify rotor navigation works correctly
- Test with keyboard-only navigation
- Validate against macOS accessibility guidelines

## Success Metrics

- VoiceOver reads all chart elements correctly
- Rotor navigation works for data exploration
- Passes Accessibility Inspector validation
- Zero VoiceOver navigation issues
- Meets macOS app store accessibility requirements

## Definition of Done

- [ ] objc2 integration complete
- [ ] Full NSAccessibility protocol implemented
- [ ] Tested with VoiceOver
- [ ] Passes Accessibility Inspector checks
- [ ] Documentation includes VoiceOver usage guide
- [ ] All tests passing
- [ ] Code reviewed and approved
