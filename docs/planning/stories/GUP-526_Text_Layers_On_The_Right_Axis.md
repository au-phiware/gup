---
implementer: sonnet
implementer-reason:
  one setter and one branch in the text layer's placement, with tests that copy
  GUP-542's
touches: [crates/gup-core/src/text_layer.rs, crates/gup-core/src/plot.rs]
---

# GUP-526: Text Layers on the Right Y Axis

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 💡
New **Created**: 2026-10-26

## Context

GUP-542's `TextLayer` always maps its `y` through the plot's left y scale. A
dual-axis chart (`Plot::y_right`) draws a series against the right axis, but a
label for that series lands at the left scale's position for the same value. The
labels are then wrong, and nothing reports it. A `Selection` binds to the right
axis by encoding through the right scale's handle. A text layer takes closures
and has no such handle, so it needs a setter.

## User Story

> "As a visualization developer with a two-axis chart, I want to label the
> series drawn against the right axis, so that its labels sit on its points."

## Acceptance Criteria

- [ ] `TextLayer::y_axis(YAxis::Right)` (or the name the implementer finds in
      the plot's right-axis API) maps the layer's `y`, `y_key` and `within_y`
      through the plot's right y scale. It is documented, and the default stays
      the left axis.
- [ ] Choosing the right axis on a plot without one is an `Error::config` at
      resolve, naming `Plot::y_right`.
- [ ] Integration test: on a plot whose right axis has a different domain from
      its left, a right-axis label is anchored at the right scale's
      `eval(y) + dy` to within 0.5 px.

## Technical Tasks

- [ ] Pass the right scale (an `Option`) to `AnnotationLayer::runs`; choose the
      scale per layer; add the setter and the error.
- [ ] The test in `crates/gup-core/tests/text_layer.rs`; commit.

## Dependencies

- GUP-542 ✅: the text layer.

## Testing Strategy

- **Integration**: the anchor test above, and the error.

## Success Metrics

- [ ] A dual-axis chart labels both of its series with two `plot.text(..)`
      calls.

## Risk Assessment

- **Low**: one more parameter on a private trait.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
