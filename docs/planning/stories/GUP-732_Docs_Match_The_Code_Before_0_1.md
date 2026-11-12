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

- [x] Every claim in the documents under Scope is checked against the code.
- [x] Each false or stale claim is fixed minimally; each fix is listed in the
      Implementation Summary with file:line, before to after, and evidence.
- [x] Each `CHANGELOG.md` `[0.1.0]` bullet is true, and each Known limitation
      still is one.
- [x] Doctests (`cargo test -p gup --doc`, `-p gup-core --doc`) pass.
- [x] `mask merge-check` and `mask all-check` pass.

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

- [x] ACs met; Implementation Summary lists every fix; retrospective written.

## Implementation Summary

Audited every document under Scope against the code (grep, the doctests, the
PERF_BUDGETS table, the Cargo manifests). Twelve fixes in ten files; the rest
was true. Doctests: `cargo test -p gup --doc` and `-p gup-core --doc` pass.

| #   | File:line                            | Before to after                                                                                                                                                                                  | Evidence                                                                                       |
| --- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| 1   | `docs/README.md:~81`                 | A banner said the tutorials "are out of date ... do not compile against `gup` today"; removed                                                                                                    | The seven tutorials are doctests of `gup` (`src/lib.rs` `mod tutorials`); CHANGELOG says so    |
| 2   | `docs/README.md:~88`                 | "Six step-by-step tutorials" to "Seven", and tutorial 7 added to the list                                                                                                                        | `docs/tutorials/07_chart_types.md` exists                                                      |
| 3   | `docs/README.md:~91`                 | Tutorial 2 "Data Binding: bind custom structs with `Selection<T, M>`" to "Plots and Selections: layers of marks on shared scales"                                                                | The tutorial's title (`02_data_binding.md:1`)                                                  |
| 4   | `README.md:26`                       | WASM scatter "about 360 KB gzipped" to "about 385 KB"                                                                                                                                            | `PERF_BUDGETS.md` `wasm.scatter.gz_bytes` 384,562                                              |
| 5   | `docs/tour/11_big_data.md:85`        | "Not yet" listed key columns (GUP-629) and Arrow batches (GUP-630) as planned; now says they exist (`KeyColumn`, `Columns::from_arrow`) and keeps GUP-632 and a drawn level of detail as not yet | INDEX: GUP-629, GUP-630, GUP-631 complete; `columns_arrow.rs`; GUP-632 planned                 |
| 6   | `docs/EGUI_INTEGRATION.md:5,74`      | "forwards pointer input" to pointer and keyboard; Interaction section now says a primary press gives egui focus and keys are forwarded while focused                                             | `gup-egui/src/widget.rs:188-224`, `input.rs`; CHANGELOG                                        |
| 7   | `docs/RELEASING.md:48`               | Step 8 said "`docs/EMBEDDING.md` if it exists (GUP-702)" to: it holds no dependency block                                                                                                        | The file exists and has no dependency line                                                     |
| 8   | `CHANGELOG.md:96`                    | Marks list "`Circle`, `Rect`, `Line` and `Area`" to include `Segment`                                                                                                                            | `crates/gup-core/src/lib.rs` marks list; tour 12                                               |
| 9   | `crates/gup-core/src/lib.rs:31`      | Crate rustdoc listed seven builders, no `boxplot`; now eight                                                                                                                                     | `pub use builder::boxplot`                                                                     |
| 10  | `docs/ACCESSIBILITY.md:121`          | The focus ring was described as always a ring around the row; now says a bar, bin, cell or box is outlined whole                                                                                 | `plot.rs` `ring_item` and `outline` (GUP-676, GUP-684); CHANGELOG                              |
| 11  | `docs/tour/08_interaction.md:50`     | Hover "thin ring on the hovered point" now notes a bar, bin, cell or box is outlined whole                                                                                                       | The same                                                                                       |
| 12  | `docs/tour/04_axes_and_labels.md:57` | "the orange line (temperature)" to "vermillion"                                                                                                                                                  | The snippet uses `OKABE_ITO_DISTINCT[1]`, `#D55E00`; the palette table lists Orange separately |

Checked and left as is: every `[0.1.0]` bullet (each named item found in the
code; Known limitations still hold), README Requirements and Quick Start,
EMBEDDING.md (doctested), BEVY_INTEGRATION.md, docs/INDEX.md, tutorials
(doctested; prose spot-checked against `RowWindow`, `PointScale`,
`pixel`/`value`, `include_zero`, `view_is_home`), the other tour pages,
examples/README.md and examples/INDEX.md (every example exists), `src/lib.rs`.

## Retrospective

**Completed**: 2026-11-12

### Key Technical Learnings

#### Stale prose hides outside the doctests

- **Challenge**: Renames break compiled snippets loudly, so those were already
  swept; the false statements were prose about what exists or is planned.
- **Solution**: Grep for status words ("not yet", "planned", "out of date", "if
  it exists") and check each against the INDEX and the code.
- **Pattern**: "Not yet" lists and "if it exists" conditionals go stale
  silently; a test comparing them with story status would catch them.

### Architectural Decisions

#### Minimal edits

- **Decision**: Fix only false statements; leave style and structure.
- **Reasoning**: The story forbids rewrites and features.
- **Trade-off**: Some pages still repeat the same facts in several places.
- **Future**: None.

### Development Workflow Insights

- Not verified: tour 9's "Right focuses the first row in view" against the
  accessibility guide's "first row of the first layer", and the CHANGELOG's
  key-column timing figures (30-45%), which come from GUP-629's own
  measurements.

### Follow-up Stories

None.
