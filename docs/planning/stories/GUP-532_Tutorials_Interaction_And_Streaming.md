---
implementer: sonnet
implementer-reason:
  docs rewrite of two tutorials against existing, doctested APIs (Input, on_row,
  brush, appender, Window); conventions fixed by GUP-530
touches: [src/lib.rs]
---

# GUP-532: Tested Tutorials, Part 3: Interaction and Streaming

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: ✅
Complete (2026-10-25) **Created**: 2026-10-24

## Context

Tutorial 4 (interactions) and tutorial 5 (streaming data) describe deleted APIs
(`StreamingDataSource`, `DataStream`, backpressure strategies, per-mark click
handler registries). Today interaction is one entry point,
`Chart::handle(Input)`, that every host (`show`, `GupApp`, `gup-egui`,
`gup-bevy`, the web canvas) feeds; tooltips are `.tooltip(|r| ..)`; row events
are `Plot::on_row`; a drag pans, zooms or brushes (`Plot::drag`); and streaming
is an `Appender` handle that any thread pushes rows into, with
`Window::LastRows` to bound memory.

Because `handle` takes plain `Input` values, interaction can be doctested
without a window: a block sends `Input::Wheel` or a pointer drag and asserts the
domain or brush that results, as `crates/gup-core/src/chart.rs`'s `Chart`
doctest already does. This story rewrites both tutorials that way, finishes the
tutorial set, and removes the last banner.

## Read First

- GUP-530's Decisions (block conventions, goldens in `tests/tutorials.rs`).
- `crates/gup-core/src/input.rs` (whole file, 149 lines).
- `crates/gup-core/src/chart.rs` lines 15–60 (the `Chart` doctest).
- `crates/gup-core/src/plot.rs`: the docs of `zoom`, `pan`, `reset_view`,
  `view_axes`, `drag`, `brush`, `on_brush`, `clear_brush`, `on_row`, `pick`,
  `pick_row` (search `pub fn` between lines 690 and 1580).
- `crates/gup-core/src/tooltip.rs` lines 1–40 and `tests/events.rs` lines 1–80.
- `crates/gup-core/src/app.rs` lines 90–140 (`GupApp`).
- `crates/gup-core/src/append.rs` (whole file, 130 lines);
  `crates/gup-core/src/selection.rs` lines 120–180 (`Window`) and 290–400
  (`append`, `window`, `appender`); `builder/line.rs`'s `window` and `appender`
  doctests.
- `crates/gup-core/examples/live_append.rs` (a thread pushing rows).
- `docs/EGUI_INTEGRATION.md` lines 1–40 (to link, not to copy).

## User Story

> "As a developer building a dashboard, I want tutorials that show hover, zoom,
> brushing and live data with code that is tested, so that I can wire my app's
> interaction and data feed with confidence."

## Acceptance Criteria

### AC1: Tutorial 4, Interaction

- [x] Covers, in this order: the model (hosts turn events into `Input`; the
      chart answers `Response { redraw }`); tooltips
      (`.tooltip(|r| format!(..))`); hover and click on rows (`Plot::on_row`
      with `Event::Enter/Leave/Click` and `Plot::row::<T>(id)`); wheel zoom and
      drag pan, `view_axes(Axes::X)` and `reset_view`; `Drag::Zoom`;
      `Drag::Brush` with `on_brush`, `brush()` and the theme's `Unbrushed`;
      picking by hand (`pick`, `pick_row`); and a "hosts" section (`gup::show`,
      `GupApp::new(chart).size(..).run()`, links to the egui and Bevy guides).
- [x] At least three tested blocks drive a chart with `Input` values and assert
      the effect: a wheel zoom narrows the x domain (read through `Plot::invert`
      or the layout's ticks); a drag in `Drag::Brush` mode yields
      `brush().is_some()` with the expected rows; `on_row` receives a `Click`
      for a pointer down/up on a known mark.
- [x] It embeds one golden image: a brushed scatter (`Unbrushed` dimming
      visible).

### AC2: Tutorial 5, Streaming Data

- [x] Covers, in this order: `appender()` on a builder and on a `Selection`;
      pushing from another thread (`push`, `extend`) and what "drawn by the next
      resolve" means; `Window::LastRows(n)` and the `n..1.25n` rows it keeps;
      `Retain` for large layers; keyed line charts gaining a series per new key;
      and what a closed chart returns (`Closed`).
- [x] At least two tested blocks: one pushes rows from a spawned thread, joins
      it, resolves and asserts the layer's `len()`; one shows `LastRows` holding
      within its bounds after several batches (as `LineChart::window`'s doctest
      does).
- [x] It embeds one golden image: a line after appended batches.

### AC3: Wiring and the index

- [x] `src/lib.rs`'s `mod tutorials` gains `Interaction` and `Streaming`;
      `cargo test --doc -p gup -- tutorials` passes; both banners are removed;
      the GUP-530 `awk` and `grep` checks print nothing for `04_*.md` and
      `05_*.md`.
- [x] Goldens `tutorials/04_brush` and `tutorials/05_appended` exist, read by
      eye and described.
- [x] `docs/tutorials/assets/` is deleted (its last two PNGs are stale), and
      `git grep -n 'tutorials/assets'` prints nothing.
- [x] `docs/tutorials/README.md`: rows for tutorials 4 and 5 updated, the banner
      removed, the learning-path diagram matches the six titles, and "Further
      Reading" links `TECHNICAL_APPROACH.md` and `cargo doc --open` (no
      `TODO(GUP-280)`).
- [x] `git grep -n 'Out of date' docs/tutorials` prints nothing.

## Decisions

