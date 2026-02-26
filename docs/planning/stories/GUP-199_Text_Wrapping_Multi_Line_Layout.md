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

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### GlyphSource Trait for Testability

- **Challenge**: The text layout engine methods were tightly coupled to
  `FontAtlas`, which requires GPU resources and can't be constructed in simple
  unit tests. Existing tests used a `MockFontAtlas` struct but could only test
  inline code, not engine methods.
- **Solution**: Introduced a `GlyphSource` trait with `metrics()` and
  `get_glyph()` methods. `FontAtlas` and `MockFontAtlas` both implement it.
  Changed private helper methods to use `impl GlyphSource` while keeping public
  API signatures accepting `&FontAtlas` (which satisfies the trait).
- **Pattern**: When testing GPU-dependent code, introduce minimal traits at the
  boundary between algorithm and GPU resource. Keep public APIs concrete for
  simplicity; use generics only in internal helpers that benefit from
  testability.

#### Word-Level Line Breaking Algorithm

- **Challenge**: Implementing word wrapping that handles edge cases: empty text,
  zero width, single long words, max line limits, and word-plus-space width
  accounting.
- **Solution**: Iterative word-by-word approach tracking current line width.
  Space width measured from actual glyph data. Long words optionally hyphenated
  with recursive breaking for very long words.
- **Pattern**: For text layout algorithms, measure text width using the same
  glyph metrics that rendering uses (not estimated widths). This ensures
  wrapping decisions match rendered output exactly.

#### Multi-Line Glyph Positioning with Anchoring

- **Challenge**: Multi-line text needs proper vertical anchor adjustment. A
  center-anchored two-line text block should be centered on its total height,
  not just the first line.
- **Solution**: Calculate total height upfront, then apply vertical anchor
  offset to the starting position. Each line applies its own horizontal anchor
  offset independently (allowing different-length lines to align properly).
- **Pattern**: Separate vertical and horizontal anchor calculations for
  multi-line text. Total height drives vertical centering; per-line width drives
  horizontal alignment.

### Architectural Decisions

#### Extending ClippingStrategy Enum

- **Decision**: Added `TextWrapping` as a new variant to the existing
  `ClippingStrategy` enum rather than creating a separate wrapping system.
- **Reasoning**: The existing clipping cascade (primary → fallback strategies)
  naturally supports text wrapping as one option. Users can configure wrapping
  as primary with truncation as fallback, or vice versa.
- **Trade-off**: The `ClippingStrategy` enum grows, but avoids a parallel
  configuration system. The `apply_strategy()` match arm dispatches cleanly.
- **Future**: Additional text strategies (e.g., ellipsis on last wrapped line,
  shrink-to-fit within wrapped layout) can be added as new variants or as
  options on the existing `TextWrapping` variant.

#### Standalone `layout_wrapped_text()` API

- **Decision**: Added a public `layout_wrapped_text()` method in addition to the
  clipping-strategy integration, so users can wrap text without needing viewport
  bounds infrastructure.
- **Reasoning**: Multi-line text layout is useful outside the clipping context
  (e.g., chart titles, annotations, tooltip text).
- **Trade-off**: Slight API surface increase, but provides a clean entry point
  for the most common use case.
- **Future**: Could extend with alignment options (left/center/right/justify)
  per line.

### Development Workflow Insights

- The story was well-scoped: the existing `ClippingStrategy` infrastructure
  provided clear integration points.
- Adding `GlyphSource` trait was the key enabler for thorough unit testing
  without GPU. This pattern should be applied to other GPU-dependent algorithms.
- All 15 new tests passed on first run, validating the algorithm design.
- The `MockFontAtlas` with uniform 9px advance per character made test
  assertions predictable (e.g., "Hello" = 5 × 9 = 45px).

### Follow-up Stories

1. **GUP-227: Multi-Line Text Alignment Options** — Add left/center/right/
   justify alignment for wrapped text lines. Currently all lines use the same
   anchor-based alignment; per-line justification would improve readability for
   paragraph-style text in annotations and tooltips.

2. **GUP-228: Ellipsis on Last Wrapped Line** — When `max_lines` truncates
   wrapped text, append an ellipsis to the last visible line to indicate
   continuation. Currently wrapping simply stops at the line limit without
   visual indication of truncation.
