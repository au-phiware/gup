---
implementer: haiku
implementer-reason:
  re-exports and import edits named file by file; every AC is a grep or a build
touches:
  [
    crates/gup-core/src/lib.rs,
    examples/gallery/,
    crates/gup-core/tests/builders.rs,
    crates/gup-core/tests/bar_value_labels.rs,
  ]
---

# GUP-613: Chart Types in the Prelude

## Story Overview

**Initiative**: Polish after 0.1 **Status**: ✅ Complete **Created**: 2026-11-04

## Context

A function that returns a chart has to name its type, and every gallery example
does (`pub fn chart() -> AreaChart<Output>`). The builder types are re-exported
at the crate root (`gup::ScatterChart`, `crates/gup-core/src/lib.rs` line 181)
but not in the prelude, and the bar orientation `Horizontal` is reachable only
as `gup::builder::Horizontal`. So all nine gallery examples import from
`gup::builder::` next to `use gup::prelude::*`, and two `gup-core` tests spell
`gup_core::builder::Horizontal` and `gup_core::builder::Orientation`. A reader
copying a gallery example learns a module path the README never mentions.

## Read First

- `crates/gup-core/src/lib.rs`: the root `pub use builder::{..}` (lines 181–185)
  and `pub mod prelude` (around line 360).
- `crates/gup-core/src/builder/bar.rs` lines 26–40 (`Orientation`, `Vertical`,
  `Horizontal`).

## User Story

> "As a visualization developer, I want `use gup::prelude::*` to bring in the
> chart types I return from my functions, so that one import line is enough."

## Acceptance Criteria

### AC1: Re-exports

- [x] The prelude adds `AreaChart`, `BarChart`, `BoxPlotChart`, `HeatmapChart`,
      `HistogramChart`, `LineChart` and `ScatterChart`.
- [x] The crate root adds `Horizontal`, `Vertical` and `Orientation` (from
      `builder`), beside the existing chart types. They are not added to the
      prelude.
- [x] A doctest on `pub mod prelude` (in `crates/gup-core/src/lib.rs`) defines
      `fn chart() -> LineChart<(f64, f64)>` with only
      `use gup_core::prelude::*;` and renders it.

### AC2: Users of the long paths

- [x] Each of the nine files in `examples/gallery/` drops its
      `use gup::builder::..` line; types come from the prelude, and
      `barh_ranked.rs` names `gup::Horizontal`.
- [x] `crates/gup-core/tests/builders.rs` line 1082 uses `gup_core::Horizontal`;
      `crates/gup-core/tests/bar_value_labels.rs` line 292 uses
      `gup_core::Orientation`.
- [x] `git grep -n "gup::builder::\|gup_core::builder::" -- ':!docs/planning' ':!crates/gup-culling-lod' ':!crates/gup-core/src'`
      prints nothing.

### AC3: Nothing else moves

- [x] `cargo test -p gup-core --test compile_fail` (the trybuild suite) passes
      with no `.stderr` change; see Escalation Triggers.
- [x] `cargo test --test gallery -- --test-threads=1` passes with no golden
      change (the examples draw exactly what they drew).
- [x] `scripts/check_gallery_sync.sh` passes.

## Decisions

- **Prelude, not root, for the chart types**: they are already at the root; the
  prelude is what examples import.
- **`Horizontal`/`Vertical`/`Orientation` at the root only**: the words are
  generic and a glob import would make them collide with a user's own types more
  easily than the `*Chart` names.
- **No change to the `builder` module**: it stays public; its module docs keep
  the long paths as they are.

## Escalation Triggers

- A trybuild `.stderr` snapshot changes. If the only change is an added
  candidate path in a "consider importing" list, report the diff instead of
  re-blessing; GUP-500/511 worked to stop that churn.
- A glob conflict error appears in any crate in the workspace (a local type
  named like a prelude item is fine; an ambiguity error is not).

## Out of Scope

