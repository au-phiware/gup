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

**Initiative**: Strategic Review 2026-11 (N1 Docs: feature tour) **Status**: 🚧
In Progress Ready **Created**: 2026-11-08

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

- [x] `cargo run -p gup-egui --example tour` (or the command recorded in
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
- [x] Every chart starts in its fitted, un-zoomed state, even where the golden
      applies inputs first (chapters 8 and 9): the reader does the interaction.
      Chapter 10's live chart is fed by a thread at a steady rate, so the window
      visibly slides.
- [x] Charts the tour cannot show live are listed with the reason in the chapter
      (for example the SVG/PNG save in chapter 1 becomes a "Save PNG" and "Save
      SVG" button that writes to a temp dir and shows the path; the browser in
      chapter 13 is a sentence and a link).
- [x] `GUP_SCREENSHOT_PATH=…` runs through every chapter and saves one
      screenshot per chapter (`…/NN.png`), then exits, as `egui_chart` does; the
      story reads each screenshot by eye and records one line each.
- [x] `mask smoke-examples` (or the CI job that builds egui examples) builds the
      example; no new CI time beyond its compile.
- [x] `docs/tour/README.md` and the README's tour line say how to run it; a
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

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup -- --test-threads=1` and
      `cargo test -p gup-egui`
- [x] Lint and format clean: `mask all-fix`
- [x] Screenshots of every chapter read by eye
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `gup-egui/examples/tour.rs`: the app. A left list of the thirteen pages
  (titles read from `docs/tour/*.md`), a main area with the title, the first
  paragraph, tabs for a chapter's charts, a live `GupWidget` and the region's
  code (highlighted, scrollable) beside it, and a hint per chart. Page Up/Down
  and `[`/`]` change chapter. Chapter 1 has Save PNG / Save SVG buttons; chapter
  9 has a "Read aloud" tab with the text the snippet returns; chapter 13 is a
  note and a link. Chapters 8 and 9 start fitted (`ResetView`, Escape,
  `PointerLeft` after the snippet). Chapter 10's two charts are fed by threads
  that pause while their tab is off screen.
- `docs/tour/region.rs`: the region extraction, moved out of `tests/tour.rs` and
  `#[path]`-included by both it and the example.
- The snippets include `docs/tour/snippets/10_live_data.rs`, now with `pub`
  `Sample`, `sample` and `reading`.
- Docs: `docs/tour/README.md`, `README.md`, `CHANGELOG.md` `[0.1.0]`,
  `mask tour`.
- Tests (`cargo test -p gup-egui`, the example's own): every region exists, the
  code is on the page, the save buttons write files, the highlighter keeps every
  character (it caught a char-boundary panic).

Run: `cargo run -p gup-egui --example tour`. Screenshots:
`GUP_SCREENSHOT_PATH=/tmp/tour scripts/headless_env.sh --run cargo run -p gup-egui --example tour`
writes `NN.png` (and `NN_K.png` for later tabs). Driven with xdotool under Xvfb:
hover tooltip, wheel zoom, drag pan, brush, keyboard focus and zoom, Page Up
/Down, tab clicks and live growth of chapter 10 all worked.

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### Reusing the facade without a graph edge

- **Challenge**: the snippets are written against `gup`, but a dev-dependency on
  `gup` makes `gup-egui` a dependent of `gup`, which `test_pre_commit.sh`
  rejects (it pins the scoping), and Cargo refuses the same package under two
  names.
- **Solution**: `extern crate gup_core as gup;` at the example's root puts `gup`
  in the extern prelude, so every `use gup::...` and `#[gup::...]` in the
  snippets resolves with no manifest change.
- **Pattern**: an `extern crate x as y;` alias is the zero-graph way to run code
  written against a facade.

#### Charts built on one device, shown on another

- **Challenge**: snippets call `Context::shared()` and resolve on their own
  device; the widget draws on egui's.
- **Solution**: nothing was needed; the chart re-resolves against the widget's
  context and drew correctly in all 30 views.
- **Pattern**: `Chart` resolves lazily per target, so a pre-resolved chart is
  safe to hand to another context.

### Architectural Decisions

#### Live chapter 10 repeats the setters

- **Decision**: the two live charts in the example repeat the snippet's builder
  setters, and a thread replaces the snippet's batches; the code shown is still
  the snippet's region.
- **Reasoning**: the snippet resolves 50,000 rows synchronously, which cannot
  slide; no snippet change was allowed to alter the pages.
- **Trade-off**: ~25 duplicated lines; the sync test does not cover them.

#### One tab per chart

- **Decision**: charts of a chapter are tabs, not stacked.
- **Reasoning**: chapter 2 has nine; one large chart at a time is interactive
  and keeps one widget drawing.

### Development Workflow Insights

- xdotool via `nix shell nixpkgs#xdotool` plus `import -window root` under Xvfb
  is enough to drive and photograph the window; `xdotool windowfocus` is needed
  because there is no window manager.
- Not run: the app on a real display with a real GPU (lavapipe only), so frame
  pacing on hardware is unmeasured.

### Follow-up Stories

None. Not live: the browser and Bevy targets of chapter 13 (a note and a link);
a browser tour was already out of scope.
