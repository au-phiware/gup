---
implementer: sonnet
implementer-reason:
  new public API whose types, text format and hook points are fixed in
  Decisions; a pure formatting function plus wiring through existing code
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/scene.rs,
    crates/gup-core/src/chart.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/builder/,
  ]
---

# GUP-592: `Plot::describe`, a Text Alternative for Every Chart

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: ✅
Complete (2026-11-02) **Created**: 2026-11-02

## Context

Owner decision 4 (2026-10-24) puts minimal accessibility in release 0.1:
`Plot::describe()`, an SVG title and description, and a canvas `aria-label`. The
rest of track N4 comes after the release. `gup-core` has no text alternative
today. The old `src/accessibility` was deleted in GUP-488, and its docs are in
`docs/archive/`. A chart exported as SVG or drawn on a canvas is a picture with
no words, so a screen reader announces nothing, or "image".

This story produces the text.
[GUP-593](GUP-593_Svg_Title_And_Canvas_Aria_Label.md) writes it into SVG and
onto the canvas. The text comes from what the resolve already computes: the
title and subtitle, each axis's title and tick labels as drawn (so tick formats
apply), the legends as drawn, and each data layer's row count. That makes it
exact and cheap, and it can never disagree with the picture.

## Read First

- `crates/gup-core/src/plot.rs`: `Plot` fields (lines 168–200); `title` and
  `subtitle` setters (528–560); `impl Chart for Plot` (375–406); `layout()`
  (1319–1325); and in `resolve_full` (from 1872), the `legends` local
  (1979–1983), the tick label loops (2516–2580), and the `Ok(Resolved { .. })`
  at the end (about 2766).
- `crates/gup-core/src/selection.rs` lines 1146–1265: the crate-private `Layer`
  trait, and `Selection::len` (558).
- `crates/gup-core/src/scene.rs` lines 26–95: `Scene`, `Scene::new`,
  `Scene::guides`.
- `crates/gup-core/src/chart.rs` lines 51–91: the `Chart` trait.
- `crates/gup-core/src/builder/mod.rs` lines 235–300: `plot_setters!`, and each
  builder's constructor (`scatter.rs:37`, `line.rs:59`, `bar.rs:163` and `203`,
  `area.rs:125`, `histogram.rs:77`, `heatmap.rs:56`, `boxplot.rs:87`).
- `crates/gup-core/src/scale/legend.rs` lines 20–90: `Legend`, `Swatch`, `Ramp`,
  `RampTick`.
- `crates/gup-core/src/scale/mod.rs` line 411: `reads_keys` (a band or point
  axis).

## User Story

> "As a reader using a screen reader, I want every Gup chart to carry a short
> description of what it shows, so that I get its title, axes and series without
> seeing it."

## Acceptance Criteria

### AC1: The API

- [x] `gup_core::Description` (and so `gup::Description`) is the struct of
      Decisions, with `Description::alt_text`. It is in the prelude.
- [x] `Plot::describe(&self) -> Option<&Description>` returns the description of
      the last successful resolve (`None` before one).
      `Plot::description(&mut self, text: impl Into<String>) -> &mut Self`
      replaces the generated `text`; the title is still derived.
- [x] `Chart::describe(&self) -> Option<&Description>` is a provided trait
      method returning `None`. `Plot` and all seven builders implement it by
      delegating to their plot. Every builder gains `.description(text)` through
      `plot_setters!(titles)`.
- [x] `Scene` gains `pub description: Option<Description>`. `Scene::new` leaves
      it `None`, `Plot::resolve` fills it, and `Scene::guides` and the tooltip
      overlay's composed scene keep it.

### AC2: The text

- [x] `crates/gup-core/src/describe.rs` holds a pure
      `pub(crate) fn describe(input: &DescribeInput<'_>) -> Description`, with
      the input struct of Decisions, and unit tests for each sentence rule in
      Decisions: singular and plural rows and layers, a keyed axis of 4 keys and
      of 12 keys, a continuous axis, an axis with no title, an axis with no
      ticks, a swatch legend, a ramp legend, a subtitle ending in a full stop,
      an override text, a plot with no layers, a plot without a title (with and
      without a kind), and `alt_text`'s three joining cases.
- [x] `resolve_full` builds a `DescribeInput` from its locals just before it
      returns, and stores the result in `scene.description`.
- [x] Doctest on `Plot::describe`, through `gup`, running headless. It is the
      scatter of Decisions' worked example. It asserts `title` exactly, and
      asserts `text` equal to the whole expected string, built from the drawn
      tick labels as the worked example says.
