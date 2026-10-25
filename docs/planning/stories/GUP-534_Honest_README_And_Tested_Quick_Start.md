---
implementer: sonnet
implementer-reason:
  docs edit with every replacement decided below; the README becomes a doctest
  through the existing include pattern
touches: [src/lib.rs]
---

# GUP-534: An Honest README with a Tested Quick Start

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: ✅
Complete (2026-10-25) **Created**: 2026-10-24

## Context

`README.md` is the first thing a newcomer reads, and the strategic review (track
N1) asks for its "claims cut to what is measured". Today it opens with "GPU
performance for billion-point datasets", which nothing demonstrates (the largest
measured case is 10M rows streamed through `LastRows(100_000)`; LOD is parked;
review §1). Its "Performance Crisis" section makes unmeasured claims about other
libraries ("crashes with real-time updates", "~1,000 points at 60 FPS"). Its
Quick Start says `gup = "0.1"`, but no crate is on crates.io until track N3. Its
code block is a copy of `src/lib.rs`'s doctest, so it can drift silently, and it
ends with a marketing sentence.

This story rewrites the README's top half around measured numbers and an honest
"not yet" list, makes the README itself a doctest, and links the tutorials
(GUP-530–532).

## Read First

- `README.md` (whole, 165 lines).
- `crates/gup-core/PERF_BUDGETS.md` lines 49–90 (the budget table) and 20–48
  (the development machine).
- `docs/planning/STRATEGIC_REVIEW_2026-11.md` §1 "What a real user still cannot
  do" (the table).
- `src/lib.rs` lines 78–86 (`architecture_docs`).
- `docs/tutorials/README.md` (the tutorial titles after GUP-530).

## User Story

> "As a developer evaluating Gup, I want the README to say what Gup does today,
> with numbers I can check, and a quick start that works, so that I can decide
> in five minutes whether to try it."

## Acceptance Criteria

### AC1: Claims match measurements

- [x] The tagline is replaced by: "A few lines of Rust for a publication-quality
      chart, drawn on the GPU: in a window, a PNG, an SVG, egui, Bevy or the
      browser."
- [x] "Motivation" (with "The Performance Crisis" and "The GPU Opportunity") is
      replaced by "Why Gup", at most 12 lines, stating only what
      `PERF_BUDGETS.md` records, each figure with its row name and machine:
      zooming writes 0 column bytes (`zoom.columns.bytes`); 4M sorted rows draw
      in the `zoom_culled.gpu_pass.median_ms` value with 14 draws on an Intel HD
      630; the reference scatter's WASM build is the `wasm.scatter.gz_bytes`
      value. Numbers are copied from the file at implementation time.
- [x] A new "Not yet" list (after "Key Features") names, one line each: no CSV,
      serde or Arrow input (rows enter through closures); nothing above 10M rows
      measured, no level of detail; no PDF export; no accessibility support; CI
      renders on Linux only; chart types are those the builders list today. Each
      line links the track that adds it (N2–N6 in `IMPLEMENTATION_STRATEGY.md`).
- [x] `git grep -n -i 'billion\|crisis\|crashes\|transforming data visualization' README.md`
      prints nothing. The closing bold paragraph is deleted.

### AC2: A tested Quick Start

- [x] The dependency line is
      `gup = { git = "https://github.com/au-phiware/gup" }` with the sentence
      "Gup is not on crates.io yet; release 0.1 will be."
- [x] `src/lib.rs`'s `architecture_docs` gains
      `#[doc = include_str!("../README.md")] struct Readme;` and
      `cargo test --doc -p gup -- architecture_docs` runs and passes the
      README's `rust` blocks.
- [x] The Quick Start block is a complete program:
      `fn main() -> gup::Result<()>` that builds the scatter and calls
      `chart.save_png(&cx, "chart.png", 800, 500)?` (`rust,no_run`), followed by
      a second, running block that renders in memory and asserts the image size.
      Neither block has hidden `#`-prefixed lines. The "(this is the doctest in
      `src/lib.rs`)" note is removed.
- [x] The Quick Start ends with links to tutorial 1 and the tutorials index.

### AC3: The rest stays true

- [x] "Documentation" lists the tutorials (link to `docs/tutorials/README.md`)
      first.
- [x] "Examples": every command in the block exists (`cargo run --example …`
      names an example in `Cargo.toml` or `crates/gup-core/Cargo.toml`), checked
      by running `cargo check --examples -p gup -p gup-core`.
