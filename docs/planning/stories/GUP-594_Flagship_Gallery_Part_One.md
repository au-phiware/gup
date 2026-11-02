---
implementer: sonnet
implementer-reason:
  five example charts on builders that exist, in a file and test pattern fixed
  in Decisions, each checked as a golden read by eye
touches:
  [
    Cargo.toml,
    examples/,
    tests/tutorials.rs,
    tests/golden/,
    scripts/gallery_config.toml,
  ]
---

# GUP-594: Flagship Gallery, Part One

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-02

## Context

Track N3 asks for "a flagship gallery of 8–10 CI-rendered charts and a README
hero image". The gallery today is thin. GUP-564 regenerated
`docs/gallery/index.html`, and it has five examples, four of them renderable:
`01_hello_chart`, `03_line_chart`, `04_bar_chart` and `composite_mixed_data`,
all small demonstrations. The Gallery workflow (`.github/workflows/gallery.yml`)
builds the examples in release mode, runs each non-skipped one with
`GUP_SCREENSHOT_PATH` under Xvfb (`scripts/generate_gallery.sh`, 640 × 480), and
writes the page from `scripts/gallery_config.toml`. Deployment waits for the
owner to enable GitHub Pages (an N3 owner step).

Track N2 has since added the vocabulary that a gallery should show: horizontal
bars, value labels, areas, histograms, heatmaps, box plots, curves, text layers,
subtitles and a dark theme. [GUP-536](GUP-536_Tutorial_Tour_Of_Chart_Types.md)
puts every chart type in tutorial 7, as blocks of at most 20 lines with goldens
in `tests/tutorials.rs`. A gallery chart is the finished version of such a
block: realistic data, a title, a subtitle, axis titles and formats, and a size
that reads well.

This story sets the pattern and delivers five charts.
[GUP-595](GUP-595_Flagship_Gallery_Part_Two_And_Hero.md) delivers the other five
and the hero image. Each chart is an example a user can run
(`cargo run --example <name>`), a thumbnail the Gallery workflow renders, and a
golden that the Tests workflow checks on every push. The golden test is what
keeps the gallery from going stale again.

## Read First

- `tests/tutorials.rs` lines 1–75: `layout_metadata`, `harness`, `check`.
- `docs/tutorials/07_chart_types.md` (GUP-536): the starting code for each chart
  type.
- `scripts/gallery_config.toml` and `scripts/generate_gallery.sh` lines 1–60.
- `Cargo.toml` (root) `[[example]]` entries, and `scripts/check_gallery_sync.sh`
  (what must agree).
- `examples/README.md` and `examples/INDEX.md`.
- The builders' rustdoc for the five charts below: `gup::scatter` (`size`,
  `x_scale`, `x_tick_format`), `gup::line` (`curve`), `gup::bar` (`group`,
  `value_labels`), `gup::barh`, `gup::area` (`stack`, `outline`).

## User Story

> "As someone choosing a chart library, I want a gallery of finished, realistic
> charts with the code that made them, so that I can see what Gup draws and
> start from the one closest to mine."

## Acceptance Criteria

### AC1: Shared test helpers

- [ ] `layout_metadata`, `harness` and a `check(name, chart, width, height)`
      move from `tests/tutorials.rs` to `tests/common/mod.rs`. Both
      `tests/tutorials.rs` and the new `tests/gallery.rs` use them through
      `mod common;`. The tutorial goldens are unchanged:
      `cargo test -p gup --test tutorials -- --test-threads=1` passes with no
      bless.

### AC2: Five gallery examples

- [ ] `examples/gallery/` holds the five files of Decisions. Each has the
      two-line header, a `//!` doc line saying what it shows, a
      `pub fn chart() -> <builder type>` that builds the chart from inline,
      deterministic data, and
      `fn main() -> gup::Result<()> { gup::show(chart()) }`. Each file is at
      most 90 lines, data included. No new dependency.
- [ ] Each chart has a title, a subtitle, both axis titles (where the chart has
      two axes), and the specific features in Decisions.
- [ ] Root `Cargo.toml` has an `[[example]]` entry for each, with
      `path = "examples/gallery/<name>.rs"`.

### AC3: Goldens, read by eye

- [ ] `tests/gallery.rs` includes each example as a module
      (`#[path = "../examples/gallery/<name>.rs"] mod <name>;`, with
      `#[allow(dead_code)]` for its `main`), and has one test per chart:
      `check("gallery/<name>", &mut <name>::chart(), 800, 500)`.
- [ ] Bless them (`GUP_BLESS=1`), then read each PNG by eye. For each, write one
      line in the Implementation Summary covering four things. (1) Every text
      element named in Decisions is present and legible (title, subtitle, axis
      titles, tick labels, legend, value labels). (2) No label overlaps another
      or is clipped at the canvas edge. (3) Every mark lies inside the plot
      area, except labels that GUP-555 deliberately lets reach into the margin.
      (4) The colours are the palette's, distinct by series. A chart that fails
      any of these is fixed in its code, not accepted.
- [ ] `cargo test -p gup --test gallery -- --test-threads=1` passes.

### AC4: Gallery page

