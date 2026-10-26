---
implementer: sonnet
implementer-reason:
  one more text run in the title block of Plot's layout, with its theme fields,
  placement and role fixed below
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/builder/mod.rs,
    crates/gup-core/src/theme.rs,
    crates/gup-core/src/scene.rs,
  ]
---

# GUP-552: Chart Subtitles

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-27) **Created**: 2026-10-24

## Context

Published charts carry a title that states the finding and a subtitle that says
what is measured ("Daily close, USD, 2020–2024"). Gup has `title` and axis
titles only; the review lists "subtitle" in N2. The title block in
`Plot::resolve_full` (the `top` margin in step 4 and the title run in step 7) is
the only place that changes.

## Read First

- `crates/gup-core/src/plot.rs` lines 1700–1790 (title height, `top`) and
  2105–2120 (the title run, `fit_text`); `Plot::title` and `title_text`.
- `crates/gup-core/src/theme.rs` (whole).
- `crates/gup-core/src/scene.rs` lines 300–320 (`TextRole`).
- `crates/gup-core/src/builder/mod.rs` lines 95–150 (`plot_setters!`).
- `crates/gup-core/tests/common/vr.rs` lines 28–36 and
  `crates/gup-core/tests/scatter_png.rs` lines 30–60 (title checks).

## User Story

> "As a report author, I want a subtitle under my chart's title, so that the
> chart says what it measures without a caption."

## Acceptance Criteria

### AC1: API and theme

- [x] `Plot::subtitle(text) -> &mut Self` and a builder `subtitle` in
      `plot_setters!`, with doctests.
- [x] `Theme` gains `subtitle_size: Px` (default 13) and `subtitle: Color`
      (default `#555555`); `default_is_the_old_styling` asserts both.
- [x] `TextRole::Subtitle` (documented "the chart subtitle"); the tests'
      `vr::role` maps it to the harness's `Other`.

### AC2: Layout

- [x] With a title and a subtitle, the subtitle's ink top is `label_gap` (±1 px)
      below the title's ink bottom, centred on the plot like the title, and the
      plot rect's top moves down by the subtitle's height plus the gap (test
      from `Layout::texts` and `Layout::plot`).
- [x] With a subtitle and no title, the subtitle takes the title's position.
- [x] A subtitle wider than the plot is shortened with `fit_text` (ellipsis), as
      the title is (test).
- [x] Without a subtitle, every layout and golden is unchanged.
- [x] `Chart::title_text` (the window title) still returns the title only.

### AC3: Visual

- [x] Golden `gup_core/subtitle` (640×400): a line chart with title "Rain fell
      most in June" and subtitle "Monthly rainfall, mm, 2025". The worker reads
      it: hierarchy clear (larger, darker title), aligned centres.

## Decisions

- **API**: `pub fn subtitle(&mut self, text: impl Into<String>) -> &mut Self` on
  `Plot`; `pub fn subtitle(self, text: impl Into<String>) -> Self` on builders.
  Why: the word every plotting tool uses; it sits beside `title`.
- **Weight and colour**: regular weight, `Theme::subtitle` colour (lighter than
  `Theme::title`), so the hierarchy shows without bold.
- **No separate alignment option**: the subtitle follows the title's centring.
  Left-aligned titles are a separate decision, out of scope.

## Escalation Triggers

- Adding the `TextRole` variant breaks an exhaustive match outside gup-core's
  tests: fix and list it.
- Any golden without a subtitle changes.

## Out of Scope

- Captions, source notes and footers; title alignment options; multi-line
  titles.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Theme fields, role, setters; commit.
- [x] Layout in step 4 and the run in step 7; AC2 tests; golden; commit.

## Dependencies

### Prerequisite Stories

- GUP-541 📋 (soft): both edit `plot_setters!` and `plot.rs`; run after it.

### Enables Stories

- GUP-553: the dark theme sets the subtitle colour.

## Testing Strategy

- **Integration tests**: layout tests in `scatter_png.rs` or a new
  `subtitle.rs`.
- **Visual validation**: `gup_core/subtitle`, read by eye.

## Success Metrics

- [x] Title and subtitle in one call each, laid out without overlap at any size
      the title fits.

## Risk Assessment

- **Low**: the browser smoke scenes measure the plot rect. _Mitigation_: they
  have no subtitle; AC2 keeps their layout.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`gup_core/subtitle`)
- [x] `mask wasm-browser` passes (layout change)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Plot::subtitle` and the builders' `subtitle` (through `plot_setters!`);
  `Theme::subtitle` (#555555 light, #a8a8b4 dark) and `Theme::subtitle_size`
  (13); `TextRole::Subtitle`.
- Layout (`plot.rs`): the title block is the title, a `label_gap`, then the
  subtitle; the subtitle's ink top is placed from measured ink bounds, so it is
  `label_gap` below the title's ink bottom. Without a title it takes the title's
  place. `fit_text` shortens it. Without a subtitle nothing changes.
- Also fixed the exhaustive `TextRole` match in `tests/tutorials.rs` (outside
  `touches:`, as the escalation trigger allows).
- Tests: `crates/gup-core/tests/subtitle.rs` (6 tests); goldens
  `gup_core/subtitle` and `gup_core/subtitle_dark`. `mask merge-check --dogfood`
  passes; `cargo test --doc -p gup` passes; WASM scatter 341563 B gz.
- Goldens, read by eye: title (18 px, near-black / near-white) over a smaller,
  lighter grey subtitle, both centred on the plot, the subtitle snug under the
  title (4 px gap) with clear space above the y title and plot; the dark theme
  keeps the hierarchy with a muted light-grey subtitle on the dark background.

## Retrospective

**Completed**: 2026-10-27

### Key Technical Learnings

#### Ink versus em-box placement

- **Challenge**: Placing the subtitle at title top plus cap height plus gap left
  a 7 px ink gap, not 4, because glyph ink differs from the metrics.
- **Solution**: Measure the title's and subtitle's ink bounds in step 7 and
  shift the subtitle to hit the gap.
- **Pattern**: When a spec is in ink pixels, position from `ink_bounds`.

### Architectural Decisions

#### Dark subtitle colour

- **Decision**: `Theme::dark()` subtitle #a8a8b4, between label and title.
- **Reasoning**: Keeps the hierarchy; a contrast test asserts at least 4.5:1 and
  below the title's.
- **Trade-off**: None.

### Development Workflow Insights

- Worktree agents' shell refuses chained `cd`/heredoc-plus-git commands; split
  them.

### Follow-up Stories

None.
