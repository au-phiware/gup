---
implementer: sonnet
implementer-reason:
  extends GUP-527's culling grid inside one function; the candidate order and
  its cost are small, measurable choices
touches: [crates/gup-core/src/text_layer.rs, crates/gup-core/tests/text_halo.rs]
---

# GUP-611: Label Placement Tries Other Anchors Before Hiding a Label

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress New **Created**: 2026-11-02

## Context

GUP-527's `TextLayer::hide_overlaps(true)` keeps a label only if its box
overlaps no label kept before it, at the layer's one anchor and offset. On the
1,000-point scatter of `resolve_cost_of_a_thousand_labels` (800×600) about half
the labels are kept. Many of the hidden ones would fit if placed to the right,
left or below their point instead of above it. Label placement that tries a
short ranked list of positions is the usual next step after greedy culling, and
it reuses GUP-527's grid of kept boxes (`Kept` in `text_layer.rs`).

## User Story

> "As a visualization developer labelling a dense scatter, I want a label that
> collides above its point to try beside or below it before it is hidden, so
> that more points keep their labels."

## Acceptance Criteria

- [ ] A setter on `TextLayer` (name decided and recorded in the story's
      Decisions, e.g. `.placements([..])`) takes a ranked list of
      `(Anchor, dx, dy)` candidates. With `hide_overlaps(true)`, each label
      takes the first candidate whose box overlaps no kept box, and is hidden
      only if none fits. Without it the first candidate is used. Documented with
      a doctest.
- [ ] Tested on `Layout::texts`: no two kept boxes overlap; a label hidden by
      GUP-527's rule but placeable at its second candidate is kept there; and
      the result is deterministic (row order, then candidate order).
- [ ] Edge room (GUP-555) accounts for every candidate's reach, or the story
      records why not.
- [ ] Cost in release with 1,000 labels and 4 candidates is measured; it stays
      under +0.3 ms over GUP-527's culling.
- [ ] The `gup_core/text_halo` golden, or a new one, shows the gain, read by
      eye.

## Technical Tasks

- [ ] Measure each candidate's box once per label (as `Measured` does for the
      single anchor) and test candidates against `Kept`.
- [ ] Tests, golden, cost measurement; commit.

## Dependencies

- GUP-527 ✅: `hide_overlaps`, halos and the `Kept` grid.

## Testing Strategy

- **Integration**: the placement rule on `Layout::texts`.
- **Visual**: a golden with placed labels.
- **Performance**: the cost test in `crates/gup-core/tests/text_layer.rs`.

## Success Metrics

- [ ] On the 1,000-point scatter, clearly more labels are kept than with
      GUP-527's single anchor, and none overlap.

## Risk Assessment

- **Low**: a contained change to one function. _Mitigation_: with one candidate
  the output equals GUP-527's byte for byte.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Rendered output verified by eye
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