- [ ] `scripts/gallery_config.toml` lists the five under a new first category,
      `Gallery`, above `Basic`, each with a one-line description and
      `skip = false`. `examples/INDEX.md` gets a `## Gallery (gallery/)` table
      with the same five, and `examples/README.md`'s table lists them.
      `scripts/check_gallery_sync.sh` passes.
- [ ] `mask ci gallery` (or `scripts/generate_gallery.sh` with the five names,
      under `scripts/headless_env.sh --run`) renders all five thumbnails. Read
      the five 640 × 480 thumbnails by eye: the same four checks as AC3, at the
      smaller size. A chart that only fails at 640 × 480 (for example, crowded
      value labels) is fixed so that it passes at both sizes.

## Decisions

- **The five charts** (file stem = example name = golden name):
  1. **`scatter_bubbles`**: "Wealth and health of nations", subtitle "GDP per
     person against life expectancy, 2020 (illustrative data)". 40 countries
     over five regions; `x_scale(Log::new())`,
     `x_tick_format(|v| format!("${}k", v / 1000.0))`; `size` by population;
     `color` by region; axis titles "GDP per person" and "Life expectancy
     (years)".
  2. **`line_monotone`**: "Monthly mean temperature", subtitle "Three cities,
     smoothed with a monotone curve". Twelve months × three cities;
     `curve(Curve::MonotoneX)`; x is the month number 1–12 with `x_tick_format`
     giving `Jan`…`Dec`; y title "°C".
  3. **`bar_grouped_labels`**: "Quarterly revenue by region", subtitle "Grouped
     bars with value labels". Four quarters × three regions; `group` by region;
     `value_labels_with(|v| format!("${v:.0}k"))`; y title "Revenue ($k)".
  4. **`barh_ranked`**: "Most spoken languages", subtitle "Total speakers,
     millions (illustrative data)". Ten languages, sorted descending so the
     largest is at the top; `value_labels()`; x title "Speakers (millions)".
  5. **`area_stacked`**: "Electricity generation by source", subtitle "Stacked
     areas, 2005–2024 (illustrative data)". Twenty years × four sources; `stack`
     by source; `outline(1.0)`; y title "TWh".
- **"Illustrative data"**: the values are invented but plausible, generated
  inline (literal arrays, or a closed-form formula such as `a + b * sin(i)`).
  There is no file I/O, no random-number crate, and no real dataset that would
  need attribution or licensing. The subtitle says "(illustrative data)"
  wherever a reader might take the numbers as fact.
- **Return type**: `chart()` returns the concrete builder type
  (`ScatterChart<Country>` and so on), so `check` and `gup::show` both take it.
  The row structs are private to each file.
- **Sizes**: goldens at 800 × 500 (the gallery's reading size); thumbnails at
  the config's 640 × 480. The examples do not read any size themselves.
- **Starting point**: copy tutorial 7's block for the same chart type, then
  extend it. Where a tutorial block and a gallery chart would be identical, the
  gallery chart adds data or features; the two are not merged.
- **Golden directory**: `tests/golden/gallery/`, beside `tutorials/`.

## Escalation Triggers

- A feature named in Decisions does not exist, or renders wrongly through its
  documented API (for example, `value_labels_with` on a grouped bar). Report it
  with the PNG. Do not work around it in the example, and do not drop the
  feature silently.
- A chart cannot pass AC3's four checks at both 800 × 500 and 640 × 480 without
  changing library code (for example, the x tick labels of the log axis overlap
  at 640 px).
- Moving the helpers changes a tutorial golden.

## Out of Scope

- The other five charts and the README hero image: GUP-595.
- Changing the Gallery workflow or its deployment (Pages is an owner step).
- New library features found missing: follow-up stories.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] Move the helpers (AC1). Commit.
- [ ] Charts 1–3: code, golden, read by eye. Commit after each.
- [ ] Charts 4–5. Commit after each.
- [ ] Config, INDEX, README tables, thumbnails (AC4). DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-536: Tutorial 7, a tour of chart types 📋 — the starting code, and the
  last story to edit `tests/tutorials.rs` before AC1 moves its helpers.
- GUP-544 ✅, GUP-547 ✅, GUP-551 ✅, GUP-552 ✅ — value labels, stacked areas,
  monotone curves and subtitles.

### Enables Stories

- GUP-595: Flagship gallery, part two, and the hero.
- GUP-590: Windows smoke job — `tests/gallery.rs` joins its test set.

## Testing Strategy

- **Integration tests**: five goldens with the harness's structural checks.
- **Visual validation**: each golden and thumbnail read by eye (AC3, AC4).
- **Regression**: tutorial goldens unchanged; `check_gallery_sync.sh`.

## Success Metrics

- [ ] Five finished charts, each runnable, thumbnailed by CI and guarded by a
      golden.

## Risk Assessment

- **Medium**: finished-looking charts expose layout limits that tutorial-sized
  ones did not: crowded labels at 640 px, long legend entries, log-axis tick
  density. _Mitigation_: AC3 and AC4 read both sizes. A library limit is
  escalated, not hidden.
- **Low**: release-mode thumbnails differ from debug-mode goldens. _Mitigation_:
  both come from the same `chart()`; only the size differs.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: the five goldens and five thumbnails
- [ ] `mask wasm-browser`: not needed; examples and tests only
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
