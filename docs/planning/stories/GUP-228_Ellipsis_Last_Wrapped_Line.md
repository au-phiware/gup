# GUP-228: Ellipsis on Last Wrapped Line

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Advanced Text Layout
and Rendering **Priority**: Low **Story Points**: 2 **Status**: 🚧 In Progress
**Dependencies**: GUP-199 (Text Wrapping and Multi-Line Layout)

## Problem Statement

When `ClippingStrategy::TextWrapping` truncates text at the `max_lines` limit,
the text simply stops without any visual indication that content continues.
Users cannot tell whether the displayed text is complete or was cut off. Adding
an ellipsis to the last visible line would provide a clear truncation signal,
consistent with the existing `TruncateWithEllipsis` strategy behaviour.

## User Story

**As a** data visualization developer **I want** wrapped text that exceeds
`max_lines` to show an ellipsis on the last line **So that** users can see that
text has been truncated

## Acceptance Criteria

- [ ] Last wrapped line appends configurable ellipsis text (default "...")
- [ ] Ellipsis replaces trailing characters to stay within container width
- [ ] Configurable via a new field on `TextWrapping` variant
- [ ] Word boundary preservation when truncating last line for ellipsis
- [ ] No change when text fits within `max_lines` (no false ellipsis)

## Technical Tasks

1. Add `ellipsis_text: Option<String>` field to `TextWrapping` variant
2. Modify `break_into_lines()` to detect when `max_lines` truncates text
3. Truncate last line to fit ellipsis within available width
4. Reuse existing `adjust_for_word_boundary()` for word-safe truncation
5. Add unit tests

## Testing Strategy

- Unit tests for ellipsis insertion at max_lines boundary
- Tests for text that fits (no ellipsis should appear)
- Tests for very narrow containers where ellipsis itself barely fits
- Performance: no regression from ellipsis logic

## Definition of Done

- [ ] Ellipsis appended to truncated wrapped text
- [ ] All tests passing
- [ ] Backward compatible (existing TextWrapping usage unchanged)

---

**Story Created**: 2025-07-18 **Origin**: GUP-199 follow-up
