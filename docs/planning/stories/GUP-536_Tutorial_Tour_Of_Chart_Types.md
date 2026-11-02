---
implementer: sonnet
implementer-reason:
  a docs tutorial over builders that exist by then, following GUP-530's
  conventions; the section list and images are fixed below
touches: [src/lib.rs]
---

# GUP-536: Tutorial 7: A Tour of Chart Types

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: 🚧
In Progress Planned **Created**: 2026-10-24

## Context

Release 0.1 is "the common chart vocabulary, tested tutorials and a gallery"
(owner decision 2). Tutorials 1–6 (GUP-530–532) teach the model; track N2 adds
the vocabulary (area, histogram, heatmap, box plot, horizontal and labelled
bars, curves, annotations, subtitle, dark theme). A newcomer needs one page that
shows each chart type with the few lines that make it, so they can find "my
chart" and copy it. Tutorial 1's "other chart types" paragraph (GUP-530)
promised this page.

## Read First

- GUP-530's Decisions (tested Markdown, goldens in `tests/tutorials.rs`).
- The rustdoc of each builder: `scatter`, `line` (with `curve`), `bar`, `barh`
  (`value_labels`), `area`, `histogram`, `heatmap`, `boxplot`, and `Plot::text`,
  `subtitle`, `Theme::dark` (whichever exist; see Dependencies).
- `docs/tutorials/01_getting_started.md` ("Other chart types" paragraph).

## User Story

> "As a newcomer, I want one page with every chart type Gup draws and the code
> for each, so that I can start from the chart closest to mine."

## Acceptance Criteria

### AC1: The tutorial

- [x] `docs/tutorials/07_chart_types.md`, "A Tour of Chart Types", has one
      section per chart in this order: scatter, line (with a step or monotone
      curve when GUP-550/551 are merged), bar (grouped, with value labels),
      horizontal bar, stacked area, histogram, heatmap, box plot (when GUP-554
      is merged), annotations with `Plot::text`, and one chart in
      `Theme::dark()` (when GUP-553 is merged). Each section is a short
      paragraph on when to use the chart, one tested `rust` block of at most 20
      lines, and its image.
- [x] Every image is a golden `tutorials/07_<chart>` in `tests/tutorials.rs`,
      read by eye and described.
- [x] `src/lib.rs`'s `mod tutorials` gains `ChartTypes`;
      `cargo test --doc -p gup -- tutorials` passes; the GUP-530 `awk` and
      `grep` checks print nothing for the file.

### AC2: Links

- [x] Tutorial 1's "other chart types" paragraph links this page instead of
      listing what N2 will add; `docs/tutorials/README.md` lists tutorial 7;
      `examples/README.md`'s "More builders … return in track N2" sentence links
      this page and names the builders that exist.

## Decisions

- **Follow GUP-530's conventions**; data inline, deterministic.
- **Skip what is not merged**: a section whose builder story has not merged is
  left out (listed in the Implementation Summary), not stubbed.
- **No new API**: gaps found go to the retrospective as follow-up stories.

## Escalation Triggers

- A builder's documented usage does not compile or renders wrongly (the section
  is the first external use of the full vocabulary): report it; do not work
  around it in the tutorial.

## Out of Scope

- The flagship gallery and README hero image (track N3).

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Sections and goldens, a few at a time; commit after each three.
- [x] Links; DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-530: Tested tutorials part 1 📋.
- GUP-537: owned category keys 📋 — every key closure on the page is owned.
- GUP-542 📋, GUP-544 📋, GUP-547 📋, GUP-548 📋, GUP-549 📋 — the chart types
  the page must show.
- GUP-550 📋, GUP-551 📋, GUP-552 📋, GUP-553 📋, GUP-554 📋 (soft) — shown if
  merged.

### Enables Stories

- Track N3's gallery can start from these images.

## Testing Strategy

- **Doctests**: `cargo test --doc -p gup -- tutorials`.
- **Visual validation**: every `tutorials/07_*` golden, read by eye.

## Success Metrics

- [x] Every builder in the crate has a tested example on one page.

## Risk Assessment

- **Low**: the page goes stale as builders change. _Mitigation_: it is a
  doctest; a breaking change fails CI.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the tutorial goldens)
- [x] `mask wasm-browser`: not needed, docs only
- [x] WASM size: unchanged, no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `docs/tutorials/07_chart_types.md`: a "how to choose" table and ten sections
  (scatter, line with `Curve::MonotoneX` and a note on `StepAfter`, grouped bar
  with `value_labels` and `group_padding`, `barh` with owned keys, stacked area,
  histogram, heatmap, box plot, `Plot::text` annotations, dark theme with
  `subtitle`). Each block is a whole program of at most 20 lines.
- `tests/tutorials.rs`: ten `tutorial_7_*` goldens under
  `tests/golden/tutorials/07_*.png`; `check_on` takes a background so the dark
  chart's confinement check uses the theme's colour.
- Links: tutorial 1 ("Other Chart Types", "Where Next"), the tutorials README
  (row and learning path), `examples/README.md`; `ChartTypes` in `src/lib.rs`.
- Nothing skipped: every builder had merged. Sketch differences: the builders
  have no `.text`; annotations go through `chart.plot_mut().text(rows)`, and
  `radius` takes `f32`, not `Px`.
- Evidence: `cargo test --doc -p gup` 66 + 4 pass (10 are ChartTypes), tutorials
  test 17 pass, `mask merge-check` PASS, `awk`/`grep` checks print nothing.

## Retrospective

**Completed**: 2026-11-02

### Key Technical Learnings

#### Doctests as programs

- **Challenge**: 20 lines per block including `fn main` and no hidden lines.
- **Solution**: generate rows with iterators/formulas, render with a bare
  `chart.render(..)?;`, put short setters on shared lines.
- **Pattern**: write the test body first, view the golden, then copy the block.

### Architectural Decisions

#### Dark golden background

- **Decision**: `check_on(name, chart, background)`; `check` keeps white.
- **Trade-off**: none; the harness already took a background.

### Development Workflow Insights

- A scripted `index()` replace hit tutorial 1's similarly named test; the
  compile error caught it. Match on unique anchors.
- `group_padding` defaults to 0.1, so showing it needed a different value.

### Follow-up Stories

None. The default single-colour bar/histogram/boxplot fill is the theme's blue
rather than the palette's first colour; not a defect, noted only.