- Builder `.text(..)` and `pick_row` (GUP-614).
- Rewriting tutorial imports such as `use gup::{Button, Input, ScatterChart}` in
  tutorial 4: valid paths, left as they are.

## Technical Tasks

- [x] Edit the two `pub use` lists and the prelude; add the doctest.
- [x] Edit the nine gallery files and the two tests.
- [x] Run the AC commands.

## Dependencies

### Prerequisite Stories

- GUP-594: Flagship Gallery Part One ✅ and GUP-595 ✅ — the examples edited.

## Testing Strategy

- **Unit tests**: the prelude doctest.
- **Integration tests**: gallery goldens, trybuild, `cargo check --examples`.

## Success Metrics

- [x] Zero `gup::builder::` imports outside `gup-core`'s own source.

## Risk Assessment

- **Low**: re-exports only. _Mitigation_: AC3 proves nothing renders or
  diagnoses differently.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no golden changes (AC3)
- [x] `mask wasm-browser`: not needed, no layout, input or wasm code change
- [x] WASM size: unchanged: re-exports add no code
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/lib.rs`: the prelude gains a line naming the seven chart
  types (`AreaChart`, `BarChart`, `BoxPlotChart`, `HeatmapChart`,
  `HistogramChart`, `LineChart`, `ScatterChart`); the crate root gains
  `pub use builder::{Horizontal, Orientation, Vertical}`; the prelude doc has a
  doctest that defines `fn chart() -> LineChart<(f64, f64)>` with only
  `use gup_core::prelude::*;` and renders it.
- `examples/gallery/` (nine files): the `use gup::builder::..` lines are gone.
  `area_stacked.rs` had no prelude import and now has `use gup::prelude::*;`;
  `barh_ranked.rs` names `gup::Horizontal`.
- `crates/gup-core/tests/builders.rs` and `bar_value_labels.rs` use
  `gup_core::Horizontal` and `gup_core::Orientation`.
- `CHANGELOG.md`: a `[0.1.0]` Added line for the prelude.
- Verification: the AC2 `git grep` prints nothing; `cargo check --examples`;
  `cargo test --doc -p gup -p gup-core` (66 + 4 + 88 + 2 passed);
  `cargo test -p gup -- --test-threads=1` (all pass);
  `cargo test -p gup-core -- --test-threads=1` (all pass, including
  `compile_fail` with no `.stderr` change); `cargo test --test gallery` (11
  passed, no golden change); `scripts/check_gallery_sync.sh` (in sync);
  `mask merge-check` (PASS, dogfood skipped).
- No golden, `.stderr` or `builder` module changed.

## Retrospective

**Completed**: 2026-11-06

### Key Technical Learnings

#### Glob import of chart types

- **Challenge**: Adding names to `gup::prelude` risks a glob ambiguity for users
  who also glob-import a module that exports the same names.
- **Solution**: No glob conflicts in the workspace: `cargo check --examples`,
  all doctests, and the gallery and tutorial suites compile and pass. The
  orientation words stay at the root, as the Decisions say.
- **Pattern**: New `pub use` lines are added beside the existing ones rather
  than edited into existing lists, so the change merges cleanly beside parallel
  stories in the same file.

### Architectural Decisions

#### Prelude for chart types, root for orientations

- **Decision**: As the story's Decisions section.
- **Reasoning**: `Horizontal`, `Vertical` and `Orientation` are generic words
  that a glob import would collide with user types.
- **Trade-off**: The bar orientation still needs a root path
  (`gup::Horizontal`).
- **Future**: Users who want `Horizontal` in scope import it by name.

### Development Workflow Insights

- `area_stacked.rs` had no prelude import, so "drop the builder line" became
  "replace it with the prelude glob" for that one file.
- The full `gup-core` suite takes longer than the Bash timeout; running it in
  the background is the workable approach.

### Follow-up Stories

None. The out-of-scope items are already covered by GUP-614.
