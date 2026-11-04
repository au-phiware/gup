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

**Initiative**: Polish after 0.1 **Status**: 📋 Planned **Created**: 2026-11-04

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

- [ ] The prelude adds `AreaChart`, `BarChart`, `BoxPlotChart`, `HeatmapChart`,
      `HistogramChart`, `LineChart` and `ScatterChart`.
- [ ] The crate root adds `Horizontal`, `Vertical` and `Orientation` (from
      `builder`), beside the existing chart types. They are not added to the
      prelude.
- [ ] A doctest on `pub mod prelude` (in `crates/gup-core/src/lib.rs`) defines
      `fn chart() -> LineChart<(f64, f64)>` with only
      `use gup_core::prelude::*;` and renders it.

### AC2: Users of the long paths

- [ ] Each of the nine files in `examples/gallery/` drops its
      `use gup::builder::..` line; types come from the prelude, and
      `barh_ranked.rs` names `gup::Horizontal`.
- [ ] `crates/gup-core/tests/builders.rs` line 1082 uses `gup_core::Horizontal`;
      `crates/gup-core/tests/bar_value_labels.rs` line 292 uses
      `gup_core::Orientation`.
- [ ] `git grep -n "gup::builder::\|gup_core::builder::" -- ':!docs/planning' ':!crates/gup-culling-lod' ':!crates/gup-core/src'`
      prints nothing.

### AC3: Nothing else moves

- [ ] `cargo test -p gup-core --test compile_fail` (the trybuild suite) passes
      with no `.stderr` change; see Escalation Triggers.
- [ ] `cargo test --test gallery -- --test-threads=1` passes with no golden
      change (the examples draw exactly what they drew).
- [ ] `scripts/check_gallery_sync.sh` passes.

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

- [ ] Edit the two `pub use` lists and the prelude; add the doctest.
- [ ] Edit the nine gallery files and the two tests.
- [ ] Run the AC commands.

## Dependencies

### Prerequisite Stories

- GUP-594: Flagship Gallery Part One ✅ and GUP-595 ✅ — the examples edited.

## Testing Strategy

- **Unit tests**: the prelude doctest.
- **Integration tests**: gallery goldens, trybuild, `cargo check --examples`.

## Success Metrics

- [ ] Zero `gup::builder::` imports outside `gup-core`'s own source.

## Risk Assessment

- **Low**: re-exports only. _Mitigation_: AC3 proves nothing renders or
  diagnoses differently.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, no golden changes (AC3)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code change
- [ ] WASM size: unchanged: re-exports add no code
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
