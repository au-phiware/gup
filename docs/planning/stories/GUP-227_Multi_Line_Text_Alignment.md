# GUP-227: Multi-Line Text Alignment Options

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Advanced Text Layout
and Rendering **Priority**: Low **Story Points**: 3 **Status**: ✅ Complete
**Dependencies**: GUP-199 (Text Wrapping and Multi-Line Layout)

## Problem Statement

The text wrapping system (GUP-199) positions multi-line text using the same
anchor-based alignment for all lines. For paragraph-style text in annotations,
tooltips, and descriptions, per-line alignment options (left, center, right,
justify) would improve readability and visual polish.

## User Story

**As a** data visualization developer **I want** to control how wrapped text
lines are aligned within their container **So that** multi-line labels and
annotations have professional typography

## Acceptance Criteria

- [x] Left, center, and right alignment options for wrapped text
- [x] Justify alignment that distributes extra space between words
- [x] Alignment configurable via `TextStyle` or wrapping parameters
- [x] Works with existing `TextWrapping` clipping strategy
- [x] Works with standalone `layout_wrapped_text()` API

## Technical Tasks

1. Add `TextAlignment` enum (Left, Center, Right, Justify) to text style system
2. Update `position_multi_line_glyphs()` to apply per-line horizontal alignment
3. Implement justify logic distributing extra space proportionally between words
4. Add unit tests for each alignment mode

## Testing Strategy

- Unit tests for each alignment mode with mock font atlas
- Visual verification with multi-line labels in different alignments
- Edge cases: single-word lines (justify should fall back to left), empty lines

## Definition of Done

- [x] All alignment modes implemented and tested
- [x] Integration with both clipping strategy and standalone API
- [x] Performance not degraded by alignment calculations

---

## Implementation Summary

**Completed**: 2025-07-21

### Key Files Changed

- **`src/text.rs`** — Added `TextAlignment` enum with Left, Center, Right, and
  Justify variants.
- **`src/text/style.rs`** — Added `text_alignment` field to `TextStyle` with
  `with_text_alignment()` builder method. Default is `TextAlignment::Left`.
- **`src/text/layout.rs`** — Redesigned `position_multi_line_glyphs()` to:
  - Accept optional `container_width` for alignment reference
  - Compute block width from widest line or container
  - Apply per-line horizontal offset for Left/Center/Right
  - Distribute extra space between words for Justify
  - Fall back to Left alignment for last lines and single-word lines in Justify

### Tests Added

9 new unit tests covering:

- Left alignment default behavior
- Center alignment with offset calculation
- Right alignment with offset calculation
- Justify distributing space between words
- Justify last-line fallback to left
- Justify single-word-line fallback to left
- Alignment with explicit container width
- Alignment combined with center anchor
- Equal-length lines aligning identically

### Test Counts

- **Layout module**: 51 tests (42 existing + 9 new)
- **Full suite**: 1870 passed, 4 ignored

---

**Story Created**: 2025-07-18 **Origin**: GUP-199 follow-up

## Retrospective

**Completed**: 2025-07-21

### Key Technical Learnings

#### Separating Anchor from Alignment

- **Challenge**: The existing `position_multi_line_glyphs()` used `TextAnchor`
  offsets per-line (each line independently anchored by its own width). Adding
  alignment required separating the concepts: anchor positions the whole block,
  alignment positions lines within the block.
- **Solution**: Compute a single block width (max line width or container width),
  apply anchor offset once to determine `block_left`, then apply per-line
  alignment offset within that block.
- **Pattern**: When adding intra-block layout features, anchor the block first,
  then apply layout within the anchored frame. This avoids double-application of
  offsets.

#### Container Width as Alignment Reference

- **Challenge**: For Justify to work correctly, the target width must match the
  wrapping container. Without it, Justify would only fill to the widest line
  (which is often already full, making Justify a no-op).
- **Solution**: Added `container_width: Option<f32>` parameter to
  `position_multi_line_glyphs()`. Callers from wrapping contexts pass their
  `max_width` / `available_width`; standalone callers pass `None` (falls back to
  widest line).
- **Pattern**: Layout functions that need external dimensional context should
  accept optional container dimensions rather than trying to infer them.

### Architectural Decisions

#### TextAlignment on TextStyle (Not ClippingStrategy)

- **Decision**: Added `text_alignment` to `TextStyle` rather than extending
  `ClippingStrategy::TextWrapping` with an alignment field.
- **Reasoning**: Alignment is a typographic property of the text, not a property
  of the clipping/wrapping strategy. It composes naturally with font size, color,
  and other style attributes.
- **Trade-off**: The `ClippingStrategy::TextWrapping` variant doesn't carry its
  own alignment override. Users must set alignment on the style.
- **Future**: If per-strategy alignment overrides are needed, the style's
  `text_alignment` can serve as the default with the strategy providing an
  optional override.

### Development Workflow Insights

- Very well-scoped 3-point story. The existing `position_multi_line_glyphs()`
  provided a clear focal point for the change.
- The `MockFontAtlas` with uniform 9px advances made test assertions
  straightforward — exact pixel offsets could be calculated by hand.
- All 9 new tests passed on first run, validating the design upfront.
- Disk space constraint (NFS partition full) required using `CARGO_TARGET_DIR`
  in `/tmp` for builds. This is a recurring infrastructure issue.

### Follow-up Stories

No new stories identified. The text alignment implementation is self-contained
and the existing GUP-228 (Ellipsis on Last Wrapped Line) is the natural next
text-related enhancement.
