# GUP-352: Interactive Tutorial Examples

## Story Overview

**Initiative**: Documentation **Status**: 📋 Planned **Created**: 2025-07-15

## Context

The tutorials delivered in GUP-281 reference existing examples from the GUP-103
suite. These examples often demonstrate more features than a single tutorial
covers, which can be overwhelming for a reader who just finished a tutorial and
wants to see exactly what they built rendered in a window. Dedicated tutorial
examples that match the tutorial code verbatim would provide a smoother learning
experience.

## User Story

> "As a developer following a Gup tutorial, I want to run a single command and
> see exactly the chart described in the tutorial, without extra features or
> complexity that I haven't learned yet."

## Acceptance Criteria

- [ ] A `examples/tutorials/` directory exists with one example per tutorial
      (where visual output is meaningful).
- [ ] Each tutorial example is a self-contained windowed program that renders
      the tutorial's "Full Example" chart.
- [ ] Tutorial documents are updated to reference these dedicated examples.
- [ ] Each example includes a quit mechanism (ESC or window close).

## Technical Tasks

- [ ] Create `examples/tutorials/` directory.
- [ ] Implement `tutorial01_scatter.rs` — windowed scatter chart from
      Tutorial 1.
- [ ] Implement `tutorial04_interactions.rs` — interactive chart from
      Tutorial 4.
- [ ] Implement `tutorial05_streaming.rs` — streaming chart from Tutorial 5.
- [ ] Implement `tutorial06_custom_marks.rs` — custom mark chart from
      Tutorial 6.
- [ ] Update tutorial documents with links to the new examples.
- [ ] Update screenshot sources in `docs/tutorials/README.md`.

## Dependencies

### Prerequisite Stories

- GUP-281: Tutorial and Guide Suite ✅ — The tutorials whose examples to create.
- GUP-103: Comprehensive Chart Examples Suite ✅ — Pattern source for windowed
  examples.

## Testing Strategy

- Each example compiles via `cargo check --examples`.
- Each example runs and renders visible output when launched.

## Success Metrics

- [ ] At least 4 tutorial examples exist and compile.
- [ ] Screenshots from tutorial examples match the tutorials.

## Risk Assessment

- **Low**: Tutorial examples may duplicate code from existing examples. Keep
  them minimal and focused on the tutorial content.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Story status updated to ✅ Complete
