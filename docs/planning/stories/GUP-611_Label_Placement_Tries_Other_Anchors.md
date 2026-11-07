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

## Decisions

- The setter is
  `TextLayer::fallbacks(impl IntoIterator<Item = (Anchor, f32, f32)>)`. The
  layer's own `anchor` with `dx`/`dy` (including `dx_with`, `dy_with`) stays the
  first candidate; each fallback is tried in order with its own anchor and
  constant px offsets replacing the layer's. With no fallbacks the output equals
  today's byte for byte. With `hide_overlaps(false)` the fallbacks are ignored
  (not measured, not counted in edge room).
- A candidate is viable if it is drawn at all (finite, in reach, has ink, fits
  its `within` span, meets the plot rect) and overlaps no kept box.
- Edge room takes, per label, the largest reach over all candidates (one edge
  per label, not one per candidate).

## Acceptance Criteria

- [x] A setter on `TextLayer` (name decided and recorded in the story's
      Decisions, e.g. `.placements([..])`) takes a ranked list of
      `(Anchor, dx, dy)` candidates. With `hide_overlaps(true)`, each label
      takes the first candidate whose box overlaps no kept box, and is hidden
      only if none fits. Without it the first candidate is used. Documented with
      a doctest.
- [x] Tested on `Layout::texts`: no two kept boxes overlap; a label hidden by
      GUP-527's rule but placeable at its second candidate is kept there; and
      the result is deterministic (row order, then candidate order).
- [x] Edge room (GUP-555) accounts for every candidate's reach, or the story
      records why not.
- [x] Cost in release with 1,000 labels and 4 candidates is measured; it stays
      under +0.3 ms over GUP-527's culling.
- [x] The `gup_core/text_halo` golden, or a new one, shows the gain, read by
      eye.

## Technical Tasks

- [x] Measure each candidate's box once per label (as `Measured` does for the
      single anchor) and test candidates against `Kept`.
- [x] Tests, golden, cost measurement; commit.

## Dependencies

- GUP-527 ✅: `hide_overlaps`, halos and the `Kept` grid.

## Testing Strategy

- **Integration**: the placement rule on `Layout::texts`.
- **Visual**: a golden with placed labels.
- **Performance**: the cost test in `crates/gup-core/tests/text_layer.rs`.

## Success Metrics

- [x] On the 1,000-point scatter, clearly more labels are kept than with
      GUP-527's single anchor, and none overlap.

## Risk Assessment

- **Low**: a contained change to one function. _Mitigation_: with one candidate
  the output equals GUP-527's byte for byte.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] Rendered output verified by eye
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`TextLayer::fallbacks` (`crates/gup-core/src/text_layer.rs`): `Measured` keeps
the ink boxes per fallback anchor (only when culling); `runs` tries candidates
in order against `Kept`, whose cells now hold the boxes inline (faster for
GUP-527's culling too); `reach` and `edges` take the largest reach over all
candidates, one edge per label. Tests in `tests/text_halo.rs` (four, one the
`gup_core/text_fallbacks` golden, new) and the cost metric in
`tests/text_layer.rs`. `gup_core/text_halo` is unchanged (0 pixels differ).

Evidence:

- 1,000-point scatter, 800x600, no halo, 3 fallbacks (right, left, below): 521
  labels kept, 599 with fallbacks (+15%); none overlap.
- Golden read by eye: labels sit above, beside and below their discs, no
  overlaps, all inside the plot.
- Release cost, same session, 3 runs: culling 0.42-0.43 ms, with 4 candidates
  0.63-0.69 ms: +0.20 to +0.26 ms over culling (budget 0.3 ms). The test prints
  the metric and does not assert it (timing).
- `mask merge-check`: PASS.

## Retrospective

**Completed**: 2026-11-07

### Key Technical Learnings

#### Cost was in the edge pass, not the culling

- **Challenge**: first measurement was +0.47 ms.
- **Solution**: `edges` pushed one edge per candidate (4x); folding to one edge
  per label with the maximum reach halved the cost. Hoisting the pixel position
  out of the candidate loop and storing boxes inline in `Kept` cells helped
  `runs` (560 us to 390 us).
- **Pattern**: time the pieces (a temporary `Instant` in `runs`) before tuning.

### Architectural Decisions

#### Fallbacks need `hide_overlaps`

- **Decision**: ignored (not measured, no edge room) without culling.
- **Trade-off**: edge room changes when `hide_overlaps` is toggled.

### Development Workflow Insights

- Gain is modest on a uniform dense scatter (+15%): the greedy row order, not
  the candidate set, dominates. More candidates (diagonals) were not tried.

### Follow-up Stories

None.
