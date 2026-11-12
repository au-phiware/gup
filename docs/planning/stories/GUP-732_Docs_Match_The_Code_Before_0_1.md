---
implementer: sonnet
implementer-reason:
  audit of prose against the code; each fix is a small, evidenced edit
touches:
  [
    README.md,
    CHANGELOG.md,
    docs/,
    examples/README.md,
    examples/INDEX.md,
    src/lib.rs,
    crates/gup-core/src/lib.rs,
  ]
---

# GUP-732: Docs Match The Code Before 0.1

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-12

## Context

0.1 is unpublished, and in the last week many public names and behaviours
changed: `Point` (scale) to `PointScale` and the geometry `Point` in the prelude
(GUP-700); `Window` to `RowWindow` (GUP-701); `Plot::invert` removed for
`Chart::value` and `pixel` (GUP-685, GUP-692); `Linear::include_zero` and
`Chart::x_view`, `y_view`, `view_is_home` (GUP-699); line and area axis
clearance (GUP-705); `Color::from_linear` (GUP-703); `save_svg` and `to_svg`
(GUP-671); new prelude names (GUP-686); hover and focus outlines (GUP-676,
GUP-684); Arrow input (GUP-630); columnar and key columns (GUP-628, GUP-629,
GUP-664); LOD internals (GUP-714, GUP-715). A reader who follows a stale
statement hits a compile error or a behaviour that is not there.

## Read First

- `CHANGELOG.md` `[0.1.0]`.
- Each user-facing document listed under Scope.

## User Story

> "As a first reader of gup, I want every statement in the documents to be true
> of the code, so that nothing I copy or rely on has already changed."

## Acceptance Criteria

- [ ] Every claim in the documents under Scope is checked against the code.
- [ ] Each false or stale claim is fixed minimally; each fix is listed in the
      Implementation Summary with file:line, before to after, and evidence.
- [ ] Each `CHANGELOG.md` `[0.1.0]` bullet is true, and each Known limitation
      still is one.
- [ ] Doctests (`cargo test -p gup --doc`, `-p gup-core --doc`) pass.
- [ ] `mask merge-check` and `mask all-check` pass.

## Scope

README.md, CHANGELOG.md, `docs/*.md` (ACCESSIBILITY, EMBEDDING,
EGUI_INTEGRATION, BEVY_INTEGRATION, RELEASING, INDEX, README),
`docs/tutorials/*.md`, `docs/tour/*.md` (prose only, unless a snippet is wrong),
`examples/README.md`, `examples/INDEX.md`, crate rustdoc in `src/lib.rs` and
`crates/gup-core/src/lib.rs`.

## Out of Scope

- Style rewrites, new features, code changes other than wrong doc comments.
- The public `Selection::lod` (in progress); `selection.rs`, `plot.rs` and LOD
  code (GUP-716).

## Dependencies

GUP-700, GUP-701, GUP-685, GUP-692, GUP-699, GUP-705, GUP-703, GUP-671, GUP-686
(all done).

## Testing Strategy

Doctests; `mask merge-check`; `mask all-check`.

## Definition of Done

- [ ] ACs met; Implementation Summary lists every fix; retrospective written.
