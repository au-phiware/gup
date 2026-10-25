---
implementer: opus
implementer-reason:
  a new kind of plot layer (CPU-laid-out text) threaded through Plot::resolve,
  the scene, layout metadata and SVG, with no precedent; the public API is fixed
  below, the internals are design work
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/scene.rs,
    crates/gup-core/src/scale/mod.rs,
  ]
---

# GUP-542: A Text Layer for Labels and Annotations

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-26) **Created**: 2026-10-24

## Context

Gup draws text only for guides: tick labels, titles, legends and tooltips, each
a `TextRun` that `Plot::resolve` places (`scene.rs`, `TextRun`, `TextRole`).
There is no way to put text at a data position, so a user cannot label points,
write a value on a bar, or annotate an event on a time series. The strategic
review lists "a text/annotation mark (so no bar value labels)" as a missing
chart type and puts it first in track N2; GUP-544's value labels depend on it.

A text layer is not a `#[derive(Mark)]` mark. A mark draws a fixed number of
vertices per row from GPU columns, and a label has a variable number of glyphs
laid out by `gup-text` on the CPU. The text pipeline already draws `TextRun`s
identically in every target (window, PNG, texture, canvas) and `SvgTarget`
writes them as `<text>`. So a text layer evaluates its rows on the CPU at
resolve, maps them through the plot's scales' CPU mirrors (exact twins of the
GPU functions), and emits `TextRun`s in a clipped scene item. Labels number in
the tens to thousands, where CPU layout per resolve is cheap; a GPU text mark
for millions of labels is out of scope.

## Read First

- `crates/gup-core/src/scene.rs` lines 290–348 (`TextStyle`, `TextRole`,
  `TextRun`).
- `crates/gup-core/src/plot.rs`: the `Plot` struct (lines 148–245),
  `revision_sum` (1619–1631), `resolve_full` steps 1–2 (1663–1700), step 5
  (1806–1870) and step 7 (1974–2232: labels, `texts`, the clip, mark items).
- `crates/gup-core/src/selection.rs` lines 826–975 (the `Layer` trait, for what
  a layer does at resolve; the text layer does not implement it).
- `crates/gup-core/src/scale/mod.rs` lines 328–383 (`DynPositionScale`) and
  `scale/band.rs` lines 18–106 (`Slots`: key codes and centres).
- `crates/gup-text/src/layout.rs` lines 1–110 (`HAlign`, `VAlign`, `Anchor`,
  `Weight`) and `TextSystem::ink_bounds`/`measure`.
- `crates/gup-core/src/svg.rs` lines 280–380 (how items, clips and text runs are
  written).
- `crates/gup-core/tests/common/vr.rs` lines 28–36 (`role`, an exhaustive
  match).

## User Story

> "As a visualization developer, I want to place text at data positions, so that
> I can label points, annotate events and write values on bars."

## Acceptance Criteria

### AC1: The API

- [x] `Plot::text(rows) -> &mut TextLayer<T>` and `TextLayer<T>`'s methods exist
      with the signatures in Decisions, each documented; `TextLayer` is exported
      from the crate root and `Anchor` is added to the prelude.
- [x] `gup_text::Anchor` gains the nine named constants in Decisions, each with
      a doc line saying where the point sits on the text.
- [x] A doctest from the `gup` crate (an external crate) labels three points and
      asserts each label's `PlacedText` in `Layout::texts` (role
      `TextRole::Annotation`) has its ink box above its point when anchored
      `Anchor::BOTTOM` with `dy(-6.0)`.

### AC2: Placement

- [x] Integration tests (`crates/gup-core/tests/text_layer.rs`): a label at a
      numeric `(x, y)` is anchored at `(x.eval, y.eval) + (dx, dy)` to within
      0.5 px; `x_key` places a label at its band's centre on a `Band` axis;
      `dx_with(Derived)` follows a value read at resolve; a row whose position
      is non-finite or whose key is not in the axis domain draws nothing; a
      row's `within_y` span that is shorter than its label's ink height draws
      nothing, and one taller draws it.
