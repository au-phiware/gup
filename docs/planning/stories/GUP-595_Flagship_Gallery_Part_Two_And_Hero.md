---
implementer: sonnet
implementer-reason:
  five more charts in GUP-594's pattern, plus one golden at the hero size;
  everything named in Decisions
touches:
  [
    Cargo.toml,
    examples/,
    tests/gallery.rs,
    tests/golden/,
    scripts/gallery_config.toml,
  ]
---

# GUP-595: Flagship Gallery, Part Two, and the Hero Image

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-02

## Context

[GUP-594](GUP-594_Flagship_Gallery_Part_One.md) sets the gallery pattern. Each
chart is `examples/gallery/<name>.rs` with `pub fn chart()` and a `gup::show`
main, a golden in `tests/gallery.rs` at 800 × 500, and an entry in
`scripts/gallery_config.toml`'s `Gallery` category. It delivers scatter, line,
grouped bar, horizontal bar and stacked area. This story adds the other five
chart types, including one built on the low-level `Plot` API and one in the dark
theme. That brings the gallery to ten. It also adds the README hero image as a
golden at banner size, so the image on the front page is checked on every push
and cannot go stale.

## Read First

- GUP-594's Decisions and its finished `tests/gallery.rs` and
  `examples/gallery/scatter_bubbles.rs` (the pattern to follow exactly).
- `docs/tutorials/07_chart_types.md` (GUP-536): the histogram, heatmap, box
  plot, annotation and dark-theme sections.
- Rustdoc: `gup::histogram` (`bins`, `stack`), `gup::heatmap` (`color_value`,
  `color_value_scale`), `gup::boxplot`, `Plot::line`, `Plot::circles`,
  `Plot::text` (`anchor`, `dy`), `Selection::curve`, `Theme::dark`, and `grid`.

## User Story

> "As someone choosing a chart library, I want the gallery to show the
> distribution, matrix and annotation charts too, and a front-page image that is
> a real Gup render, so that I can judge the whole vocabulary at a glance."

## Acceptance Criteria

### AC1: Five more gallery examples

- [x] The five files of Decisions in `examples/gallery/`, each following
      GUP-594's AC2 rules: header, doc line, `pub fn chart()`, `gup::show` main,
      inline deterministic data, at most 90 lines, no new dependency. Each has
      an `[[example]]` entry.
- [x] Each has a title, a subtitle, axis titles where it has axes, and the
      features listed for it in Decisions.

### AC2: Goldens, read by eye

- [x] `tests/gallery.rs` gains one test per chart at 800 × 500
      (`gallery/<name>`). Each PNG is read by eye against GUP-594's AC3 four
      checks, one line each in the Implementation Summary. The dark chart is
      checked against `Theme::dark().background`: the harness's `LayoutMetadata`
      background for that case is the dark theme's background, not white.
- [x] `cargo test -p gup --test gallery -- --test-threads=1` passes with all
      ten.

### AC3: The hero

- [x] Test `hero` in `tests/gallery.rs` renders `scatter_bubbles::chart()` at
      1200 × 630 as golden `gallery/hero`. Read it by eye with the four checks.
      Its file is `tests/golden/gallery/hero.png`, which GUP-597 links from the
      README. Record its size in bytes; it must be under 250 KB.

### AC4: Gallery page

- [x] The five join the `Gallery` category in `scripts/gallery_config.toml`,
      `examples/INDEX.md` and `examples/README.md`. `check_gallery_sync.sh`
      passes. Thumbnails are rendered and read by eye at 640 × 480, as in
      GUP-594's AC4.
- [x] The regenerated `docs/gallery/index.html`
      (`scripts/generate_gallery_html.sh`) lists ten Gallery charts first, and
      `scripts/check_gallery_links.sh` passes.

## Decisions

- **The five charts**:
  - **`histogram_stacked`**: "Commute times", subtitle "Minutes door to door, by
    mode (illustrative data)". About 600 trips over three modes (walk, bike,
    bus); `bins(Bins::Width(5.0))`; `stack` by mode; x title "Minutes"; the y
    axis keeps the builder's "Count" title.
  - **`heatmap_weekly`**: "Website visits by hour", subtitle "Mean visits per
    hour, weekday by hour of day (illustrative data)". x keys `"00"`…`"23"`, y
    keys `Mon`…`Sun`; `color_value` visits;
    `color_value_scale(Sequential::viridis())`; x title "Hour", no y title (the
    weekday names suffice).
  - **`boxplot_latency`**: "Response time by service", subtitle "Box plots,
    whiskers at 1.5 IQR, outliers as points (illustrative data)". Five services
    with 60 values each, and two or three outliers in at least two of them; y
    title "Latency (ms)".
  - **`annotated_prices`**: the low-level API, to show the drop-down. `chart()`
    returns `Plot`. "Share price and company events", subtitle "A step line with
    event markers and text labels (illustrative data)". Daily prices for about
    120 days, drawn with `plot.line(..)` and `.curve(Curve::StepAfter)`; three
    events, each drawn with `plot.circles(..)` and labelled with
    `plot.text(..)`, using `.anchor(Anchor::BOTTOM).dy(-8.0)`; `x_title("Day")`,
    `y_title("Price ($)")`.
  - **`dark_multiline`**: "Server load", subtitle "CPU use of five hosts over a
    day, in Theme::dark()". 96 rows (15-minute steps) × five hosts; `gup::line`
    with `color` by host; `theme(Theme::dark())`; `grid(Axes::Y)`;
    `y_tick_format(|v| format!("{v:.0}%"))`; x title "Hour".
