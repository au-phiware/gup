---
implementer: sonnet
implementer-reason:
  harness, sync test and chapter plan are settled in Decisions; every chapter
  reuses calls that already compile in a tutorial, gallery example or test
touches:
  [
    tests/tour.rs,
    tests/golden/tour/,
    docs/tour/,
    README.md,
    docs/tutorials/README.md,
    docs/INDEX.md,
    CHANGELOG.md,
  ]
---

# GUP-669: Feature Tour, Infrastructure and Chapters 1 to 8

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs: feature tour) **Status**: ✅
Complete (2026-11-08) **Created**: 2026-11-08

## Context

Release 0.1 has seven tutorials, which teach one topic at a time, and a gallery
of ten charts. Neither answers "what can Gup do?" in one sitting. The tour is a
set of thirteen short pages, one per feature area: a runnable snippet and the
image it draws, with a link to the tutorial that teaches the topic. It aims at
breadth, not teaching. This story builds what the pages stand on (the test file
that renders every snippet as a golden image, the check that keeps each page's
code identical to the tested code, the index) and writes chapters 1 to 8.
GUP-670 writes chapters 9 to 13.

The tutorials already keep their code and images honest, in two ways:
`docs/tutorials/*.md` are doctests (`src/lib.rs`, `mod tutorials`), and
`tests/tutorials.rs` repeats each chart's calls by hand next to its golden. The
second half is a copy that can drift from the text. The gallery does better:
`tests/gallery.rs` includes `examples/gallery/*.rs` with `#[path]`, so the
rendered code is the shown code. The tour follows the gallery: the snippet is a
real Rust file included by the test, and a check ties the page's code block to
it.

All APIs below were checked against `main` at 2026-11-08. Gaps between the
request and the code are in Decisions ("What does not exist").

## Read First

- `tests/gallery.rs` lines 1-60 (how a module is included with `#[path]`) and
  lines 140-150 (`check_on` with a dark background); `tests/common/mod.rs` lines
  49-72 (`harness`, `check`, `check_on`).
- `tests/golden/README.md` (blessing: `GUP_BLESS=1`, tolerance 0.25% of pixels
  beyond ΔE 3).
- `docs/tutorials/07_chart_types.md` section starts: Scatter 34, Line 68, Bar
  101, Barh 133, Stacked Area 169, Histogram 204, Heatmap 231, Box Plot 261,
  Annotations 293, A Dark Theme 331. The chapter 2 and 7 snippets take their
  builder calls from these; `tests/tutorials.rs` lines 391-610 are the same
  charts as tests.
- `docs/tutorials/04_interactions.md` lines 51-113 (tooltip, the `pixel` helper)
  and 315-410 (brush); `tests/tutorials.rs` lines 305-360.
- `docs/tutorials/02_data_binding.md` lines 374-423 (a second y axis).
- `crates/gup-core/tests/tick_format.rs` lines 223-300 (`date_axis`,
  `currency_axis`, `percent_axis`) and `crates/gup-core/tests/scales_png.rs`
  lines 298-335 (`time_tick_hierarchy`, daily rows from 1 January 2020).
- `examples/gallery/scatter_bubbles.rs` lines 55-62 (`dollars`),
  `examples/gallery/annotated_prices.rs` (a `Plot` with a step line and text).
- `crates/gup-core/src/text_layer.rs` lines 420-610 (`halo`, `hide_overlaps`,
  `fallbacks`, `avoid_points`) and `crates/gup-core/tests/text_halo.rs` lines
  330-440 (a dense labelled scatter).
- `crates/gup-core/src/chart.rs` lines 105-153 (`ChartExt`: `render`,
  `save_png`) and `crates/gup-core/src/svg.rs` lines 60-92, 154-185 (`SvgTarget`
  and `save`).
- `CHANGELOG.md` `[0.1.0]` "Added" (the list the tour must not contradict).

## User Story

> "As someone deciding whether Gup can draw what I need, I want a short tour of
> what it can do, each stop with the code and the picture, so that I can see the
> breadth in ten minutes and know which tutorial to open next."