- [x] An integration test, `crates/gup-core/tests/describe.rs`, covers one chart
      per builder (scatter, line, bar, barh, area, histogram, heatmap, box
      plot). Each asserts the kind sentence and one axis or legend sentence, so
      the builders' kinds and keyed axes are exercised. The bar case sets
      `y_tick_format(|v| format!("${v:.0}"))` and asserts the `$` in the y-axis
      sentence: tick formats reach the text.

### AC3: Cost

- [x] `Plot::describe` does no work: it reads the stored value. A cached resolve
      (nothing changed) does not rebuild the description. Test: two resolves in
      a row return `Description`s at the same address (`std::ptr::eq` on
      `describe()`).
- [x] `mask wasm-size`: `wasm.scatter.gz_bytes` grows by at most 3,000 B over
      the parent commit; report both numbers. Do not re-record `PERF_BUDGETS.md`
      (the orchestrator does).
- [x] `mask perf-budget`'s exact counters are unchanged. Timing rows are
      advisory in worker runs.

## Decisions

- **Types** (new module `describe.rs`, re-exported at the crate root):

  ```rust
  /// A chart's text alternative: what a screen reader announces for it.
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct Description {
      /// A short name: the chart's title, else its kind ("Scatter chart"),
      /// else "Chart".
      pub title: String,
      /// What it shows: layers, axes and legends, in sentences.
      pub text: String,
  }

  impl Description {
      /// `title` and `text` as one string, for an `alt` or `aria-label`:
      /// joined with ". ", or with " " when `title` already ends in `.`,
      /// `!` or `?`; `title` alone when `text` is empty.
      pub fn alt_text(&self) -> String;
  }

  pub(crate) struct DescribeInput<'a> {
      pub title: Option<&'a str>,
      pub subtitle: Option<&'a str>,
      pub kind: Option<&'static str>,
      pub text_override: Option<&'a str>,
      /// (mark name, rows) per data layer, in plot order.
      pub layers: Vec<(&'static str, usize)>,
      /// x, y, right y; `None` where the plot has no such axis.
      pub axes: [Option<AxisText<'a>>; 3],
      pub legends: &'a [(Option<String>, Legend)],
  }

  pub(crate) struct AxisText<'a> {
      pub title: Option<&'a str>,
      /// Band or point axis (`DynPositionScale::reads_keys`).
      pub keyed: bool,
      /// The tick labels as drawn, in order.
      pub labels: &'a [String],
  }
  ```

  The labels' element type is whatever `resolve_full`'s `x_ticks.labels` holds.
  If that is not `String`, use its type: the field mirrors it and is not public.

- **Kind**: a crate-private `Plot::kind: Option<&'static str>`, set by a
  `pub(crate) fn set_kind` in each builder's constructor. The values are:
  `scatter` → "Scatter chart"; `line` → "Line chart"; `bar` → "Bar chart";
  `barh` → "Horizontal bar chart"; `area` → "Area chart"; `histogram` →
  "Histogram"; `heatmap` → "Heatmap"; `boxplot` → "Box plot". A plain `Plot` has
  none.

- **Layer facts**: the `Layer` trait gains `fn mark_name(&self) -> &'static str`
  (`M::NAME`, lowercased by the formatter) and `fn len(&self) -> usize`
  (`Selection::len`). Text layers (`annotations`) are not data layers and are
  not counted.

- **The text**, sentence by sentence, joined with single spaces. Each sentence
  ends with `.`. `{n}` is formatted with `,` thousands separators (`12,000`).
  Quotes are plain ASCII `"`.
  1. The subtitle, if set, with its trailing `.`, `!` or `?` kept, or `.` added
     if it has none.
  2. `{Kind or "Chart"}, {rows} data row(s) in {k} layer(s).`, where `rows` is
     the sum over data layers. Without a user title the `title` is already the
     kind, so the sentence drops it and is capitalised:
     `6 data rows in 1 layer.` (`alt_text` then reads
     `Chart. 6 data rows in 1 layer. …`, not `Chart. Chart, …`). With no data
     layers: `{Kind or "Chart"} with no data.`, or `No data.` without a user
     title. Singular: `1 data row`, `1 layer`. When there are two or more layers
     with different mark names, append ` ({mark} {rows}, {mark} {rows})` before
     the full stop, using the list rule. Example, a titled `Plot` with no kind:
     `Chart, 210 data rows in 2 layers (circle 200 and line 10).`
  3. For each axis present, x then y then right y, named `X axis`, `Y axis` and
     `Right y axis`. With a title: `X axis "{title}"`; without one: `X axis`.
     Then:
     - keyed, with labels: `: {n} categories, {list}.` (`1 category`);
     - continuous, with at least one label: `: {first} to {last}.`, or
       `: {first}.` with a single label;
     - no labels: just `.`, or no sentence at all when there is no title either.
  4. For each legend drawn, in order: `Colour legend "{title}"` (or
     `Colour legend` without a title). Then `: {list}.` of the swatch labels, or
     for a ramp `: {first tick label} to {last tick label}.`
