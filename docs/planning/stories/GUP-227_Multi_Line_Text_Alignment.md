# GUP-227: Multi-Line Text Alignment Options

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Advanced Text Layout
and Rendering **Priority**: Low **Story Points**: 3 **Status**: 🚧 In Progress
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

- [ ] Left, center, and right alignment options for wrapped text
- [ ] Justify alignment that distributes extra space between words
- [ ] Alignment configurable via `TextStyle` or wrapping parameters
- [ ] Works with existing `TextWrapping` clipping strategy
- [ ] Works with standalone `layout_wrapped_text()` API

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

- [ ] All alignment modes implemented and tested
- [ ] Integration with both clipping strategy and standalone API
- [ ] Performance not degraded by alignment calculations

---

**Story Created**: 2025-07-18 **Origin**: GUP-199 follow-up