## Acceptance Criteria

### AC1: The harness: `tests/tour.rs` and the golden directory

- [x] `tests/tour.rs` exists, in the style of `tests/gallery.rs`: `mod common;`,
      one `#[path = "../docs/tour/snippets/NN_name.rs"] #[allow(dead_code)] mod`
      per snippet file, and one `#[test]` per golden calling `common::check` or
      `common::check_on` with the name `tour/<stem>` (Decisions: naming and
      sizes).
- [x] Goldens live in `tests/golden/tour/`, blessed with
      `GUP_BLESS=1 cargo test -p gup --test tour -- --test-threads=1`.
- [x] `cargo test -p gup --test tour -- --test-threads=1` passes, twice in a row
      with no golden rewritten (`git status tests/golden` clean after the second
      run).
- [x] The tests do not use `std::time`, `rand`, the clock, the network or the
      working directory (the export snippet writes under
      `std::env::temp_dir()`).

### AC2: The sync test

- [x] `tests/tour.rs` has a test `pages_match_their_snippets` implementing the
      rules in Decisions ("The page marker and the sync test"): every code block
      preceded by a `<!-- tour: PATH#ID -->` marker equals that region of
      `PATH`; every `<!-- tour-quote: PATH -->` block is a contiguous piece of
      `PATH`; every image link into `tests/golden/tour/` resolves; every PNG in
      `tests/golden/tour/` is linked from a page; every file in
      `docs/tour/snippets/` is included by a `#[path]` in `tests/tour.rs`; every
      `docs/tour/NN_*.md` is linked from `docs/tour/README.md`; no displayed
      region exceeds 30 lines.
- [x] It normalises `\r\n` to `\n`, so it passes on the Windows job of the
      Platforms workflow.
- [x] A seeded failure: change one character of a displayed block, run the test,
      see it fail with a message naming the page, the marker and the first
      differing line, and revert. Record the message in the retrospective.

### AC3: Chapters 1 to 8 exist and match the plan

- [x] `docs/tour/01_*.md` to `docs/tour/08_*.md` exist with the names, regions
      and goldens in Decisions ("Chapter plan"); each page follows the page
      template in Decisions.
- [x] Every API a page names exists in `main` (the chapter plan lists the ones
      that do not; those are said so on the page, not shown as working code).
- [x] Each region's displayed code is at most 30 lines, uses only `gup::` paths,
      uses no `#[doc(hidden)]` item (`grep -n "doc(hidden)"` on the item's
      definition for each unfamiliar call) and compiles as part of
      `tests/tour.rs`.

### AC4: The images are right

Read each golden PNG (the Read tool shows it) and check the stated content;
record one line per image in the retrospective.

- [x] `01_five_lines`: a scatter with both axes labelled and coloured discs by
      key, nothing outside the plot area.
- [x] `02_*` (nine images): each shows the chart its name says (a scatter; a
      line chart with at least two lines; grouped bars; stacked bars; horizontal
      bars; a stacked area; a histogram; a heatmap with a colour ramp legend;
      box plots), each with its title.
- [x] `03_encodings`: a log x axis (its ticks fall at 1, 2 and 5 times a power
      of ten: met differently, see the summary), discs of different sizes, and a
      viridis colour ramp legend.
- [x] `04_time_axis`: year labels (2020, 2021, 2022) heavier than the month
      labels; `%` on every y tick; the title and a lighter subtitle.
      `04_dual_axis`: a left axis and a right axis, each with its own title.
      `04_bar_labels`: `$` tick labels on the value axis written as `$500` below
      a thousand and `$1k` above, and a value label above each bar in the same
      format.
- [x] `05_palette`: five lines in blue, vermillion, reddish purple, bluish
      green, sky blue (`OKABE_ITO_DISTINCT`), with a legend whose swatches match
      the lines. `05_dark`: the same chart on the dark background with a legend
      readable on it. `05_high_contrast`: black text and axes, larger text,
      lines in the `HIGH_CONTRAST` colours.