- **The list rule**: one item: `a`. Two: `a and b`. Three to eight:
  `a, b, c and d`. More than eight: the first six, then `and {n − 6} more`, as
  in `a, b, c, d, e, f and 6 more`.
- **Override**: `text_override` (from `Plot::description`) replaces sentences
  1–4 whole. The title is still derived.
- **Worked example** (the AC2 doctest):

  ```rust
  let mut chart = gup::scatter(vec![(1.0, 55.0, "Africa"), (20.0, 74.0, "Europe"), (60.0, 81.0, "Asia")])
      .x(|r| r.0).y(|r| r.1).color(|r| r.2)
      .title("Wealth and health").x_title("GDP ($k)").y_title("Life expectancy");
  ```

  Resolved at 640 × 400, its `title` is `Wealth and health`, and its `text`
  begins `Scatter chart, 3 data rows in 1 layer. X axis "GDP ($k)":` (then a
  space and the first tick label), and its last sentence starts `Colour legend`
  and ends `: Africa, Europe and Asia.` (with the legend's title between them if
  the builder gives it one; assert whichever the resolve produces, and record
  it). The axes' first and last tick labels are whatever the resolve draws. The
  doctest reads them from `layout().x_ticks` and formats the expected string
  from them. It does not hard-code numbers.

- **Where it lives in `Scene`**, not `Layout`: vector and canvas targets see
  only the scene (GUP-593). `Layout` is unchanged.
- **What is left out for 0.1**: data values, trends and summaries (N4's hidden
  data table), per-mark descriptions, and translations.

## Escalation Triggers

- `resolve_full`'s tick labels are not available per axis at the end of the
  function (for example, if they are consumed by the label loops). Report where
  they go. Do not recompute ticks a second time.
- Adding `mark_name` or `len` to `Layer` needs a change to `#[derive(Mark)]`'s
  expansion or to `Mark`'s public items.
- `wasm.scatter.gz_bytes` grows by more than 3,000 B.
- A test or doctest that compares a whole `Scene` (with `==` or `Debug` output)
  breaks because of the new field. List them before changing any.

## Out of Scope

- Writing the description into SVG or onto a canvas: GUP-593.
- Keyboard focus, a hidden data table, and a high-contrast theme: track N4.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [x] `describe.rs`: the types, the formatter, unit tests (AC2, first item).
      Commit.
- [x] `Layer::mark_name`/`len`, `Plot::kind`, `set_kind` in builders,
      `Scene::description`, the `resolve_full` hook (AC1, AC2). Commit.
- [x] `Plot::describe`/`description`, `Chart::describe`, builder setter, prelude
      and doctest (AC1, AC2). Commit.
- [x] `tests/describe.rs`; the cache test; `mask wasm-size` (AC2, AC3). DoD
      commands.

## Dependencies

### Prerequisite Stories

- GUP-541 ✅ (tick formats), GUP-552 ✅ (subtitle), GUP-543 ✅, GUP-547 ✅,
  GUP-548 ✅, GUP-549 ✅, GUP-554 ✅ (the builders whose kinds are named).

### Enables Stories

- GUP-593: SVG title and canvas `aria-label` — writes this text.
- GUP-597: README for release 0.1 — the accessibility line in "Not yet".

## Testing Strategy

- **Unit tests**: every sentence rule in `describe.rs`, without a GPU.
- **Integration tests**: one chart per builder in `tests/describe.rs`.
- **Doctest**: the worked example, through `gup`.
- **Visual validation**: none; nothing renders differently. Run the root
  `cargo test` to show the goldens unchanged.

## Success Metrics

- [x] Every chart a builder or a `Plot` draws has a description that names its
      title, kind, row count, axes and legends.

## Risk Assessment

- **Low**: the generated English is awkward for some charts (a box plot's rows
  are its values, not its boxes). _Mitigation_: `description(text)` overrides
  it; N4 refines the wording with users.
- **Low**: `Scene` construction sites outside `Plot` (tests, the WASM harness,
  egui and Bevy) do not compile, if any builds `Scene` with a struct literal.
  _Mitigation_: `Scene::new` sets the field; grep for `Scene {` first.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not applicable; goldens unchanged