- [x] After `Plot::zoom`, labels move with the data (positions re-evaluated) and
      the zoom writes 0 column bytes (`Context::upload_stats`).
- [x] Labels are clipped to the plot rect: a label anchored on the plot edge has
      no ink outside `Layout::plot` (pixel check).

### AC3: Output

- [x] Golden `gup_core/text_annotations` (480×320): a scatter of six cities
      labelled above their discs, one labelled at the right edge (clipped), and
      a reference line with a `"threshold"` annotation at its right end
      (`Anchor::BOTTOM_RIGHT`). The worker reads it: labels legible, centred
      over their points, none overlapping a tick label.
- [x] `SvgTarget` writes every visible label as a `<text>` inside the plot clip
      group (test: the SVG contains each city name once and no hidden one).

### AC4: Cost

- [x] With 1,000 labels on a 1,000-point scatter, the release-mode time of a
      full `resolve` is measured with and without the text layer and recorded in
      the Implementation Summary; and a cached resolve (nothing changed) does no
      text-layer work (test via a counter or by timing).
- [x] `mask wasm-size` scatter gz bytes reported (the reference scatter has no
      text layer).

## Decisions

- **The public API** (all setters take `&mut self` and return `&mut Self`, as
  `Selection`'s do):

  ```rust
  impl Plot {
      pub fn text<T: Send + Sync + 'static>(
          &mut self,
          rows: impl IntoIterator<Item = T>,
      ) -> &mut TextLayer<T>;
  }

  impl<T: Send + Sync + 'static> TextLayer<T> {
      pub fn x<D: ColumnValue>(&mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> &mut Self;
      pub fn y<D: ColumnValue>(&mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> &mut Self;
      pub fn x_key<K: DictionaryKey>(&mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> &mut Self;
      pub fn y_key<K: DictionaryKey>(&mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> &mut Self;
      pub fn text<S: std::fmt::Display>(&mut self, f: impl Fn(&T) -> S + Send + Sync + 'static) -> &mut Self;
      pub fn anchor(&mut self, anchor: Anchor) -> &mut Self;
      pub fn dx(&mut self, px: f32) -> &mut Self;
      pub fn dy(&mut self, px: f32) -> &mut Self;
      pub fn dx_with(&mut self, px: Derived<Px>) -> &mut Self;
      pub fn dy_with(&mut self, px: Derived<Px>) -> &mut Self;
      pub fn size(&mut self, px: f32) -> &mut Self;
      pub fn color(&mut self, color: Color) -> &mut Self;
      pub fn color_by(&mut self, f: impl Fn(&T) -> Color + Send + Sync + 'static) -> &mut Self;
      pub fn weight(&mut self, weight: Weight) -> &mut Self;
      pub fn within_x(&mut self, span: impl Fn(&T) -> (f64, f64) + Send + Sync + 'static) -> &mut Self;
      pub fn within_y(&mut self, span: impl Fn(&T) -> (f64, f64) + Send + Sync + 'static) -> &mut Self;
      pub fn z(&mut self, z: i32) -> &mut Self;
      pub fn len(&self) -> usize;
      pub fn is_empty(&self) -> bool;
  }
  ```

  User code:

  ```rust
  let mut plot = Plot::new();
  plot.circles(cities.clone()).x(|c| c.lon).y(|c| c.lat);
  plot.text(cities)
      .x(|c| c.lon)
      .y(|c| c.lat)
      .text(|c| c.name)
      .anchor(Anchor::BOTTOM)
      .dy(-6.0);
  ```

  Why these names: `plot.text(rows)` mirrors `plot.circles(rows)` and
  `plot.line(rows)`; `.x`/`.y` take the same closures as a `Selection`'s
  position setters; `.text(..)` names the one thing a text layer adds, and a
  `Display` bound lets `.text(|r| r.count)` and `.text(|r| r.name)` work without
  `format!`. `x_key` says what the closure returns on a band axis (a `Selection`
  encodes keys with `x.encode_key`, which a newcomer would not guess here).
  `dx`/`dy` are the usual names for pixel offsets.

- **Where code lives**: `crates/gup-core/src/text_layer.rs` (new) holds
  `TextLayer<T>` and a `pub(crate) trait AnnotationLayer` (object-safe:
  `revision`, `z`,
  `runs(&mut self, cx, x, y, plot_rect, theme) -> Result<Vec<TextRun>>`,
  `as_any_mut`). `Plot` gets `annotations: Vec<Box<dyn AnnotationLayer>>`;
  `plot.rs` changes are limited to the adder, `revision_sum`, and one call in
  step 7.
- **Positions**: `x`/`y` map through the plot's x and y scales with
  `DynPositionScale::eval`, after ranges are set. `x_key`/`y_key` need a new
  `DynPositionScale::key_position(&self, key: &str) -> Option<f64>`: the centre
  of `key`'s band (or point) for a scale that `reads_keys()`, `None` for a
  numeric scale or an unknown key. A label never adds a key to a domain. A layer
  with no `y` (or no `x`) draws nothing; a numeric closure on a band axis is
  `Error::config` at resolve, naming `x_key`.
- **Labels do not fit domains**: text annotates data another layer draws, and an
  offset label is not a data extent. Documented on `Plot::text`.
- **Evaluation and caching**: the `text` closure and the position closures run
  once per row, on the first resolve after the layer changed, into cached
  `Arc<str>` strings and `f64` values; every full resolve maps the cached values
  to pixels. A cached resolve does nothing. Rows are immutable after
  `Plot::text` (no appender).
- **Drawing**: one `ItemKind::Text` item per layer at `z` (default `Z_AXES - 1`,
  above data layers, below axes; validated like a layer's z), clipped to the
  plot rect (`Item::clip`). Rows whose anchor lies more than `4 × size` px
  outside the plot rect are skipped before layout.
- **Style defaults**: size `Theme::tick_size`, colour `Theme::label`, weight
  `Regular`, anchor `Anchor::CENTER`, offsets 0. A per-row `color_by` wins over
  `color`; the last call of the two wins.
- **`within_x`/`within_y`**: the span `(lo, hi)` in data units is mapped through
  the axis scale; the label is drawn only if its ink box's width (height) is at
  most `|eval(hi) − eval(lo)| − 2` px. On a band axis the span is ignored
  (documented).
- **Layout metadata**: labels are `PlacedText` in `Layout::texts` with
  `TextRole::Annotation` (new variant, documented "a label drawn by a text
  layer"); `tests/common/vr.rs` maps it to the harness's `Other`.
- **Anchor constants** on `gup_text::Anchor`, named by where the point sits on
  the text: `CENTER` (Middle, Middle), `TOP` (Middle, Top), `BOTTOM` (Middle,
  Bottom), `LEFT` (Start, Middle), `RIGHT` (End, Middle), `TOP_LEFT` (Start,
  Top), `TOP_RIGHT` (End, Top), `BOTTOM_LEFT` (Start, Bottom), `BOTTOM_RIGHT`
  (End, Bottom). So `Anchor::BOTTOM` puts the text above its point.
- **Not pickable**: a text layer has no hover, tooltip or brush.

## Escalation Triggers

- The GPU text path ignores `Item::clip` for some target (a label draws outside
  the plot rect in a PNG or the window): stop and report; do not clip on the CPU
  instead.
- 1,000 distinct labels at 12 px overflow the glyph atlas or a per-frame glyph
  budget: report the limit hit and the error; do not raise limits without a
  measurement.
- A full resolve with 1,000 labels costs more than 5 ms more than without them
  (release): report the profile before optimising.
- `TextRole::Annotation` breaks an exhaustive match outside `gup-core`'s tests
  (for example in `gup-egui`): fix it and list it.

## Out of Scope

- Bar value labels (GUP-544), which build on this layer.
- Label collision avoidance, halos, rotation, multi-line text and labels outside
  the plot rect (follow-ups if wanted).
- A GPU text mark for very many labels (track N5).
- Text quality: shaping and fallback fonts (GUP-405, GUP-416).

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Add the `Anchor` constants and `TextRole::Annotation`; commit.
- [x] Write `text_layer.rs` and `key_position`; wire `Plot::text`,
      `revision_sum` and step 7; first placement test; commit.
- [x] The remaining AC2 tests, the golden, the SVG test; commit.
- [x] The cost measurement, the doctest, docs; DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-467: SVG writes marks ✅ — text runs in SVG.
- GUP-540 📋 and GUP-541 📋 (soft): both edit `plot.rs`; run after them.
- GUP-537 📋 (soft): `x_key`/`y_key` take owned keys like the builders' key
  setters, looked up by label (`key_position(&label)`, the key's `Display`), and
  share `DictionaryKey`'s diagnostic.

### Enables Stories

- GUP-544: bar value labels.
- GUP-536: the chart tour shows annotations.

## Testing Strategy

- **Unit tests**: `key_position` on `Band` and `Point`; the `within` rule.
- **Integration tests**: `text_layer.rs` as in AC2; the zoom upload count.
- **Visual validation**: `gup_core/text_annotations`, read by eye.
- **Performance**: AC4's measurement, recorded.

## Success Metrics

- [x] A user labels points or annotates a line with one `plot.text(..)` call,
      identical in PNG, window and SVG.

## Risk Assessment

- **Medium**: the per-resolve CPU cost grows with label count, and zooming
  re-lays out every label. _Mitigation_: off-plot rows are skipped before
  layout; AC4 records the cost; the docs state the intended scale (thousands).
- **Low**: a second kind of layer in `Plot` complicates later layer work.
  _Mitigation_: the annotation trait is small and private; `plot.rs` touches are
  confined to three places.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`gup_core/text_annotations`)
- [x] `mask wasm-browser` passes (new scene item kind in every target)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`plot.text(rows)` adds a `TextLayer<T>`. It has the API in Decisions, and each
setter bumps the layer's revision and drops its cache. On the first full resolve
after a change, its closures run once per row into cached `Arc<str>` labels,
`f64` positions or `Arc<str>` keys, colours and `within` spans. Every full
resolve then maps the cached values through the plot's x and y scales, after
step 5 has set their ranges. Band and point axes go through the new
`DynPositionScale::key_position`. Each label's ink is laid out with
`TextSystem::ink_bounds`, and each layer becomes one `ItemKind::Text` item at
its `z` (default `Z_AXES - 1`), clipped to the plot rect. Its labels go into
`Layout::texts` with `TextRole::Annotation`. A cached resolve never reaches the
layer.

Choices made inside the Decisions:

- **Overhang**: a text layer adds no overhang and fits no domain, so GUP-528's
  edge insets and GUP-529's widening ignore it. A label near an edge is clipped,
  as AC3's Sydney is. This follows "labels do not fit domains". Making room
  would mean measuring labels before step 5, which ties the plot rect to label
  text.
- **Hidden labels**: a label is dropped (from the scene, `Layout::texts` and the
  SVG) if its anchor lies more than 4 × size px outside the plot rect (skipped
  before layout) or its ink box does not meet the plot rect at all (after
  layout). The `PlacedText::bounds` of a partly visible label is its ink box cut
  to the plot rect, so the harness's text and confinement checks see only drawn
  pixels. This is documented on `PlacedText::bounds`.
- **Key on a numeric axis**: `x_key` on a numeric axis is an `Error::config`
  naming `x(..)`, the mirror of the specified value-on-band error naming
  `x_key(..)`. It is not a silent no-op.
- **Empty labels** draw nothing.
- **`key_position`** needs the scale's keys, so `PositionScale` gained a
  `#[doc(hidden)] fn key_code(&self, key: &str) -> Option<f64>` (default `None`,
  implemented by `Band` and `Point`), like `ShaderFn::cpu_color`.
- **The private trait** `AnnotationLayer::runs` takes `&TextSystem` and returns
  `Vec<PlacedText>`, so each label's ink is laid out once and not again for
  `Layout::texts`.
- **`Anchor` in the prelude** is a separate `pub use crate::scene::Anchor;`
  line. The shared `pub use crate::{..}` block is not reflowed, which avoids
  conflicts with GUP-549.

Files: `crates/gup-core/src/text_layer.rs` (new), `crates/gup-core/src/plot.rs`
(field, `Plot::text`, `revision_sum`, z validation, step 7, and the
`PlacedText::bounds` doc), `crates/gup-core/src/scale/{mod,band}.rs`
(`key_position`, `key_code`), `crates/gup-core/src/scene.rs`
(`TextRole::Annotation`), `crates/gup-core/src/lib.rs`,
`crates/gup-text/src/layout.rs` (nine `Anchor` constants), `src/lib.rs` (the
`gup` doctest), and `tests/common/vr.rs` with `tests/tutorials.rs` (the
exhaustive `role` matches). Only these two matches, both in tests, broke on the
new variant. Nothing outside the tests matches `TextRole` exhaustively.

Tests: 12 integration tests in `crates/gup-core/tests/text_layer.rs`, 3 unit
tests (`key_position` on `Band`, `Point` and `Linear`; the `within` rule; the
axis check), 1 `gup` crate doctest (three cities, ink above each point and
centred on it), and the `Plot::text` doctest. `cargo test -p gup-core`: all
pass. Root `cargo test`: all pass.

### Evidence

- **Golden** `tests/golden/gup_core/text_annotations.png` (new, 480×320), read
  by eye. Six blue discs with dark grey 12 px labels centred 6 px above them:
  Perth, Darwin, Alice Springs, Adelaide, Hobart, and Sydney, which is cut to
  "Sydn" at the right edge of the plot rect. An orange (`#d55e00`) line at y =
  −18 has an orange "threshold" label above its right end, right-aligned 4 px in
  from it. Auckland (off the domain) draws nothing. The labels are crisp and
  legible, they do not overlap the discs or each other, and none touches a tick
  label (also asserted on the ink boxes). The SVG raster
  (`text_annotations_svg.png`, compared by `svg_marks::assert_matches`) looks
  the same, clipping included.
- **AC4 cost** (release, `resolve_cost_of_a_thousand_labels`): the median of 21
  full resolves (alternating 800/801 px wide), 1,000-point scatter, without and
  with 1,000 labels. Three runs: 0.056 → 0.614 ms, 0.073 → 0.604 ms and 0.051 →
  0.628 ms, so the labels add about **0.55 ms**, far under the 5 ms trigger. All
  1,000 labels then render into an `ImageTarget` without any atlas error. A
  cached resolve does no text-layer work: a `dy_with` counter does not move over
  three cached resolves, and the resolve generation does not change.
- **WASM** (`mask wasm-size` in `mask merge-check`): scatter **338,647 B gz**
  (recorded 337,974, +0.2%, budget +2%: pass); over wgpu 306,287 B gz. The
  reference scatter has no text layer. The growth is the annotation loop in
  `resolve_full`.
- **`mask merge-check --dogfood`**: all pass (story-ids, doc, gup-core,
  wasm-size, wasm-browser, dogfood).
- **`cargo test --doc -p gup`**: 50 + 4 passed.
- **`scripts/perf_budget.sh --skip-wasm`**:
  - Run 1, at load average 12.7 (other workers building): timing FAILs on
    `zoom.gpu_pass.median` 3.72, `zoom.cpu.median` 1.00, `zoom.cpu.p95` 3.61 and
    `zoom_culled.cpu.median` 3.07. `pipeline.link_create.median` passed at
    1.316.
  - Run 2, at load average 3.7: every zoom row passes (`zoom.cpu.median` 0.523,
    `zoom.gpu_pass.median` 2.24, `zoom_culled.cpu.median` 1.297). The one timing
    FAIL is `pipeline.link_create.median_ms` at 1.365 against a 1.332 limit. Run
    1 passed it, and this story does not touch pipeline creation.
  - Every exact row passes in both runs (0 column bytes, 136 uniform bytes per
    frame, instance bytes unchanged). The wasm rows are "not measured" because
    of `--skip-wasm`.
- **`mask all-check`**, `cargo check --examples --workspace` and
  `mask smoke-examples` all pass.
- Not run: a windowed check. No example draws a text layer, and windows share
  `Renderer::prepare` and its per-item scissor with `ImageTarget`, which the
  pixel clip test covers.

## Retrospective

**Completed**: 2026-10-26

### Key Technical Learnings

#### A CPU layer beside the GPU layers

- **Challenge**: getting a second kind of layer into `Plot::resolve` without
  touching the `Layer` trait or the GPU path.
- **Solution**: a private object-safe `AnnotationLayer`, kept in its own `Vec`.
  The plot touches it in four places: `revision_sum`, the z check, the adder,
  and step 7, after the ranges are final. Its output is plain `TextRun`s in a
  clipped `ItemKind::Text` item, so every target, SVG included, needed no
  change.
- **Pattern**: anything that can be expressed as existing scene items should be.
  The scene's per-item clip made the plot-rect clipping free in every target.

#### Counting work without internal hooks

- **Challenge**: proving from an integration test that "a cached resolve does no
  text-layer work".
- **Solution**: a `dy_with(Derived)` closure with an atomic counter. A `Derived`
  is read once per full resolve, so the count shows whether the layer ran. A
  `.text(..)` closure with a counter showed in the same way that a zoom re-maps
  but does not re-evaluate.

#### Clipped boxes in the layout

- **Challenge**: a label cut by the plot rect has an ink box partly outside it.
  The harness exempts text regions from `marks_confined`, so the full box would
  have hidden clipping bugs.
- **Solution**: `PlacedText::bounds` holds the drawn part, the ink box cut to
  the plot rect. Labels whose box misses the plot rect entirely are dropped.

### Architectural Decisions

#### No overhang from text layers

- **Decision**: labels add nothing to GUP-528's insets or GUP-529's widening.
- **Reasoning**: labels annotate data that other layers draw. Their size depends
  on their text and anchor, and reserving room for them would make the plot rect
  depend on label measurement before ranges exist (a fixed-point problem).
  Clipping is the documented behaviour.
- **Trade-off**: a label next to the plot's edge is cut off. A user who wants it
  whole pads the domain or anchors the label inward.
- **Future**: labels outside the plot rect (direct labels in a margin) would
  need the plot to reserve margin, a separate design.

#### `key_code` on `PositionScale`, hidden

- **Decision**: a defaulted `#[doc(hidden)]` method on the public trait, not a
  downcast.
- **Reasoning**: `DynPositionScale` is implemented once, generically over
  `S: PositionScale`, so it can only reach the keys through the trait.
- **Trade-off**: one more hidden method on a public trait.

### Development Workflow Insights

- The worktree sandbox refuses compound shell commands that mix heredocs, `cd`
  and cargo. Appending with the Edit tool and running each command alone worked.
- `mask merge-check --dogfood` (12 minutes here) ran the new tests, goldens and
  `wasm-browser` in one go, a good final gate.
- `perf_budget.sh` timing rows fail at load average 12, with other workers
  building, and pass at load average 4. Check `uptime` before reading a timing
  FAIL.
- `Weight` is not in the prelude, so `.weight(Weight::SemiBold)` needs
  `gup::scene::Weight`. This is left as is, because the story added only
  `Anchor`.
- A plot with only text layers is still "the plot has no layers": labels fit no
  domain, so there is nothing to scale.

### Follow-up Stories

1. **GUP-526: Text layers on the right y axis**: a `TextLayer` always maps y
   through the plot's left y scale, so a dual-axis chart cannot label its
   right-axis series.
2. **GUP-527: Legible labels over dense marks (halo and overlap culling)**: an
   optional halo (an outline in the background colour) and greedy dropping of
   labels whose ink overlaps an earlier one. These were out of scope here, and
   the 1,000-label measurement shows how dense labels get.
