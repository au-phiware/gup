---
implementer: sonnet
implementer-reason:
  an egui example that hosts the tour's existing snippet functions in
  GupWidgets; no library change, the layout is decided below
touches:
  [
    gup-egui/examples/tour.rs,
    gup-egui/Cargo.toml,
    docs/tour/README.md,
    README.md,
    maskfile.md,
    CHANGELOG.md,
  ]
---

# GUP-681: The Tour as an Interactive App

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs: feature tour) **Status**: 📋
Ready **Created**: 2026-11-08

## Context

The feature tour (GUP-669, GUP-670; `docs/tour/`) shows each chapter's code and
a still image. The owner asked for it to be interactive: the reader should be
able to zoom, pan, hover, brush and use the keyboard on every chart while
reading the code that drew it. Every chapter's code already exists as functions
in `docs/tour/snippets/NN_*.rs`, compiled by `tests/tour.rs` through `#[path]`
modules, and `gup-egui` has a widget (`GupWidget`) that hosts a live chart in an
egui window (`gup-egui/examples/egui_chart.rs`). One example can include the
same snippet files and show each chart live.

## User Story

> "As a developer evaluating gup, I want to run one command and page through the
> tour with every chart live, so that I can feel the interaction, not just read
> about it."

## Read First

- `docs/tour/README.md`, the thirteen pages and `docs/tour/snippets/*.rs`
  (functions, `// tour:begin ID` / `// tour:end ID` regions).
- `tests/tour.rs` (how snippets are included and which function draws which
  golden; charts that apply inputs before their golden).
- `gup-egui/src/widget.rs` (`GupWidget::new`, `show`), `gup-egui/src/lib.rs`
  (`context`), `gup-egui/examples/egui_chart.rs` (eframe setup, the
  `GUP_SCREENSHOT_PATH` exit path), `gup-egui/Cargo.toml`.
- GUP-622 (keys forwarded by the egui host) and GUP-676 (hover ring, if merged).

## Acceptance Criteria

- [ ] `cargo run -p gup-egui --example tour` (or the command recorded in
      Decisions if the example must live elsewhere) opens one window:
  - a left panel listing the thirteen chapters by their page titles; the
    selected one is highlighted; Page Up / Page Down (or `[` / `]`) and clicks
    change chapter;
  - a main area with the chapter's title, a one-to-three-sentence summary (the
    page's first paragraph as plain text), and for each chart in the chapter:
    the live chart in a `GupWidget` and, beside or below it, the region's code
    in a monospace, scrollable block — the same text the page shows, read from
    the snippet file's region at build time (`include_str!` plus the same region
    extraction the sync test uses), never a second copy;
  - a one-line hint per chart of what to try ("scroll to zoom, drag to pan,
    hover a point", "Tab to the chart, then arrows", "drag with Shift to brush"
    — whatever the chart actually supports).
- [ ] Every chart starts in its fitted, un-zoomed state, even where the golden
      applies inputs first (chapters 8 and 9): the reader does the interaction.
      Chapter 10's live chart is fed by a thread at a steady rate, so the window
      visibly slides.
- [ ] Charts the tour cannot show live are listed with the reason in the chapter
      (for example the SVG/PNG save in chapter 1 becomes a "Save PNG" and "Save
      SVG" button that writes to a temp dir and shows the path; the browser in
      chapter 13 is a sentence and a link).
- [ ] `GUP_SCREENSHOT_PATH=…` runs through every chapter and saves one
      screenshot per chapter (`…/NN.png`), then exits, as `egui_chart` does; the
      story reads each screenshot by eye and records one line each.
- [ ] `mask smoke-examples` (or the CI job that builds egui examples) builds the
      example; no new CI time beyond its compile.
- [ ] `docs/tour/README.md` and the README's tour line say how to run it; a
      CHANGELOG `[0.1.0]` Added line.

## Decisions

- **Host**: egui (`gup-egui`), because it already hosts live charts with input,
  text, panels and scroll areas; no new dependency (no markdown renderer:
  summaries are plain text).
- **Code shown is the snippet region**, extracted at build time from the files
  `tests/tour.rs` includes, so the app, the pages and the goldens cannot
  disagree.
- **No library changes.** If a chart cannot be hosted without one, list it as
  not live with the reason and write a follow-up.
- A browser version (WebGPU, on the Pages gallery) is a later story, not this
  one.

## Escalation Triggers

- The snippet files cannot be included from `gup-egui` (crate or prelude
  mismatch: they `use gup::prelude::*`) without a library or workspace change
  beyond a dev-dependency.
- A chapter's chart needs inputs replayed to make sense live.
- `GupWidget` cannot host more than one chart per frame or loses input when
  charts are swapped.

## Out of Scope

- A browser/WASM tour; markdown rendering; editing code live.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup -- --test-threads=1` and
      `cargo test -p gup-egui`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Screenshots of every chapter read by eye
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
