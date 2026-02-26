# GUP-199: Text Wrapping and Multi-Line Layout

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 5  
**Status**: ✅ Complete  
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

- [x] Word-level text wrapping within container width
- [x] Configurable maximum number of lines
- [x] Line height and spacing calculations using existing
      `TextStyle.line_spacing`
- [x] Integration as a `ClippingStrategy::TextWrapping` variant
- [x] Hyphenation support (optional, configurable)
- [x] Performance: wrapping 100 labels in <5ms

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

- [x] Text wrapping strategy implemented
- [x] Tests passing with >90% coverage
- [x] Performance benchmarks meet targets
- [x] Integration with existing clipping strategy cascade

---

## Implementation Summary

**Completed**: 2025-07-18

### Key Files Changed

- **`src/text.rs`** — Added `GlyphSource` trait enabling mock-based testing of
  text layout algorithms. `FontAtlas` implements this trait.
- **`src/text/layout.rs`** — Core implementation:
  - Added `ClippingStrategy::TextWrapping` variant with `max_lines`,
    `line_spacing_factor`, and `hyphenate` fields
  - `break_into_lines()` — Word-level line breaking algorithm
  - `hyphenate_word()` — Mid-word breaking with hyphen insertion
  - `position_multi_line_glyphs()` — Multi-line glyph positioning with proper Y
    offsets and line spacing
  - `apply_text_wrapping()` — Clipping strategy handler integrating with the
    existing cascade
  - `layout_wrapped_text()` — Public API for standalone multi-line text layout
  - Made `measure_text()` and `position_glyphs()` generic over `GlyphSource`

### Tests Added

15 new unit tests covering:

- Line breaking: basic wrapping, single-line fit, max lines, empty text, zero
  width, multiple words
- Hyphenation: basic, too-short words, long words without hyphenation
- Multi-line positioning: glyph Y offsets, line spacing factor, bounds
  calculation
- Performance: 100 labels wrapped in <5ms
- Variant construction: `ClippingStrategy::TextWrapping`

### Test Counts

- **Layout module**: 42 tests (27 existing + 15 new)
- **Full suite**: 1682 passed, 3 pre-existing GPU failures, 4 ignored

---

**Story Created**: 2026-02-26  
**Origin**: GUP-105 follow-up ("Could Have" AC not implemented)
