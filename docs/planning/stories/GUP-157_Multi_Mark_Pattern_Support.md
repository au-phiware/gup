# GUP-157: Multi-Mark Pattern Support

## Story Overview

**Title**: Extend Pattern Rendering to All Mark Types  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Medium  
**Story Points**: 5  
**Status**: 📋 Planned

## Context

GUP-113 created pattern infrastructure and GUP-119 integrated it with circles. However, Gup has multiple mark types (rectangles, lines, paths, text), and all should support pattern rendering for comprehensive accessibility.

## User Story

**As a** colorblind user  
**I want** patterns on all visualization types  
**So that** I can distinguish data categories regardless of chart type

## Acceptance Criteria

### AC1: Pattern Shaders for All Marks

- [ ] Rectangle marks with pattern support
- [ ] Line marks with pattern support  
- [ ] Path marks with pattern support
- [ ] Text background patterns (optional)
- [ ] Boxplot marks with pattern support

### AC2: Consistent Pattern Behavior

- [ ] All marks use same PatternRenderer
- [ ] Patterns scale appropriately per mark type
- [ ] Pattern orientation handles mark rotation
- [ ] Edge cases handled (small marks, overlaps)

### AC3: Testing & Examples

- [ ] Tests for each mark type with patterns
- [ ] Example showing all mark types with patterns
- [ ] Visual regression tests
- [ ] Performance validation across mark types

## Dependencies

### Prerequisite Stories

- GUP-113: Pattern-Based Rendering Implementation ✅
- GUP-119: Mark Pipeline Pattern Integration

## Technical Tasks

- [ ] Create pattern-enabled fragment shaders for each mark
- [ ] Implement pattern scaling logic per mark type
- [ ] Handle pattern orientation for rotated marks
- [ ] Add pattern tests for each mark type
- [ ] Create comprehensive example
- [ ] Validate performance across mark types

## Success Metrics

- All mark types render patterns correctly
- Consistent pattern appearance across types
- Performance targets met for all mark types
- No visual artifacts or edge cases

## Definition of Done

- [ ] All mark types support patterns
- [ ] Pattern shaders for each mark type
- [ ] Tests for all mark types
- [ ] Example with multiple mark types
- [ ] Documentation updated
- [ ] Performance validated