- **The hero is `scatter_bubbles`** at 1200 × 630. It is the north-star chart
  (scatter, log x, size and colour encodings), and 1200 × 630 is the common
  social-preview ratio. It is the same `chart()`, at another size. There is no
  separate hero example.
- **"Illustrative data"**, return types, golden directory and starting point: as
  GUP-594's Decisions.

## Escalation Triggers

- As GUP-594's: a named feature is missing or renders wrongly through its
  documented API; a chart cannot pass the four checks at both sizes without a
  library change.
- The harness's `text_present` or `marks_confined` check misjudges the dark
  chart even with the dark background set in `LayoutMetadata`.
- The hero golden is 250 KB or more.

## Out of Scope

- The README text around the hero: GUP-597.
- Deploying the gallery (Pages): owner step.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [x] Charts 6–8: code, golden, read by eye. Commit after each.
- [x] Charts 9–10. Commit after each.
- [x] Hero (AC3). Commit.
- [x] Config, INDEX, README tables, thumbnails, page regeneration (AC4). DoD
      commands.

## Dependencies

### Prerequisite Stories

- GUP-594: Flagship gallery, part one 📋 — the pattern and the helpers.
- GUP-548 ✅, GUP-549 ✅, GUP-554 ✅, GUP-542 ✅, GUP-550 ✅, GUP-553 ✅ — the
  chart types shown.

### Enables Stories

- GUP-597: README for release 0.1 — the hero image.

## Testing Strategy

- **Integration tests**: five goldens and the hero, with structural checks.
- **Visual validation**: each golden, the hero and each thumbnail, read by eye.

## Success Metrics

- [x] Ten finished charts, every builder plus the `Plot` API and the dark theme,
      each runnable, thumbnailed and guarded by a golden.

## Risk Assessment

- **Medium**: the heatmap's 24 x keys crowd at 640 px. A band axis is labelled
  by its keys, and a tick format on it is a config error (GUP-541).
  _Mitigation_: if the labels overlap at 640 × 480, use two-hour keys (`"00"`,
  `"02"`, …, twelve columns), with the subtitle saying "per two hours", and
  record the change. If that still overlaps, escalate.
- **Low**: the dark chart's harness metadata. _Mitigation_: AC2 sets the
  background explicitly.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: five goldens, the hero and five
      thumbnails
- [x] `mask wasm-browser`: not needed; examples and tests only
- [x] WASM size: unchanged, no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Five examples in `examples/gallery/` (45-58 lines each, `[[example]]` entries in
`Cargo.toml`), six new tests in `tests/gallery.rs` (five charts at 800 × 500,
and `hero`), six new goldens in `tests/golden/gallery/`. The gallery config,
`examples/INDEX.md`, `examples/README.md` and the regenerated
`docs/gallery/index.html` list ten Gallery charts first.
`cargo test -p gup --test gallery -- --test-threads=1`: 11 pass. The existing
five goldens did not change under `GUP_BLESS=1`.

Four checks (not blank; title, subtitle and axis titles present; marks inside
the plot; the configured colours) by eye, 800 × 500 unless noted:

- `histogram_stacked`: three stacked bands (walk, bike, bus) in 5-minute bins
  from 0 to 60, "Count" y title, legend at right.
- `heatmap_weekly`: 7 × 24 cells through viridis with an evening and a lunchtime
  peak, lighter weekends; hour labels thin to every second key at 800 and at
  640, with no overlap, so the two-hour fallback was not needed.
- `boxplot_latency`: five boxes with whiskers and outlier dots (auth, search,
  checkout), "Latency (ms)" title, y grid.
- `annotated_prices`: `Plot` step line over 120 days; three dots each with a
  label above, none overlapping the line (each event sits on the highest day of
  its window).
- `dark_multiline`: five lines on the dark background (the test passes
  `Theme::dark().background` to the harness), grid, "%" ticks, legend; x pinned
  to 0-24 because the automatic domain left a strip.
- `hero`: `scatter_bubbles::chart()` at 1200 × 630, 118,132 bytes (< 250 KB);
  bubbles, log x, legend all inside the frame.

Thumbnails at 640 × 480 were rendered with `scripts/generate_gallery.sh` and
read; all five are clean.

## Retrospective

**Completed**: 2026-11-03

### Key Technical Learnings

#### Labels clear of a step line

- **Challenge**: labels anchored above an event on a step line crossed the line
  where it rose after the event.
- **Solution**: put each event on the highest day of a window (`max_by`), so
  nothing is higher beside it; add a bump to the data so the last event is a
  peak too.
- **Pattern**: choose annotation points that are local maxima when labels sit
  above.

#### Box plots need spread to look like box plots

- **Challenge**: with outliers at 470 ms the boxes of the quick services were
  flat lines.
- **Solution**: keep outliers within about 2.5 times the largest whisker and
  widen the spreads.

### Architectural Decisions

#### Dark chart's harness background

- **Decision**: `check_on` with the background from `Theme::dark().background`.
- **Reasoning**: AC2; the structural checks then pass without exceptions.

### Development Workflow Insights

- A worktree-isolated shell refuses long compound commands (heredoc plus several
  statements); put scripts in `/tmp` and run them as one plain command.
- The pre-commit hook runs the gallery sync check, so the config, INDEX and
  README entries must land in the same commit as the Cargo.toml entries.

### Follow-up Stories

None.
