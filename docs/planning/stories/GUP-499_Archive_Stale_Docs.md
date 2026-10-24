---
implementer: haiku
implementer-reason:
  file moves and deletions from an exact list; every new paragraph's text is
  given in Decisions; ACs are commands
touches:
  [
    docs/,
    README.md,
    CONVENTIONS.md,
    examples/README.md,
    crates/gup-culling-lod/README.md,
  ]
---

# GUP-499: Archive Stale Docs

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 📋 Planned
**Created**: 2026-10-24

## Context

The RFC-001 flip (GUP-486, GUP-488) deleted the old render path, but `docs/`
still holds about 9,600 lines that describe it: the grid, mark-system, custom
mark and type-construction guides, eight accessibility guides and
`docs/accessibility/` (the old ARIA/AT-SPI/VoiceOver layer, deleted), pattern
and shader benchmark reports for deleted benches, the old text pipeline, and
transpiler research for an approach that was not adopted. `docs/INDEX.md` and
`docs/README.md` list them as current. A newcomer cannot tell which documents
are true (STRATEGIC_REVIEW_2026-11 §1 "Docs" and §3 triage).

The review's triage: drop the stale guides (git keeps them); archive the
accessibility docs (N4, and N3's minimal accessibility, start from them) and the
research; make `PERFORMANCE_GUIDE.md` a pointer to
`crates/gup-core/PERF_BUDGETS.md`. The six tutorials are also stale, but their
rewrite is track N1; this story only makes sure each says so. It also moves the
"what is next" text in `README.md`, `docs/IMPLEMENTATION_STRATEGY.md`,
`docs/README.md` and `examples/README.md` from the October review's T5–T7 to the
November review's N0–N6.

## Read First

- `docs/INDEX.md` (114 lines) and `docs/README.md` (181 lines), whole: both are
  rewritten.
- `docs/IMPLEMENTATION_STRATEGY.md` from `## What is next` to the end.
- `README.md` "Development Status" (around line 93).
- `CONVENTIONS.md` "Documentation Structure" (around lines 219–230).
- `docs/tutorials/01_getting_started.md` lines 1–6 (the existing banner).

## User Story

> "As a newcomer reading Gup's docs, I want every document listed as current to
> describe code that exists, and the rest clearly marked as history, so that I
> do not learn an API that was deleted."

## Acceptance Criteria

### AC1: deleted

- [ ] `git rm`: `docs/GRID_SYSTEM.md`, `docs/CUSTOM_MARK_GUIDE.md`,
      `docs/TYPE_CONSTRUCTION_GUIDE.md`, `docs/mark-system/` (4 files),
      `docs/CI_BENCHMARK_INTEGRATION.md`, `docs/CI_PERFORMANCE_ALERTS.md`,
      `docs/CROSS_PLATFORM_AXIS_PERFORMANCE.md`, `docs/EVENT_FORWARDING.md`,
      `docs/PATTERN_BENCHMARK_RESULTS.md`,
      `docs/PATTERN_PERFORMANCE_BENCHMARKING.md`,
      `docs/SHADER_PERFORMANCE_BENCHMARKING.md`, `docs/WASM_PERFORMANCE.md`,
      `docs/VISUAL_REGRESSION_TESTING.md`,
      `docs/text-rendering-architecture.md`.

### AC2: moved

- [ ] `git mv` into `docs/archive/accessibility/` (flat):
      `docs/ACCESSIBILITY_COMPATIBILITY.md`,
      `docs/ACCESSIBILITY_KNOWN_ISSUES.md`, `docs/FOCUS_ELEMENTS_GUIDE.md`,
      `docs/MACOS_VOICEOVER_GUIDE.md`, `docs/WINDOWS_UIAUTOMATION_GUIDE.md`,
      `docs/SCREEN_READER_TESTING.md`, `docs/SCREEN_READER_TESTING_README.md`,
      `docs/SCREEN_READER_CHECKLIST.md` and the four files of
      `docs/accessibility/`.
- [ ] `git mv` into `docs/archive/research/`: the three files of
      `docs/research/`, `docs/existing-solutions-analysis.md`,
      `docs/graphics-programming.md`.
- [ ] `git mv docs/LOD_SYSTEM.md crates/gup-culling-lod/LOD_SYSTEM.md` (it
      describes that parked crate), and `crates/gup-culling-lod/README.md`'s
      "See also `docs/LOD_SYSTEM.md`" becomes a link to `LOD_SYSTEM.md`.
- [ ] `docs/archive/README.md` exists with the text in Decisions.
- [ ] `git diff --stat -M main` shows the moves as renames (similarity ≥ 90%),
      not as delete plus add. Archived files' contents are not edited.

### AC3: rewritten

- [ ] `docs/PERFORMANCE_GUIDE.md` is replaced by the pointer text in Decisions.
- [ ] `docs/IMPLEMENTATION_STRATEGY.md`'s `## What is next` section is replaced
      by the text in Decisions; `cargo test --doc -p gup` passes (the file is a
      doctest).
- [ ] `README.md` "Development Status" is the text in Decisions.
- [ ] `examples/README.md`: "return with the T5 stories" becomes "return in
      track N2 (strategic review 2026-11)".
- [ ] `CONVENTIONS.md` "Documentation Structure": the `graphics-programming.md`
      and `docs/patterns/` items are replaced by one item:
      `**[.github/agents/story-worker.md](.github/agents/story-worker.md)** - Rust design patterns and recurring GPU/WGSL learnings`.
- [ ] Every `docs/tutorials/*.md` file (01–06 and `README.md`) has the banner in
      Decisions as the first paragraph after its H1, replacing any existing "Out
      of date until T6" paragraph: `grep -L 'track N1' docs/tutorials/*.md`
      prints nothing and `grep -l 'until T6' docs/tutorials/*.md` prints
      nothing.
- [ ] Tutorial links to deleted documents are removed: each list item that links
      to `GRID_SYSTEM.md`, `CUSTOM_MARK_GUIDE.md` or `mark-system/` is deleted
      (`docs/tutorials/README.md` lines 66–69, `01_getting_started.md` line 237,
      `02_data_binding.md` line 253, `06_custom_marks.md` lines 366–369), and
      the sentence at `06_custom_marks.md` lines 20–22 becomes "For a mark
      written with `#[derive(Mark)]` today, see
      [`crates/gup-core/tests/derive_mark.rs`](../../crates/gup-core/tests/derive_mark.rs)."
      No other tutorial text changes: the tutorials' links to deleted examples
      (12 today, e.g. `../../examples/zoom_pan.rs`) are left for N1's rewrite,
      which is why tutorials 01–06 are not in the AC4 link check.
- [ ] `docs/README.md` and `docs/INDEX.md` follow the outlines in Decisions.

### AC4: links and listings are true

- [ ] This prints nothing:

      ```bash
      for f in README.md CONVENTIONS.md docs/README.md docs/INDEX.md \
               docs/tutorials/README.md docs/archive/README.md \
               docs/PERFORMANCE_GUIDE.md crates/gup-culling-lod/README.md \
               examples/README.md; do
        grep -oP '\]\(\K[^)#\s]+(?=[)#])' "$f" | grep -v '^https\?:' |
          while read -r l; do
            [ -e "$(dirname "$f")/$l" ] || echo "BROKEN $f -> $l"
          done
      done
      ```

- [ ] Every Markdown file directly in `docs/` is linked from `docs/INDEX.md`:

      ```bash
      for f in docs/*.md; do b=${f#docs/}; [ "$b" = INDEX.md ] && continue
        grep -q "($b)" docs/INDEX.md || echo "UNLISTED $b"; done
      ```

      prints nothing.

- [ ] This prints nothing (no current doc links to a removed or archived one):

      ```bash
      git grep -n -E 'GRID_SYSTEM|CUSTOM_MARK_GUIDE|TYPE_CONSTRUCTION_GUIDE|mark-system/|PATTERN_(BENCHMARK|PERFORMANCE)|SHADER_PERFORMANCE|WASM_PERFORMANCE|VISUAL_REGRESSION_TESTING|text-rendering-architecture|EVENT_FORWARDING|CI_BENCHMARK_INTEGRATION|CI_PERFORMANCE_ALERTS|CROSS_PLATFORM_AXIS|docs/LOD_SYSTEM' \
        -- . ':!docs/planning' ':!docs/archive'
      ```

- [ ] `mask all-check` passes (prettier and mdl on the changed Markdown).

## Decisions

- **Delete vs archive**: guides of deleted code are deleted (git keeps them; N1
  writes a new custom-mark guide around `#[derive(Mark)]`). Accessibility and
  research are archived, because later tracks start from them.
  `01_D3_RESEARCH.md`, `02_WEBGPU_VS_DOM_ANALYSIS.md`,
  `03_RUST_VISUALIZATION_LANDSCAPE.md`, `MARKET_ANALYSIS.md` and
  `MISSION_AND_GOALS.md` stay where they are (background, not descriptions of
  code). `EGUI_INTEGRATION.md` and `BEVY_INTEGRATION.md` are current (GUP-483,
  GUP-484) and stay.
- **`docs/archive/README.md`**:

  ```markdown
  # Archived documents

  These documents describe the system Gup had before the RFC-001 core (deleted
  in GUP-486 and GUP-488). They are kept for history and as input to future
  work, not as a description of the code: nothing here is current.

  - [`accessibility/`](./accessibility/) - the old accessibility layer (ARIA,
    AT-SPI, VoiceOver, UI Automation, keyboard focus, patterns, sonification),
    its WCAG 2.1 AA audit and its testing guides. Release 0.1's minimal
    accessibility (`Plot::describe()`, an SVG title and description and a canvas
    `aria-label`; strategic review 2026-11, Decision 4) and track N4 start from
    these.
  - [`research/`](./research/) - the Rust-to-WGSL transpilation research and the
    existing-solutions analysis (an approach that was not adopted), and the
    graphics-programming patterns of the old render path.

  Current documentation: [docs/README.md](../README.md).
  ```

- **`docs/PERFORMANCE_GUIDE.md`**:

  ```markdown
  # Performance

  Gup's performance budgets, how they are measured and on which machine are in
  [`crates/gup-core/PERF_BUDGETS.md`](../crates/gup-core/PERF_BUDGETS.md). It is
  the only current performance reference; the guide that was here described the
  render path deleted in GUP-488.

  - `mask perf-budget` - the budgets' pass/fail table (needs a display and a
    GPU)
  - `mask gup-core-window` - window parity and the 100K-point zoom benchmark
  - `mask wasm-size` and `mask wasm-attrib` - WASM size against its budget, and
    where the bytes go

  Level of detail and density aggregation are parked in `crates/gup-culling-lod`
  until track N5 (strategic review 2026-11).
  ```

- **`docs/IMPLEMENTATION_STRATEGY.md` `## What is next`**:

  ```markdown
  ## What is next

  The tracks below are defined in the
  [November 2026 strategic review](./planning/STRATEGIC_REVIEW_2026-11.md)
  ("Proposed next tracks" and "Owner decisions"); the stories index holds their
  stories and status.

  - **N0, hygiene.** Current crate docs, stale docs archived, the mobile and
    Tauri crates deleted, honest gates and a checkpoint script for work in
    progress.
  - **N1, docs and tutorials.** The six tutorials rewritten as tested Markdown,
    a `#[derive(Mark)]` guide and documented public items.
  - **N2, chart vocabulary.** A text mark, area and stacked area, horizontal
    bars, histograms, a heatmap builder, line curves, a subtitle and a dark
    theme.
  - **N3, release 0.1.** Publishable crates, CI on Windows, a gallery, a
    changelog and minimal accessibility: `Plot::describe()`, an SVG title and
    description and a canvas `aria-label`.
  - **N4, accessibility.** Keyboard navigation, a hidden data table on the web
    and a high-contrast theme.
  - **N5, performance at scale.** Columnar input, level of detail from
    `crates/gup-culling-lod`, and measured 100M- and 1B-row demonstrations.
  - **N6, export and platforms.** PDF through SVG, and mobile or Tauri hosts
    when a user asks.

  Parked items stay parked until a strategic review's decisions say otherwise; a
  story that extends one needs a decision first.
  ```

- **`README.md` "Development Status"** (body):

  ```markdown
  RFC-001 (the core architecture) is implemented: `gup` is a facade over
  `gup-core`. Next, from the
  [strategic review](./docs/planning/STRATEGIC_REVIEW_2026-11.md): N0, hygiene;
  N1, tested tutorials; N2, more chart types; then N3, release 0.1. See
  [`IMPLEMENTATION_STRATEGY.md`](./docs/IMPLEMENTATION_STRATEGY.md).
  ```

- **Tutorial banner** (after each H1 in `docs/tutorials/`):

  ```markdown
  > **Out of date.** This tutorial was written for the API before the RFC-001
  > flip and does not compile against `gup` today. It is being rewritten as
  > tested Markdown (strategic review 2026-11, track N1). Until then, start from
  > the [README](../../README.md) Quick Start and
  > [Technical Approach](../TECHNICAL_APPROACH.md).
  ```

- **`docs/README.md` outline** (keep each blurb to 1–3 sentences; reuse the
  current blurbs where the document is unchanged):
  1. `# Gup Documentation` + one sentence.
  2. `## Core Documentation`: Mission and Goals; Technical Approach; Market
     Analysis; Implementation Strategy (blurb: "How work is done (stories,
     gates, dogfood), what RFC-001 built, and the tracks N0–N6 that come
     next.").
  3. `## Research Documents`: the three numbered research docs (unchanged).
  4. `## Integrations`: egui (`EGUI_INTEGRATION.md`), Bevy
     (`BEVY_INTEGRATION.md`).
  5. `## Performance`: `PERFORMANCE_GUIDE.md` (a pointer to
     `crates/gup-core/PERF_BUDGETS.md`).
  6. `## Tutorials`: the current list of six, under a note that they are out of
     date until track N1 (replacing the "T6" note).
  7. `## Archive`: `archive/README.md`, one sentence (accessibility and
     research, history only).
  8. `## Quick Navigation`: unchanged, minus links to removed documents.
  9. The closing "These documents represent the current..." line stays.
- **`docs/INDEX.md` outline**: the intro; `## Project Overview` (README,
  MISSION_AND_GOALS, TECHNICAL_APPROACH, IMPLEMENTATION_STRATEGY);
  `## Integrations` (EGUI_INTEGRATION, BEVY_INTEGRATION); `## Performance`
  (PERFORMANCE_GUIDE); `## Tutorials` (tutorials/README.md, "out of date until
  track N1"); `## Research & Analysis` (01–03, MARKET_ANALYSIS,
  references/msdfgen.txt); `## Archive` (archive/README.md); `## Planning`
  (unchanged).

## Escalation Triggers

- A file in the AC1 delete list is included by code (`include_str!`,
  `#[doc = include_str!]`) or read by a script or workflow
  (`git grep -n '<name>' -- '*.rs' '*.sh' '*.yml' '*.toml'` finds it).
- A document outside the lists turns out to describe deleted code and is linked
  as current from `docs/README.md` or `docs/INDEX.md` (report it; do not
  decide).
- `cargo test --doc -p gup` fails after the `IMPLEMENTATION_STRATEGY.md` edit.

## Out of Scope

- Rewriting the tutorials or the custom-mark guide: N1.
- Fixing links inside archived files.
- `docs/planning/` (history), `docs/MISSION_AND_GOALS.md`.
- The mobile and Tauri docs: GUP-497.
- `crates/gup-core` crate docs and `CLAUDE.md`: GUP-498.

## Technical Tasks

- [ ] Start commit within 30 minutes.
- [ ] `git rm` the AC1 list; `git mv` the AC2 lists; commit (moves alone, so git
      records renames).
- [ ] Write `docs/archive/README.md`, `docs/PERFORMANCE_GUIDE.md`; edit the LOD
      link; checkpoint.
- [ ] Edit `IMPLEMENTATION_STRATEGY.md`, `README.md`, `examples/README.md`,
      `CONVENTIONS.md`, the tutorial banners.
- [ ] Rewrite `docs/README.md` and `docs/INDEX.md`.
- [ ] Run the AC4 commands, `cargo test --doc -p gup`, `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-497: Delete the Mobile and Tauri Integrations 📋 — removes
  `docs/README.md`'s Mobile section and `TAURI_INTEGRATION.md` first, so the two
  stories do not edit the same lines.
- GUP-496: Checkpoint Script 📋 — soft, for the DoD's checkpoint line.

### Enables Stories

- N1 tutorials and custom-mark guide; N3 minimal accessibility and N4 start from
  `docs/archive/accessibility/`.

## Testing Strategy

- **Commands**: the three AC4 loops and greps print nothing;
  `cargo test --doc -p gup` passes; `mask all-check` passes.
- **History**: `git diff --stat -M main` shows renames for every move.

## Success Metrics

- [ ] About 9,600 lines leave the current docs (`git diff --stat -M main`).
- [ ] `docs/INDEX.md` lists only files that exist, and every file in `docs/`.

## Risk Assessment

- **Low**: a moved file is renamed and edited in one commit and git loses the
  rename. _Mitigation_: moves get their own commit (Technical Tasks).
- **Low**: prettier rewraps the given text. _Mitigation_: run `mask all-fix`;
  rewrapping is fine, wording is not changed.

## Definition of Done

- [ ] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test --doc -p gup` passes; `mask all-check` passes
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser`: not needed (docs only); say so in the Implementation
      Summary
- [ ] WASM size: "unchanged: no library code change"
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
