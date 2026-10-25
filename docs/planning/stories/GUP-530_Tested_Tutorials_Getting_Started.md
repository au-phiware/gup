---
implementer: sonnet
implementer-reason:
  docs rewrite against a named API surface, with the include and golden
  mechanism fixed in Decisions; the GUP-489 doctest-include pattern exists
touches: [src/lib.rs]
---

# GUP-530: Tested Tutorials, Part 1: Getting Started and Plots

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: 🚧
In Progress Created**Created**: 2026-10-24

## Context

The six tutorials under `docs/tutorials/` describe the API before the RFC-001
flip (`ComposableShaderFunction`, `wgsl_function!`, `StreamingDataSource`,
`MarkRegistry`, `attr_parallel`, a `tokio` dependency). GUP-499 bannered each
"Out of date". The strategic review (track N1) asks for them to be rewritten as
tested Markdown, and release 0.1's scope is "vocabulary + docs" (owner decision
2, 2026-10-24), so a newcomer's first page must compile.

GUP-489 already compiles `docs/TECHNICAL_APPROACH.md` and
`docs/IMPLEMENTATION_STRATEGY.md` as doctests of the `gup` crate (`src/lib.rs`,
`mod architecture_docs`). This story extends that pattern to the tutorials, sets
the conventions every tutorial story follows (GUP-531, GUP-532, GUP-536), and
rewrites the first two tutorials: getting started with the builders, and plots
and selections (the old "Data Binding").

The tutorials keep their file names so existing links stay valid.

## Read First

- `src/lib.rs` (the whole file, 86 lines: the facade and `architecture_docs`).
- `crates/gup-core/src/builder/mod.rs` lines 1–60 (the builder pattern) and the
  doc examples of `scatter`, `line` and `bar` (`builder/scatter.rs` lines 1–60,
  `builder/line.rs` lines 30–80, `builder/bar.rs` lines 40–75).
- `crates/gup-core/src/plot.rs` lines 80–147 (the `Plot` docs and examples).
- `crates/gup-core/src/marks.rs` lines 1–120 (`Circle` and `Rect` docs).
- `crates/gup-core/src/lib.rs` lines 1–90 (crate docs) and the `prelude`.
- `crates/gup-core/tests/common/vr.rs` lines 1–80 (layout metadata and the
  golden harness) and `crates/gup-core/tests/builders.rs` lines 115–165 (a
  golden check).
- `docs/tutorials/01_getting_started.md` and `02_data_binding.md` as they are
  (to keep the learning path, not the code).

## User Story

> "As a Rust developer new to Gup, I want a getting-started tutorial whose every
> snippet compiles and runs, so that I can copy it and get the chart it shows."

## Acceptance Criteria

### AC1: Tutorials compile as doctests

- [ ] `src/lib.rs` gains `#[cfg(doctest)] mod tutorials` with one
      `#[doc = include_str!("../docs/tutorials/<file>.md")]` unit struct per
      rewritten tutorial (`GettingStarted`, `PlotsAndSelections`), with a doc
      comment naming GUP-530.
- [ ] `cargo test --doc -p gup -- tutorials` runs every `rust` block of both
      files and they pass; the count of doctests run is recorded.
- [ ] No ignored blocks and no hidden lines: `grep -c 'rust,ignore'` prints 0
      for both files, and
      `awk '/^```rust/{f=1;next} /^```/{f=0} f && /^ *# /' docs/tutorials/0[12]_*.md`
      prints nothing.

### AC2: Tutorial 1, Getting Started

- [ ] Covers, in this order: the `Cargo.toml` dependency (Decisions); a scatter
      from a `Vec` of structs (`gup::scatter(rows).x(..).y(..)`); saving a PNG
      (`save_png`) and opening a window (`gup::show`); colour by key and size by
      value; titles and grid; a line chart (`gup::line`, `.color` by key, a
      `None` gap); a bar chart (`gup::bar` with `.group` and `.stack`); and
      "where next" (links to tutorials 2–6).
- [ ] The "Out of date" banner is removed.
- [ ] It embeds two images from the golden store (AC4): the final scatter and
      the grouped bar chart.

### AC3: Tutorial 2, Plots and Selections

- [ ] Retitled "Plots and Selections" (file name unchanged). Covers: dropping
      from a builder to its plot (`into_plot`, `layer_mut`, `layers_mut`);
      `Plot::new` with `circles`, `line`, `rects`, `segments` layers sharing
      axes; `plot.x(..)`/`plot.y(..)` returning `ScaleRef` handles;
      `.attr(Circle::RADIUS, Px(4.0))`, `x.encode(..)`, `encode_key` on a
      `Band`; a colour scale (`Sequential::viridis().encode(..)`) with its
      legend; a compile-error example (`rust,compile_fail`) showing that a
      colour scale cannot feed `Circle::RADIUS`; `Plot::y_right`; and `Retain`
      in one paragraph.
- [ ] The "Out of date" banner is removed. It embeds one image (AC4): a plot
      with a scatter layer, a line layer and a legend.

### AC4: Tutorial images are goldens

- [ ] A new root integration test `tests/tutorials.rs` renders each embedded
      image from the same calls as the tutorial's block (the test function's doc
      comment names the tutorial and section) and checks it against
      `tests/golden/tutorials/<name>.png` through `gup-visual-regression`'s
      `Harness` (`GUP_BLESS=1` to bless), at 640×400.