- [x] `mask wasm-browser`: run it (the resolve changes in the browser build); it
      passes unchanged
- [x] WASM size: `mask wasm-size` scatter gz bytes, parent and after
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2026-11-02

- `crates/gup-core/src/describe.rs`: `Description` (+ `alt_text`),
  `DescribeInput`, `AxisText`, `describe()` and 13 unit tests.
- `Layer::mark_name`/`len`; `Plot::kind` (`set_kind` in all seven builder
  constructors; bar/barh choose by orientation); `Scene::description`
  (`Scene::new` `None`, `guides()` keeps it; the overlay clones it);
  `Plot::describe`/`description`; `Chart::describe` (provided, `None`) with all
  seven builders delegating; `.description(text)` through
  `plot_setters!(titles)`; `Description` in the prelude and crate root.
- `resolve_full` builds the input from its locals just before returning.
- Tests: 13 unit, `tests/describe.rs` (13: one per builder, stacked-area legend,
  a two-layer `Plot` with a right axis, tick format, override, cache `ptr::eq`,
  scene), doctest on `Plot::describe`. Workspace tests,
  `cargo test --doc -p gup` (66 + 4) and `cargo check --examples` pass.
- WASM: `wasm.scatter.gz_bytes` parent (f9293c6e) 360,424 B, after 362,922 B:
  +2,498 B (limit 3,000). The first cut was 371,811 B (+11,387):
  `str::to_lowercase` pulls Unicode tables (use `to_ascii_lowercase`), and
  `format!` chains cost the rest. `mask merge-check`: wasm-size FAIL only
  because the recorded 354,964 B is stale (the parent already measures 360,424
  B, over its +2% limit); the orchestrator re-records. The other checks pass,
  `wasm-browser` included.
- Deviation: the doctest uses `gup_core::scatter` (a `gup-core` doctest cannot
  depend on `gup`, which re-exports it). Tick labels come from `layout().texts`
  (`TextRole::TickLabel`; the first `x_ticks.len()` are x), because
  `Layout::x_ticks` holds values and pixels, not labels.

Descriptions as a screen reader hears them (`alt_text`):

1. Heatmap: "Visits. Heatmap, 12 data rows in 1 layer. X axis: 4 categories, 0,
   1, 2 and 3. Y axis: 3 categories, Wed, Tue and Mon. Colour legend "Visits": 0
   to 30."
2. Plot (circle + line, right axis): "Climate. Chart, 30 data rows in 2 layers
   (circle 20 and line 10). X axis: 0 to 20. Y axis: 0 to 20. Right y axis "Rain
   (mm)": 0 to 9."
3. Box plot: "Latency. Box plot, 12 data rows in 4 layers (rect 2, segment 4,
   segment 4 and segment 2). X axis: 2 categories, api and web. Y axis: 0 to
   120."

The third shows the wording gaps follow-ups GUP-600 and GUP-601 address.

## Retrospective

**Completed**: 2026-11-02

### Key Technical Learnings

#### WASM size of string code

- **Challenge**: The first cut grew the scatter by 11.4 KB gz, over the 3 KB
  cap.
- **Solution**: `to_ascii_lowercase` instead of `to_lowercase` (-8 KB), then
  `String::push_str` and a hand-written digit loop instead of `format!` chains
  (-0.7 KB).
- **Pattern**: Avoid Unicode-aware `str` methods in code reached from the WASM
  scatter; measure with `mask wasm-size` after each formatting change.

#### Labels as drawn

- **Challenge**: `Layout` has no tick labels, only values and pixels.
- **Solution**: the formatter takes `Ticks::labels` from `resolve_full` (which
  also set the text runs), so the text and picture share one source.
- **Pattern**: Band axes list labels in drawn order, so a vertical band axis
  reads bottom to top ("Wed, Tue and Mon").

### Architectural Decisions

#### Description lives in the cache's Scene

- **Decision**: `Plot::describe` reads `cache.scene.description`; no separate
  field.
- **Reasoning**: A cached resolve keeps the same address (tested), and the
  overlay clone carries it for free.
- **Trade-off**: `Plot::description(text)` bumps the revision, so it forces a
  full re-resolve.
- **Future**: GUP-593 reads `scene.description`.

### Development Workflow Insights

- The pre-commit `md` check rejects an unformatted story: run `prettier --write`
  on it before the Start commit.
- Measuring the parent's WASM size needs a second worktree; `git worktree add` a
  sibling and remove it after.

### Follow-up Stories

1. **GUP-600**: group same-named marks and describe composite marks (box plot).
2. **GUP-601**: read vertical band axes top to bottom.