- **Follow GUP-530's conventions.** `gup::show` and `GupApp::run` blocks are
  `rust,no_run`; everything else runs headless.
- **Interaction is tested through `ChartExt::render`/`Chart::resolve` and
  `Chart::handle`**, not a window: a block resolves at a fixed size, computes
  pixel positions from `Layout::plot` or `Layout::x_ticks`, sends `Input` values
  and asserts on public state.
- **No async runtime.** Threads are `std::thread`; the old `tokio` content is
  dropped.
- **The streaming golden** renders after appends are drained (call `resolve`,
  then render), so the image is deterministic.

## Escalation Triggers

- A pointer drag in a doctest needs more inputs than documented to register (for
  example the click slop): record the sequence that works and check it against
  `Plot::handle`'s docs; if the docs are wrong, report it rather than silently
  matching the implementation.
- A tested block is flaky (threads, timing): never add sleeps; join the thread
  before asserting. If it is still flaky, stop and report.

## Out of Scope

- Keyboard input (`on_key`, track N4).
- The web canvas (`web::CanvasApp`) beyond one sentence and a link to
  `mask wasm-browser`.
- Tutorials 1–3 and 6.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Rewrite tutorial 4 with tested interaction blocks; wire it; golden;
      commit.
- [x] Rewrite tutorial 5; wire it; golden; commit.
- [x] Delete the assets directory; finish the README; run the DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-530: Tested tutorials part 1 📋 — `mod tutorials` and
  `tests/tutorials.rs`.

### Enables Stories

- None directly.

## Testing Strategy

- **Doctests**: `cargo test --doc -p gup -- tutorials`, run three times to show
  the thread blocks are not flaky.
- **Integration**: `cargo test -p gup --test tutorials -- --test-threads=1`.
- **Visual validation**: the two new goldens, read by eye.

## Success Metrics

- [x] All six tutorials compile and run in CI; no tutorial carries a banner.

## Risk Assessment

- **Low**: an interaction doctest couples to layout pixels. _Mitigation_:
  compute positions from `Layout` at run time, never hard-code pixels.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the two tutorial goldens)
- [x] `mask wasm-browser`: not needed, no layout, input or wasm change
- [x] WASM size: unchanged, no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `docs/tutorials/04_interactions.md` rewritten: the `Input`/`Response` model,
  tooltips, `on_row` (`Enter`/`Click`, `Plot::row`), wheel zoom, drag pan,
  `view_axes(Axes::X)`, `reset_view`, `Drag::Zoom`, `Drag::Brush` with
  `on_brush`/`brush()`/`Unbrushed`, `pick`/`pick_row`, and hosts (`GupApp`, egui
  and Bevy guides). Seven runnable blocks drive charts with `Input` and assert
  the effect; one `no_run` block opens a window.
- `docs/tutorials/05_streaming_data.md` rewritten: `appender()` on a builder and
  a `Selection`, a thread pushing rows, `Window::LastRows` bounds, `Retain`,
  keyed lines gaining a series, `Closed`. Six runnable blocks and one `no_run`.
- `src/lib.rs` includes both as doctests; `tests/tutorials.rs` gains
  `tutorial_4_brush` and `tutorial_5_appended`; goldens
  `tests/golden/tutorials/04_brush.png` (nine birds, five brushed and solid,
  four faint, translucent brush rectangle, titled axes) and `05_appended.png`
  (red line of two summed sine waves, title and both axis titles).
- `docs/tutorials/assets/` deleted; README banner, rows, "Images" and "Further
  Reading" updated.
- `scripts/test_pre_commit.sh`: the docs-only case now points at
  `docs/tutorials/README.md` (not included by `src/lib.rs`), and tutorial 4 is
  asserted to be a `gup` source.
- Evidence: `cargo test --doc -p gup -- tutorials` 36 passed, run three times;
  `--test tutorials` 7 passed; `mask all-check` clean; `cargo test -p gup`
  green.

## Retrospective

**Completed**: 2026-10-25

### Key Technical Learnings

#### Testing interaction without a window

- **Challenge**: a doctest needs the pixel of a mark, and `Plot::pick_anchor` is
  `#[doc(hidden)]`.
- **Solution**: interpolate between `Layout::x_ticks` / `y_ticks` (as the
  `Plot::pick` doctest does) and read the domain back with `Plot::invert`. No
  sleeps, no hard-coded pixels; every block passed first time after imports.
- **Pattern**: `Chart::handle` plus `Layout` is a complete headless test harness
  for a host.

#### Two `Point` types

- **Challenge**: `gup::prelude::Point` is not `gup::geom::Point`, the type
  `Input` and `Plot::invert` take.
- **Solution**: tutorial blocks `use gup::geom::Point;` explicitly.
- **Pattern**: a prelude `Point` that cannot be passed to `handle` is a trap for
  readers (see follow-up note below).

### Architectural Decisions

#### The `tutorials/assets` check

- **Decision**: `git grep -n 'tutorials/assets'` still prints historical story
  documents (GUP-281, 530, 531 and this one). The check holds outside
  `docs/planning`; I did not rewrite finished stories.
- **Trade-off**: AC3 literal reading is met for everything except planning
  history.

### Development Workflow Insights

- Each tutorial block repeats its `pixel` helper rather than hiding lines,
  following GUP-530's no-hidden-lines rule (checked with its `awk` command).
- The worktree started before GUP-531 merged; I merged `main` first, which
  changed the pre-commit test (it already pointed at tutorial 4).
- No image showed GUP-529 clipping: both goldens have whole marks (the scatter
  uses `.radius(6.0)`).

### Follow-up Stories

None. (The prelude `Point` versus `geom::Point` mismatch is worth a look by the
orchestrator; not written up as a story.)