- [ ] Goldens `tutorials/01_scatter`, `tutorials/01_grouped_bars` and
      `tutorials/02_layers` exist; the worker reads each PNG and records what it
      shows (title, tick labels, legend, marks inside the plot area).
- [ ] The tutorials link the images as
      `../../tests/golden/tutorials/<name>.png`. The stale
      `docs/tutorials/assets/tutorial01_scatter.png` is deleted (the other three
      stay until GUP-531 and GUP-532 replace them).

### AC5: Index

- [ ] `docs/tutorials/README.md`'s table gives the new titles and one-line
      descriptions of tutorials 1 and 2, its "Screenshot Sources" section is
      replaced by one paragraph on `tests/tutorials.rs` and `GUP_BLESS`, and its
      prerequisites say "Rust 1.85+ (edition 2024)" and drop `tokio`. The banner
      on the README stays until GUP-532.

## Decisions

- **Doctests of the `gup` crate**, not `gup-core`: tutorials teach the crate a
  user depends on (`use gup::prelude::*;`), as the README does.
- **Every `rust` block compiles and runs** unless it opens a window or writes a
  file, which is `rust,no_run`. No `rust,ignore`. No hidden `#`-prefixed lines:
  GitHub renders them. A block that uses `?` is a complete program
  (`fn main() -> gup::Result<()> { … Ok(()) }`); a short snippet without `?` may
  rely on rustdoc's implicit `main`. Repeating a few lines between steps is
  fine.
- **Rendering in a test block** uses `Context::shared()?` and
  `chart.render(&cx, 640, 400)?` with one visible assertion
  (`assert_eq!(image.dimensions(), (640, 400));`). Writing PNGs is shown with
  `save_png(&cx, "scatter.png", 640, 400)` in a `no_run` block.
- **Dependency line** until release 0.1 (track N3 changes it):
  `gup = { git = "https://github.com/au-phiware/gup" }`, with one sentence that
  0.1 is not on crates.io yet.
- **Images are goldens, not screenshots**: `tests/tutorials.rs` duplicates the
  tutorial's chart calls; a stale image therefore fails CI instead of drifting.
  The test builds metadata from `Layout` with a local 20-line copy of
  `crates/gup-core/tests/common/vr.rs`'s `layout_metadata` (the root crate
  cannot import gup-core's test modules).
- **Data in examples** is inline `Vec`s of small structs (no CSV, no `rand`):
  the doctests must not need dev-dependencies the `gup` crate lacks.
- **Mention what is missing honestly**: tutorial 1's "other chart types"
  paragraph lists scatter, line and bar only, and says area, histogram, heatmap
  and annotations arrive in track N2 (GUP-536 rewrites it once they exist).

## Escalation Triggers

- A doctest that renders fails under the shared build directory with "multiple
  versions of crate wgpu" (GUP-500 did not reproduce it): stop and report the
  exact command and output.
- An API a tutorial needs does not exist or behaves differently from its docs
  (for example `layers_mut` on a scatter): report it; do not work around it in
  the tutorial.
- `cargo test --doc -p gup` takes more than 3 minutes longer than before
  (edition 2024 merges doctests, so it should not).

## Out of Scope

- Tutorials 3–6 (GUP-531, GUP-532) and the chart tour (GUP-536).
- The README (GUP-534) and the integration guides (GUP-535).
- Any API change. Gaps found go in the retrospective as follow-up stories.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] Add `mod tutorials` to `src/lib.rs` with tutorial 1 only; rewrite tutorial
      1; run `cargo test --doc -p gup -- tutorials`; commit.
- [ ] Add `tests/tutorials.rs` with the two tutorial-1 images; bless; read the
      PNGs; commit.
- [ ] Rewrite tutorial 2, add it to `mod tutorials`, add its image; commit.
- [ ] Update `docs/tutorials/README.md`; delete the stale asset.
- [ ] Run the DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-489: Revise architecture docs ✅ — the doctest-include pattern.
- GUP-498: Current `gup-core` crate docs ✅ — the API descriptions to match.
- GUP-537: owned category keys 📋 — the tutorial's colour-by-key, group and
  stack calls use owned keys (`|r| r.region`, `r.name.clone()`).

### Enables Stories

- GUP-531, GUP-532: follow this story's conventions and extend `mod tutorials`.
- GUP-534: the README links the rewritten tutorials.
- GUP-536: the chart tour follows the same conventions.

## Testing Strategy

- **Doctests**: `cargo test --doc -p gup -- tutorials`.
- **Integration**: `cargo test -p gup --test tutorials -- --test-threads=1`.
- **Visual validation**: the three goldens, read by eye.

## Success Metrics

- [ ] Every `rust` block in tutorials 1 and 2 is compiled by CI.
- [ ] A newcomer can copy tutorial 1's final program into a new crate and get
      the image shown.

## Risk Assessment

- **Low**: doctest count slows CI. _Mitigation_: edition 2024 merges doctests
  into one binary; record the time before and after.
- **Low**: the duplicated chart code in `tests/tutorials.rs` drifts from the
  tutorial text. _Mitigation_: each test's doc comment names its block; the
  reviewer diffs them.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the three tutorial goldens)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm change
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
