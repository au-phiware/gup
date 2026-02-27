# GUP-228: Ellipsis on Last Wrapped Line

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Advanced Text Layout
and Rendering **Priority**: Low **Story Points**: 2 **Status**: ✅ Complete
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

- [x] Last wrapped line appends configurable ellipsis text (default "...")
- [x] Ellipsis replaces trailing characters to stay within container width
- [x] Configurable via a new field on `TextWrapping` variant
- [x] Word boundary preservation when truncating last line for ellipsis
- [x] No change when text fits within `max_lines` (no false ellipsis)

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

- [x] Ellipsis appended to truncated wrapped text
- [x] All tests passing
- [x] Backward compatible (existing TextWrapping usage unchanged)

---

## Implementation Summary

**Completed**: 2025-02-28

### What Was Implemented

- Added `ellipsis_text: Option<String>` field to `ClippingStrategy::TextWrapping`
- New `append_ellipsis_to_last_line()` method that truncates the last wrapped
  line and appends the ellipsis string, staying within the container width
- Truncation detection in `apply_text_wrapping()`: compares wrapped word count
  against total word count to determine if text was actually truncated
- Word boundary preservation reusing the existing `adjust_for_word_boundary()`
  helper
- Graceful fallback for very narrow containers (line becomes just the ellipsis)

### Key Files Changed

- `src/text/layout.rs` — all changes in one file: enum field, strategy
  pass-through, helper method, and tests

### Test Count

- **9 new tests** added (7 ellipsis-specific + 1 variant construction + 1
  performance)
- **60 total** text layout tests pass
- **All project tests pass** including doctests

**Story Created**: 2025-07-18 **Origin**: GUP-199 follow-up

## Retrospective

**Completed**: 2025-02-28

### Key Technical Learnings

#### Reusing Existing Truncation Patterns

- **Challenge**: Needed to fit an ellipsis into a last line that may already be
  at or near the container width limit
- **Solution**: Adapted the binary search + word boundary adjustment pattern
  already used by `TruncateWithEllipsis` into a standalone
  `append_ellipsis_to_last_line()` helper
- **Pattern**: When adding a feature similar to an existing one, extract the
  shared algorithm (binary search for fit + `adjust_for_word_boundary`) rather
  than duplicating code

#### Truncation Detection via Word Count Comparison

- **Challenge**: `break_into_lines()` returns lines but doesn't indicate whether
  text was truncated or naturally ended
- **Solution**: Compare total words in the original text against words covered by
  the wrapped lines. If fewer words are covered AND lines reached `max_lines`,
  text was truncated
- **Pattern**: Post-hoc truncation detection is simpler than modifying the
  wrapping function to return a flag, keeping the existing API stable

### Architectural Decisions

#### Option<String> Rather Than Default Ellipsis

- **Decision**: Used `ellipsis_text: Option<String>` defaulting to `None`
  instead of a non-optional field defaulting to `"..."`
- **Reasoning**: Backward compatibility — all existing code constructs
  `TextWrapping` without this field, so `None` preserves the previous behaviour
  (no ellipsis). Users opt in explicitly.
- **Trade-off**: Requires `Some("...".to_string())` at the call site instead of
  getting ellipsis by default
- **Future**: Could add a convenience constructor or builder method to simplify
  common usage

#### Ellipsis Logic in `apply_text_wrapping` Not `break_into_lines`

- **Decision**: Ellipsis is applied after line breaking, not during
- **Reasoning**: Keeps `break_into_lines` focused on line splitting, and the
  ellipsis is an output formatting concern. This preserves the function for reuse
  in contexts that don't want ellipsis
- **Trade-off**: Requires a separate truncation detection step
- **Future**: Clean separation makes it easy to add other post-processing (e.g.
  "show more" link text)

### Development Workflow Insights

- Disk space was the main bottleneck — the ZFS dataset for `/home/corin/src` was
  at 100%, preventing `sed` temp files and large link steps. Resolved by
  symlinking `target/` to `/tmp/gup-target`.
- The story was small and self-contained; all changes fit in a single file
  (`src/text/layout.rs`), which made iteration fast.
- The existing `MockFontAtlas` test helper with ~9px per character made it easy
  to reason about widths in test assertions.