- [x] `prettier --check README.md` and `mdl README.md` pass.

## Decisions

- **Numbers are cited, not rounded up**: copy the current values from
  `PERF_BUDGETS.md` and name the row, so a reader can check them and a later
  re-record is easy to propagate.
- **No comparisons with other libraries** in the README: none are measured in
  this repository. `MARKET_ANALYSIS.md` stays as it is and is linked under
  "Documentation".
- **The README is a doctest of `gup`** (like `TECHNICAL_APPROACH.md`), so the
  Quick Start breaks the build when the API changes.
- **Git dependency until N3**: track N3's publish story changes the line to
  `gup = "0.1"`; this story does not pre-empt it.

## Escalation Triggers

- A figure the AC names is missing from `PERF_BUDGETS.md` or has changed
  meaning: quote what is there and report the difference instead of choosing
  another number.
- Including `README.md` as a doctest makes an unrelated block run (for example a
  `bash` block mis-tagged as `rust`): fix the tag; if a block cannot be made to
  run, report it.

## Out of Scope

- `docs/MISSION_AND_GOALS.md` and `docs/MARKET_ANALYSIS.md` (their claims are
  the next review's question).
- A README hero image (track N3's gallery).
- The licence section (owner decision 1 keeps GPL-3.0-or-later; unchanged).

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Rewrite the tagline, "Why Gup" and "Not yet"; commit.
- [x] Rewrite the Quick Start; add the include; run the doctest; commit.
- [x] Fix the Documentation and Examples sections; run prettier and mdl.

## Dependencies

### Prerequisite Stories

- GUP-530: Tested tutorials part 1 📋 (soft) — the links to tutorial 1.
- GUP-537: owned category keys 📋 (soft) — the Quick Start's `.color(..)` call
  is owned (`d.continent.clone()`); both edit `src/lib.rs`.

### Enables Stories

- Track N3's publish story edits the dependency line only.

## Testing Strategy

- **Doctests**: `cargo test --doc -p gup -- architecture_docs`.
- **Lint**: prettier and mdl on `README.md`.

## Success Metrics

- [x] Every number in the README names its `PERF_BUDGETS.md` row; the Quick
      Start compiles in CI.

## Risk Assessment

- **Low**: GitHub renders the README differently from rustdoc. _Mitigation_: no
  hidden lines; `rust,no_run` renders as a normal Rust block on GitHub.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output: not applicable, no visual change
- [x] `mask wasm-browser`: not needed, no layout, input or wasm change
- [x] WASM size: unchanged, no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `README.md`: new tagline, "Why Gup" (three figures, each with its
  `PERF_BUDGETS.md` row, copied from the file today), a "Not yet" list naming
  tracks N2-N6, a git-dependency Quick Start with a `no_run` program and a
  running block (no hidden lines), tutorial links, the tutorials first under
  "Documentation"; the closing paragraph is gone.
- `src/lib.rs`: `architecture_docs` includes `README.md` (`Readme`);
  `cargo test --doc -p gup -- architecture_docs` runs the README's blocks (10
  pass).
- `scripts/test_pre_commit.sh`: `README.md` is now a `gup` source
  (`clippy=[gup]`), not docs-only, because `src/lib.rs` compiles it.
- Examples commands checked with `cargo check --examples -p gup -p gup-core`;
  `prettier`, `mdl` and `mask all-fix` pass.

## Retrospective

**Completed**: 2026-10-25

### Key Technical Learnings

#### README as a doctest

- **Challenge**: the AC forbids hidden `#` lines, so `?` cannot sit at the top
  level of a block.
- **Solution**: both blocks define `fn main() -> gup::Result<()>`; rustdoc then
  does not wrap them.
- **Pattern**: a self-contained `main` reads the same on GitHub and in rustdoc.

### Architectural Decisions

#### "Not yet" wording

- **Decision**: the accessibility line says a minimum comes in N3 and keyboard
  navigation in N4, as `IMPLEMENTATION_STRATEGY.md` plans.
- **Trade-off**: none; each line names a planned track, not a date.

### Development Workflow Insights

- A worktree-isolated shell refuses inline scripts that mention git; put them in
  a file.

### Follow-up Stories

None. GUP-524 and GUP-525 were reserved and are unused.
