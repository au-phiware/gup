# GUP-199: Text Wrapping and Multi-Line Layout

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 5  
**Status**: 📋 Planned  
**Dependencies**: GUP-105 (Text Clipping Detection)

## Problem Statement

The text rendering system supports single-line text with clipping strategies
(truncation, font scaling, reposition, hide) but lacks multi-line text wrapping.
When text exceeds container width, wrapping to multiple lines within boundaries
would provide an additional strategy for keeping content readable without
truncation.

## User Story

**As a** data visualization developer  
**I want** text to automatically wrap to multiple lines within container
boundaries  
**So that** long labels and descriptions remain fully readable without
truncation

## Acceptance Criteria

- [ ] Word-level text wrapping within container width
- [ ] Configurable maximum number of lines
- [ ] Line height and spacing calculations using existing
      `TextStyle.line_spacing`
- [ ] Integration as a `ClippingStrategy::TextWrapping` variant
- [ ] Hyphenation support (optional, configurable)
- [ ] Performance: wrapping 100 labels in <5ms

## Technical Tasks

1. Implement word-level line breaking algorithm in `TextLayoutEngine`
2. Add `ClippingStrategy::TextWrapping` variant with `max_lines` and
   `line_spacing_factor` fields
3. Generate multiple lines of `PositionedGlyph` with proper Y offsets
4. Update `TextBounds` calculation for multi-line text
5. Add unit and integration tests

## Testing Strategy

- Unit tests for line breaking algorithm with various text lengths
- Integration tests with real GPU FontAtlas
- Performance tests with many wrapped labels
- Visual verification with demo

## Definition of Done

- [ ] Text wrapping strategy implemented
- [ ] Tests passing with >90% coverage
- [ ] Performance benchmarks meet targets
- [ ] Integration with existing clipping strategy cascade

---

**Story Created**: 2026-02-26  
**Origin**: GUP-105 follow-up ("Could Have" AC not implemented)