- [x] `06_curves`: three lines through the same rows, one stepped, one smooth
      with no overshoot, one translucent, whose crossings are not darker than
      its arms.
- [x] `07_labels`: labels with halos on a dense scatter; at least one label sits
      beside or below its point rather than above it; no label overlaps another
      or a point's disc (look at the densest area).
- [x] `08_tooltip`: the view is zoomed (its ticks are finer than the fitted
      view's: met differently, see the summary) and a tooltip box with the row's
      text sits beside the point (the library draws no hover highlight: met
      differently), inside the plot. `08_brush`: a translucent rectangle over
      some points; the rows outside it faint.

### AC5: Index and wiring

- [x] `docs/tour/README.md` lists chapters 1 to 13 in order with a one-line
      description each: chapters 1 to 8 link to their pages; chapters 9 to 13
      are plain text ending "(coming in GUP-670)", with no link.
- [x] `README.md` links the tour in the sentence under the hero image ("See [the
      gallery]...") and in the "Documentation" list. `docs/tutorials/README.md`
      "Further Reading" links it. `docs/INDEX.md` lists it under "Tutorials".
      `README.md` is still compiled as a doctest by `src/lib.rs`:
      `cargo test --doc -p gup` passes.
- [x] `CHANGELOG.md` `[0.1.0]` "Added" gets one line for the tour.
- [x] `grep -rniE "d3|observable" docs/tour tests/tour.rs` finds nothing.

### AC6: Cost

- [x] `cargo test -p gup --test tour -- --test-threads=1` on a warm build takes
      under 45 s (run it three times and report the median); the full
      `cargo test -- --test-threads=1` grows by under 60 s. Over that, escalate.

## Decisions

### The sync mechanism: snippet files plus a Rust sync test

- **One source of truth for code**: `docs/tour/snippets/NN_name.rs`, a real Rust
  file included by `tests/tour.rs` with `#[path]`, as the gallery is. The test
  renders what the file builds, so the golden is the image of the code.
- **Why not a copy of the code in the test** (what `tests/tutorials.rs` does):
  it drifts. **Why not make the pages doctests**: a doctest compiles the page
  but draws nothing to compare, so the image and the code could still part.
  **Why not generate the pages**: it adds a build step and a committed generated
  file for no gain over a check.
- **Why a Rust test rather than a script like `scripts/check_gallery_sync.sh`**:
  it runs in every `cargo test` including the Windows and macOS jobs (Platforms
  runs `cargo test -p gup --tests`), edits no hot file (`maskfile.md`,
  `scripts/pre_commit.sh`), and fails in the same run that re-blesses goldens.
  The cost is that the pre-commit hook does not run it (docs-only commits run no
  Rust test); CI does.
- **Region markers in the snippet file**: a line `// tour:begin ID` and a line
  `// tour:end ID` (indented as the code is) delimit a region. The region is the
  lines between them, with the begin marker's indentation removed from each line
  (blank lines stay blank). `ID` is `[a-z0-9_]+`, unique within the file. Code
  outside regions (data helpers, the `pixel` helper, `main`) is compiled but not
  shown.
- **The page marker and the sync test**: in a page, a line
  `<!-- tour: docs/tour/snippets/NN_name.rs#ID -->` must be followed, after at
  most one blank line, by a fenced block that opens with three backticks and
  `rust`. The block's text must equal the region, line for line, after `\r\n`
  becomes `\n` and trailing spaces are trimmed. A line
  `<!-- tour-quote: PATH -->` is the same but the block need only be a
  contiguous run of lines of `PATH` (for code the tour quotes from a guide or
  example it cannot compile itself; GUP-670 uses it). Every
  `![..](../../tests/golden/tour/X.png)` must name an existing file, and every
  PNG in that directory must be named by some page. The test reports the page,
  the marker and the first differing line.
- **Snippet file shape**: copyright header (two `//` lines and a blank line), a
  `//!` line saying which chapter it is, `use gup::prelude::*;`, then one
  `pub fn <id>() -> impl Chart` (or `gup::Result<impl Chart>` when it sends
  `Input`) per golden, where `<id>` is the region's ID, and a `fn main()` that
  shows the chart (`gup::show(<id>())`). The golden for region `ID` of chapter
  `NN` is `tests/golden/tour/NN_<ID>.png`. A page's `NN_name.md` and its snippet
  file `NN_name.rs` share the stem; chapter 1's region is `five_lines`, so its
  golden is `01_five_lines.png`.
- **Sizes**: 640 by 400 (the tutorials' size) for every golden except chapter 2,
  480 by 300 (nine images on one page). Dark and high-contrast charts use
  `common::check_on` with the theme's `background.to_rgba8()` as in
  `tests/gallery.rs`.
- **Data is inline and deterministic**: arrays, ranges and formulas; a hand-made
  linear congruential generator if pseudo-random data is needed (chapter 7). No
  `rand`.
- **`wgsl_function` paths**: not needed in this story (GUP-670 chapter 12).

### The page template

Every page, in this order: `# N. Title`; one sentence on what the chapter shows;
the marker and the code block; the image
`![alt](../../tests/golden/tour/NN_ID.png)` with alt text that says what is in
it; two to six sentences or a short table on what each call does (prose wraps at
80 columns, as `prettier` writes it); a "Go deeper" line linking the tutorial
sections and the API item names; a "Not in 0.1" line only where a real gap
exists (cite `CHANGELOG.md` "Known limitations" or the story that owns it). No
page tells a story or teaches; a reader can read all eight in ten minutes.

### Chapter plan

Region IDs are function names and golden stems. "Source" is where the calls
already compile; copy them, do not invent calls.

| Page file                   | Regions (golden)                          | Source and content                                                                                                                                                                                                                                                                              |
| --------------------------- | ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `01_five_lines.md`          | `five_lines` (golden), `save` (no golden) | `five_lines`: `gup::scatter(rows).x(..).y(..).color(..)` and two axis titles (tutorial 1, step 2 data). `save`: `pub fn save(dir: &Path) -> gup::Result<()>` builds `five_lines()`, `save_png(&cx, dir/"chart.png", 640, 400)`, then the SVG (below). `main` is `gup::show(five_lines())`.      |
| `02_vocabulary.md`          | nine, one per builder, all 480x300        | `scatter`, `line`, `bar` (grouped), `bar_stacked`, `barh`, `area` (stacked), `histogram`, `heatmap`, `boxplot`. Calls from tutorial 7 and the gallery (`bar_grouped_labels` for `.group`; `.stack` is on `gup::bar`). Each region is the builder call only, at most 12 lines.                   |
| `03_encodings.md`           | `encodings`                               | Tutorial 1 step 4/5 scatter: `.x_scale(Log::new())`, `.color_value(..)` with `.color_value_scale(Sequential::viridis())`, `.size(..)`. A table of the scales with the chapter that draws each (below). A table "where an axis ends" (below).                                                    |
| `04_axes_and_labels.md`     | `time_axis`, `dual_axis`, `bar_labels`    | `time_axis`: `gup::line` over 900 daily rows, `.x_scale(Time::new())`, `.y_tick_format(\|v\| format!("{v:.0}%"))`, `.title`, `.subtitle`. `dual_axis`: tutorial 2 step 5 as a `Plot` plus `y_right_tick_format`. `bar_labels`: `gup::bar`, `.y_tick_format(dollars)` and `.value_labels()`.     |
| `05_colour_and_themes.md`   | `palette`, `dark`, `high_contrast`        | One five-series `gup::line` over a shared `rows()` helper; `.legend_title(..)`; `.theme(Theme::dark())`; `.theme(Theme::high_contrast())`. A table of `OKABE_ITO_DISTINCT`'s eight hex values (AC below checks it). `.legends(false)` in prose.                                                 |
| `06_lines_and_curves.md`    | `curves`                                  | A `Plot` like `annotated_prices`: three `plot.line(rows)` layers over the same rows with `.curve(Curve::StepAfter)`, `.curve(Curve::MonotoneX)` and a `Color { a: 0.4, ..c }` line through `.color(..)` (as `crates/gup-core/tests/line_translucent.rs`). `Curve` has five variants.            |
| `07_text_and_annotation.md` | `labels`                                  | `gup::scatter(rows).text(rows, \|t\| { .. })` with `.halo(Px(2.0))`, `.hide_overlaps(true)`, `.fallbacks([(Anchor::LEFT, 5.0, 0.0), (Anchor::RIGHT, -5.0, 0.0), (Anchor::TOP, 0.0, 6.0)])`, `.avoid_points(Px(4.0))`; about 60 points from a local generator, dense enough that labels collide. |
| `08_interaction.md`         | `tooltip`, `brush`                        | `tooltip`: tutorial 4 step 1 and 3: `.tooltip(..)`, resolve, `Input::wheel` to zoom, a drag to pan, `Input::pointer_moved` onto a row, resolve again; the `pixel` helper sits outside the region. `brush`: tutorial 4 step 5 (`Drag::Brush`, `Input` down, move, up).                           |

The two tables on page 3, with the facts they must state:

- Scales: Linear (`Linear::new()`, every chapter), Log (this chapter), Time
  (chapter 4), Band (the bar, boxplot and heatmap builders make one from keys;
  chapter 2), Point (`Point::new()`, used on a `Plot`'s axis; no image in the
  tour, say so), Categorical (`Categorical::okabe_ito()` is what `.color(..)`
  uses; chapter 5), Sequential (`Sequential::viridis()`, this chapter and the
  heatmap), Pow (`Pow::sqrt()`, what `.size_scale(..)` takes), Symlog and
  Diverging (names only, link to the rustdoc).
- Where an axis ends. The request named "AxisFit Nice, Ticks and Extent":
  `AxisFit` is a private enum in `plot.rs`, with no public name or setter, so
  the page names the three behaviours, not the type: a scatter's automatic axis
  rounds outward to a round number (Nice); a bar chart's value axis ends on a
  labelled tick (Ticks; `crates/gup-core/tests/bar_axis_ends.rs`); a line or
  area chart's x axis ends at the first and last rows (Extent;
  `crates/gup-core/tests/axis_extent.rs`); an explicit
  `Linear::new().domain(a, b)` wins over all three. State no new claim; each row
  cites its test.

The SVG export in `save` (chapter 1) is
`chart.save_svg(&cx, dir.join("chart.svg"), 640, 400)` (GUP-671, merged before
this story started: `ChartExt::save_svg` and `to_svg` mirror `save_png`), so the
page does not show the three-call `SvgTarget` dance. The test for `save` writes
to `std::env::temp_dir().join(format!("gup-tour-{}", std::process::id()))`,
checks the PNG is 640 by 400, the SVG starts `<svg` and contains `<circle`, and
removes the directory.

**Decision (2026-11-08)**: chapter 1 uses `save_svg`, because GUP-671 added it
after this story was written.

### What does not exist (say so on the page; do not show it as code)

- An HTML export (page 1 says so; GUP-670's chapter 13 says there is no HTML
  export). `save_svg` does exist since GUP-671.
- Built-in "percent", "date" or "currency" tick formatters: a tick format is a
  closure `Fn(f64) -> String`. Page 4 shows closures. A time axis labels itself
  with its hierarchy by default; a custom date label is a closure over Unix
  seconds (`crates/gup-core/tests/tick_format.rs` `date_axis`).
- `.right_axis()` or any right-axis setter on a builder: the second y axis is
  `Plot::y_right`, so `dual_axis` is a `Plot`, as in tutorial 2.
- A convenience for a builder to hover a row from data: the snippet uses the
  `pixel` helper of tutorial 4 (layout ticks), because `Plot::pick_anchor` is
  `#[doc(hidden)]` and must not appear in a page.
- Line `curve` is per chart on a builder, so chapter 6 uses a `Plot`.

### Other decisions

- Page names are those in the plan; the index text for a chapter is the first
  sentence of its page.
- The tour does not repeat tutorial text: where a feature is taught, the page
  links the tutorial section (by heading anchor) and stops.
- No change to any library crate, `maskfile.md`, scripts or workflows. The
  Platforms workflow runs `cargo test -p gup --tests` on Windows and macOS, so
  `tests/tour.rs` is picked up with no edit.
- The existing goldens are not touched. A tour golden that looks like a
  tutorial's is acceptable: it is its own file.

## Escalation Triggers

In addition to the standing triggers in `story-worker.md`:

- A chapter needs a library change to show its feature (a missing setter, a
  `doc(hidden)` item, a wrong render). Stop and report which; do not change the
  library, and do not substitute a different feature without saying so.
- A golden is not deterministic: two consecutive runs differ, or it differs
  between a local run and CI (read the CI diff artifact), by more than the
  harness tolerance.
- `cargo test -p gup --test tour` adds more than about 60 s to CI, or more than
  45 s locally on a warm build.
- A chapter-7 image shows labels over points or over each other after all four
  options are on (that would mean GUP-611 or GUP-656 does not do what its
  retrospective says on this data); report the image.
- `Chart::resolve` followed by `SvgTarget` does not give the marks (the `svg`
  feature is on by default; a missing `<circle` means something else is wrong).
- A page needs more than 30 lines of code in one region to be correct.
- The tooltip golden shows no tooltip (the tooltip is part of the next
  `resolve`; check the resolve size equals the render size).

## Out of Scope

- Chapters 9 to 13: GUP-670.
- An interactive `cargo run --example tour` app that pages through the chapters
  with the arrow keys: a possible later story, not allocated here.
- An HTML export, built-in tick formatters, or a right-axis builder setter:
  library changes; report as follow-ups.
- Rewriting the tutorials or the gallery; translating the tour into the gallery
  HTML page.
- Showing `AxisFit` as a public API.
- Any claim about performance (chapter 11 is GUP-670).

## Technical Tasks

- [x] Read the sources listed in Read First; run the tutorial 7 tests once so
      the tutorial calls are known to compile.
- [x] Create `tests/tour.rs` with the `check`/`check_on` helpers, the `#[path]`
      modules and the sync test (AC1, AC2).
- [x] Write the snippet files and pages for chapters 1 to 8 in the order of the
      plan, blessing and reading each golden as it is written.
- [x] Write `docs/tour/README.md`; wire the README, tutorials README,
      `docs/INDEX.md` and CHANGELOG (AC5).
- [x] Seed and revert the sync failure (AC2); time the test (AC6).
- [x] `prettier --write` on every Markdown file touched; `mdl` on them.
- [x] Set the story and `INDEX.md` status; write the retrospective with a list
      of anything a page could not show and why (these become follow-up
      stories).

## Dependencies

### Prerequisite Stories

- GUP-536: Tutorial Tour Of Chart Types ✅ — the builder calls chapter 2 and 7
  reuse.
- GUP-530 ✅, GUP-531 ✅, GUP-532 ✅: the tutorials the pages link to.
- GUP-658: Default Categorical Order Distinct First ✅ and GUP-625:
  High-Contrast Theme ✅ — chapter 5's palette and theme.
- GUP-611 ✅ and GUP-656 ✅ — chapter 7's `fallbacks` and `avoid_points`.

### Enables Stories

- GUP-670: Feature Tour, Chapters 9 to 13 — uses the harness, the sync test and
  the index.

## Testing Strategy

- **Unit tests**: none beyond the tour tests.
- **Integration tests**: `tests/tour.rs`: one test per golden, the `save` test,
  the palette-table test (the page lists the eight hex values of
  `OKABE_ITO_DISTINCT`, in order: format each `Color` through `to_rgba8()` as
  `#RRGGBB`, uppercase, and assert the page text contains them in that order),
  and the sync test.
- **Visual validation**: every golden read by eye (AC4).
- **Performance**: the timing in AC6.

## Success Metrics

- [x] Eight pages, twenty-one goldens and one sync test, all passing in
      `cargo test -p gup --test tour -- --test-threads=1`.
- [x] Changing a character of a displayed block fails the sync test (seeded).
- [x] Every API named on a page exists, and each gap is stated.

## Risk Assessment

- **Medium**: some chapter-4, 7 and 8 charts depend on layout details (tick
  thinning, label placement, a tooltip's position) and the goldens are
  platform-sensitive. _Mitigation_: use the harness's tolerance as is, keep data
  simple, and escalate on a CI diff rather than loosen anything.
- **Low**: nine goldens on page 2 make it long. _Mitigation_: 480 by 300, and
  one sentence each.
- **Low**: the sync test's marker parsing is small bespoke code. _Mitigation_:
  the seeded failure in AC2, and the rules are line-based.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (every golden PNG read, one line each in
      the retrospective)
- [x] `mask wasm-browser`: not needed, no library, layout, input or wasm code
      changes; state so
- [x] WASM size: unchanged: no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2026-11-08

- `tests/tour.rs`: 24 tests (21 golden tests, `save_writes_a_png_and_an_svg`,
  `the_palette_table_lists_the_default_palette`, `pages_match_their_snippets`).
  A warm run takes 5 s (three runs: 5.0, 4.9, 5.0 s of test time); two
  consecutive runs rewrote no golden.
- `docs/tour/snippets/01..08_*.rs`: eight snippet files included with `#[path]`;
  `docs/tour/01..08_*.md` and `docs/tour/README.md` (chapters 9 to 13 are plain
  text "coming in GUP-670"); 21 goldens in `tests/golden/tour/`.
- Wiring: `README.md`, `docs/tutorials/README.md`, `docs/INDEX.md` (which also
  said the tutorials were out of date and listed six; corrected to seven) and
  one `CHANGELOG.md` line. No library crate, script or workflow changed.
- `mask merge-check` PASS (gup-core 542 s, wasm-size 94 s, wasm-browser 5 s),
  `mask all-check` clean, `cargo test --doc -p gup` 66 + 4 passed.

### What the images show

| Golden             | What was seen                                                                                                                                                                                  |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `01_five_lines`    | Twelve discs in four colours, both axes titled, legend on the right, all inside the plot.                                                                                                      |
| `02_scatter`       | Two colours rising with hours; title, both axis titles, legend.                                                                                                                                |
| `02_line`          | Two smooth seasonal lines (Perth above Hobart), title, legend.                                                                                                                                 |
| `02_bar`           | Grouped bars, phones and laptops in three quarters.                                                                                                                                            |
| `02_bar_stacked`   | Stacked bars, laptops above phones, totals to 25.                                                                                                                                              |
| `02_barh`          | Four horizontal bars, longest at the top.                                                                                                                                                      |
| `02_area`          | Three stacked bands over twelve months, legend in stack order.                                                                                                                                 |
| `02_histogram`     | Bins of ten, peaking at 40 to 60.                                                                                                                                                              |
| `02_heatmap`       | 5 by 4 cells in viridis with a ramp legend titled Visits.                                                                                                                                      |
| `02_boxplot`       | Three boxes, one outlier dot above `auth`.                                                                                                                                                     |
| `03_encodings`     | Log x axis, discs of different sizes, viridis ramp. The ticks read 200, 500, 1k, 2k ... 200k, not powers of ten.                                                                               |
| `04_time_axis`     | 2020, 2021, 2022 in bold among quarterly months, `%` on every y tick, title and lighter subtitle.                                                                                              |
| `04_dual_axis`     | Orange line on the left axis, blue line on the right, each axis titled.                                                                                                                        |
| `04_bar_labels`    | Axis reads `$200` ... `$1k`, `$1.2k` ...; labels `$480`, `$1.25k` above the bars.                                                                                                              |
| `05_palette`       | Five separated lines in the five default colours, matching legend.                                                                                                                             |
| `05_dark`          | The same lines on the dark background; legend and text readable.                                                                                                                               |
| `05_high_contrast` | Black text and axes, larger text, blue, vermillion, green, black, pink.                                                                                                                        |
| `06_curves`        | Blue step, orange smooth curve, wide translucent green line with even colour at its joins.                                                                                                     |
| `07_labels`        | Sixty labelled points with halos; labels beside the point where above is taken; some points in the crowded middle unlabelled; no overlap with another label or a disc is visible at this size. |
| `08_tooltip`       | Zoomed and panned to four birds (ticks every 0.2, finer than the fitted view); a box reading "Kea / 0.9 kg, 0.95 m" beside the Kea.                                                            |
| `08_brush`         | Translucent rectangle over the five lightest birds, which stay solid; the other four are faint.                                                                                                |

### Where the story's expectations differed from the code

- `03_encodings`: the log axis ticks are 1, 2 and 5 times a power of ten, so the
  page says that; there is no way to ask for powers of ten only.
- `08_tooltip`: zooming makes the ticks finer (more of them), not fewer, and the
  library draws no hover highlight, only the tooltip. The page says what is
  drawn.
- `06_curves`: a translucent line through x-sorted rows cannot cross itself, so
  "crossings not darker than arms" is shown at the joins of a wide zig-zag.
- "Where an axis ends": the Nice rule rounds outward to the 10-tick step, finer
  than the labelled ticks, so a scatter chart's last label can sit short of the
  axis end; the page says so, and cites `y_axes_still_round_out`.
- `OKABE_ITO_DISTINCT` is not in the prelude; the snippets use `Color::hex(..)`
  and the page names the constant.

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### Formatting is part of the sync

- **Challenge**: `cargo fmt --all` reformats the snippet files (they are modules
  of `tests/tour.rs`), so a page that was in sync a minute earlier was not.
- **Solution**: the sync test caught it with a message naming the page, the
  marker and the first differing line. I refreshed the blocks from the regions
  with a throwaway script, then ran `cargo fmt` before each later edit.
- **Pattern**: write a snippet, run `cargo fmt --all`, then copy the region to
  the page. The next story (GUP-670) could add a `GUP_TOUR_REFRESH` mode to the
  test, but a hand copy is fine for thirteen pages.

#### The seeded failure

Changing `Hour of day` to `Hour of dayX` in `02_vocabulary.md` fails with:

```text
02_vocabulary.md: the block after `<!-- tour: docs/tour/snippets/02_vocabulary.rs#heatmap -->`
differs from the snippet; first difference at line 7 of the block:
  page:    Some("    .x_title(\"Hour of dayX\")")
  snippet: Some("    .x_title(\"Hour of day\")")
```

Reverted.

#### Alt text must fit the markdown lint

- **Challenge**: `mdl` allows 130 characters outside code and tables, and an
  image line carries about 50 characters of path.
- **Solution**: alt text under about 75 characters; detail goes in the prose
  under the image.

### Architectural Decisions

#### A Rust sync test with `// tour:begin` regions

- **Decision**: as the story specified; one test file, no script.
- **Reasoning**: it runs in every `cargo test`, including Windows and macOS.
- **Trade-off**: the pre-commit hook does not run it for docs-only commits; a
  page edited by hand can break CI. Run `cargo test -p gup --test tour` after
  editing a page.
- **Future**: GUP-670 adds `tour-quote` pages and more regions with no change to
  the test.

#### Chapter 1 uses `save_svg`

- **Decision**: `chart.save_svg(..)` (GUP-671) instead of three calls.
- **Reasoning**: it exists and mirrors `save_png`; the page is shorter and true.

### Development Workflow Insights

- Every snippet compiled on the first try because each call was copied from a
  tutorial or a test; the time went on reading images and fixing data, not API.
- Data choices mattered more than expected: the first chapter 4 dual axis drew
  one line twice (two linear series on two axes), and the first chapter 5 lines
  tangled. Seasonal and well-spaced data fixed both.
- The tour test takes 5 s, far under the 45 s budget.

### Follow-up Stories

None. The gaps (no HTML export, no built-in tick formatters, no right-axis
builder setter, no hover highlight, no powers-of-ten log ticks option, no
`on_hover` that takes data coordinates) are library changes the story already
listed as out of scope; none made a chapter read badly enough to warrant a story
now.
