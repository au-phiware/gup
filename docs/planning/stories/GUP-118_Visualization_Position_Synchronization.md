# GUP-118: Visualization Position Synchronization

## Story Overview

**Title**: Visualization Position Synchronization  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Medium  
**Story Points**: 5  
**Status**: 🚧 In Progress  
**Started**: 2025-01-24

## Context

GUP-117 created the DOM overlay structure with placeholder element positioning.
For production use, overlay elements need to be positioned at the actual
coordinates of their corresponding visualization marks.

This story implements position synchronization between GPU-rendered marks and
DOM overlay elements, including updates on pan, zoom, and data changes.

## User Story

**As a** keyboard-only user  
**I want** focusable elements to appear exactly where data points are rendered  
**So that** I can accurately understand the spatial layout of the visualization

## Acceptance Criteria

### AC1: Mark Position Integration

- [ ] Query mark positions from GPU buffers
- [ ] Transform GPU coordinates to screen coordinates
- [ ] Apply transforms to overlay element positioning
- [ ] Handle viewport coordinate system correctly

### AC2: Dynamic Updates

- [ ] Update positions on data changes
- [ ] Update positions on pan operations
- [ ] Update positions on zoom operations
- [ ] Update positions on window resize

### AC3: Performance

- [ ] Position updates run at 60 FPS
- [ ] Use requestAnimationFrame for smooth updates
- [ ] Batch position updates efficiently
- [ ] Minimize layout thrashing

### AC4: Coordinate Accuracy

- [ ] Overlay elements align with visual marks (±2px tolerance)
- [ ] Handles transforms correctly (translation, scale)
- [ ] Respects chart margins and padding
- [ ] Works with multiple charts

## Dependencies

### Prerequisite Stories

- GUP-117: Web Accessibility DOM Overlay ✅

### Enables Stories

- Production-quality web accessibility
- Accurate keyboard navigation targets
- Touch target alignment

## Technical Tasks

- [ ] Add position query API to mark system
- [ ] Implement coordinate transformation pipeline
- [ ] Create update subscription system
- [ ] Add position sync to WebDomOverlay
- [ ] Write performance tests
- [ ] Document coordinate systems

## Testing Strategy

- Unit tests for coordinate transformations
- Integration tests for position accuracy
- Performance tests for update frequency
- Visual tests with screenshot comparison
- Manual testing with keyboard navigation

## Success Metrics

- Overlay elements within ±2px of visual marks
- 60 FPS position updates
- No visible lag during interactions
- Works across browsers

## Definition of Done

- [ ] Position synchronization implemented
- [ ] Dynamic updates working
- [ ] Performance targets met
- [ ] Tests passing
- [ ] Documentation updated
- [ ] Code reviewed
