---
implementer: sonnet
implementer-reason: small formatting change in one pure function
touches: [crates/gup-core/src/describe.rs, crates/gup-core/tests/describe.rs]
---

# GUP-600: Describe Composite Marks and Same-Named Layers

## Story Overview

**Initiative**: Strategic Review 2026-11 (N4 follow-up) **Status**: 📋 Planned
**Created**: 2026-11-02

## Context

GUP-592 lists layers by mark name, so a box plot reads "12 data rows in 4 layers
(rect 2, segment 4, segment 4 and segment 2)". Its rows are summaries, not data
rows, and its layers are an implementation detail.

## User Story

> "As a reader using a screen reader, I want the chart description to read
> naturally, so that I understand it on one hearing."

## Acceptance Criteria

- [ ] Layers with the same mark name are merged in the parenthesis
      (`segment 10`).
- [ ] A box plot reads as its source rows and boxes, e.g. "40 data rows in 2
      boxes", not by internal layers (needs a per-builder description hook).
- [ ] Unit tests for both.

## Technical Tasks

- [ ] Change `describe()` and its unit tests.
- [ ] Update `tests/describe.rs`.

## Dependencies

### Prerequisite Stories

- GUP-592 ✅

## Testing Strategy

Unit tests in `describe.rs` and `tests/describe.rs`; no goldens change.

## Success Metrics

- [ ] The three descriptions in GUP-592's summary read naturally.

## Risk Assessment

- **Low**: wording only. _Mitigation_: `description(text)` overrides it.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] `mask all-fix` clean; `mask wasm-size` within +1,000 B gz
- [ ] Retrospective added
