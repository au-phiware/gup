# GUP-116: Linux AT-SPI2 Integration

## Story Overview

**Title**: Linux AT-SPI2 Integration  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Medium  
**Story Points**: 8  
**Status**: 💡 New

## Context

GUP-112 implemented the architecture for platform-specific accessibility bridges
with a stub implementation for Linux AT-SPI2. This story completes the Linux
implementation by integrating D-Bus bindings and implementing the full AT-SPI2
protocol for ATK objects.

Linux desktop environments provide AT-SPI2 (Assistive Technology Service
Provider Interface) over D-Bus for accessibility. Orca, GNOME's screen reader,
uses AT-SPI2 to access application content.

## User Story

**As a** Linux user with visual impairments using Orca  
**I want** Gup visualizations to be fully accessible through AT-SPI2  
**So that** I can explore data visualizations with my screen reader

## Acceptance Criteria

### AC1: AT-SPI2 Protocol Implementation

- [ ] Integrate zbus crate for D-Bus communication
- [ ] Implement AT-SPI2 interfaces (Accessible, Component, Text)
- [ ] Support ATK object hierarchy
- [ ] Handle AT-SPI2 events and signals

### AC2: Orca Screen Reader Support

- [ ] Announcements via AT-SPI2 object:text-changed signal
- [ ] Focus management via focus events
- [ ] Semantic role mapping to ATK roles
- [ ] Support for navigation patterns

### AC3: D-Bus Integration

- [ ] Connect to AT-SPI2 accessibility bus
- [ ] Register application with AT-SPI2 registry
- [ ] Create D-Bus object paths for visualization elements
- [ ] Handle D-Bus method calls for accessibility queries

## Dependencies

### Prerequisite Stories

- GUP-016: Core Accessibility System ✅
- GUP-112: Platform-Specific Accessibility Integration ✅

### Enables Stories

- Production-ready Linux accessibility
- Orca compatibility
- GNOME/KDE desktop integration

## Technical Tasks

- [ ] Add zbus dependency to Cargo.toml
- [ ] Create D-Bus interface definitions
- [ ] Implement AT-SPI2 accessible objects
- [ ] Map AriaRole to ATK roles
- [ ] Implement AT-SPI2 interface methods
- [ ] Add D-Bus signal emission
- [ ] Create Linux-specific integration tests
- [ ] Document AT-SPI2 mapping

## Testing Strategy

- Manual testing with Orca enabled
- Test with Accerciser (accessibility inspector)
- Verify D-Bus communication with d-feet
- Test with keyboard-only navigation
- Validate against GNOME accessibility guidelines
- Test on multiple Linux distributions

## Success Metrics

- Orca reads all chart elements correctly
- Passes Accerciser validation
- Zero Orca navigation issues
- Works on Ubuntu, Fedora, Arch Linux
- Meets GNOME/KDE accessibility standards

## Definition of Done

- [ ] zbus integration complete
- [ ] Full AT-SPI2 protocol implemented
- [ ] Tested with Orca
- [ ] Passes Accerciser checks
- [ ] Documentation includes Orca usage guide
- [ ] All tests passing
- [ ] Code reviewed and approved
