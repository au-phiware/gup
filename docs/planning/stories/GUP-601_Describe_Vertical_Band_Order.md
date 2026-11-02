---
implementer: sonnet
implementer-reason: small formatting change in one pure function
touches: [crates/gup-core/src/describe.rs, crates/gup-core/tests/describe.rs]
---

# GUP-601: Read Vertical Band Axes Top to Bottom

## Story Overview

**Initiative**: Strategic Review 2026-11 (N4 follow-up) **Status**: 📋 Planned
**Created**: 2026-11-02

## Context

A vertical band axis (heatmap y, horizontal bar y) draws its first key at the
top, but its tick labels are in bottom-to-top order, so GUP-592 reads "Y axis: 3
categories, Wed, Tue and Mon" for rows drawn Mon, Tue, Wed from the top.

## User Story

> "As a reader using a screen reader, I want the chart description to read
> naturally, so that I understand it on one hearing."

## Acceptance Criteria

- [ ] A keyed y axis lists its labels top to bottom, matching the reading order
      of the picture.
- [ ] Unit and integration tests (heatmap, barh) assert the order.

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
