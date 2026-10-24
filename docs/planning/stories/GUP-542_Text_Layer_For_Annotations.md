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

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

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

- [ ] `Plot::text(rows) -> &mut TextLayer<T>` and `TextLayer<T>`'s methods exist
      with the signatures in Decisions, each documented; `TextLayer` is exported
      from the crate root and `Anchor` is added to the prelude.
- [ ] `gup_text::Anchor` gains the nine named constants in Decisions, each with
      a doc line saying where the point sits on the text.
- [ ] A doctest from the `gup` crate (an external crate) labels three points and
      asserts each label's `PlacedText` in `Layout::texts` (role
      `TextRole::Annotation`) has its ink box above its point when anchored
      `Anchor::BOTTOM` with `dy(-6.0)`.

### AC2: Placement

- [ ] Integration tests (`crates/gup-core/tests/text_layer.rs`): a label at a
      numeric `(x, y)` is anchored at `(x.eval, y.eval) + (dx, dy)` to within
      0.5 px; `x_key` places a label at its band's centre on a `Band` axis;
      `dx_with(Derived)` follows a value read at resolve; a row whose position
      is non-finite or whose key is not in the axis domain draws nothing; a
      row's `within_y` span that is shorter than its label's ink height draws
      nothing, and one taller draws it.
- [ ] After `Plot::zoom`, labels move with the data (positions re-evaluated) and
      the zoom writes 0 column bytes (`Context::upload_stats`).
- [ ] Labels are clipped to the plot rect: a label anchored on the plot edge has
      no ink outside `Layout::plot` (pixel check).

### AC3: Output

- [ ] Golden `gup_core/text_annotations` (480×320): a scatter of six cities
      labelled above their discs, one labelled at the right edge (clipped), and
      a reference line with a `"threshold"` annotation at its right end
      (`Anchor::BOTTOM_RIGHT`). The worker reads it: labels legible, centred
      over their points, none overlapping a tick label.
- [ ] `SvgTarget` writes every visible label as a `<text>` inside the plot clip
      group (test: the SVG contains each city name once and no hidden one).

### AC4: Cost

- [ ] With 1,000 labels on a 1,000-point scatter, the release-mode time of a
      full `resolve` is measured with and without the text layer and recorded in
      the Implementation Summary; and a cached resolve (nothing changed) does no
      text-layer work (test via a counter or by timing).
- [ ] `mask wasm-size` scatter gz bytes reported (the reference scatter has no
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

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] Add the `Anchor` constants and `TextRole::Annotation`; commit.
- [ ] Write `text_layer.rs` and `key_position`; wire `Plot::text`,
      `revision_sum` and step 7; first placement test; commit.
- [ ] The remaining AC2 tests, the golden, the SVG test; commit.
- [ ] The cost measurement, the doctest, docs; DoD commands.

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

- [ ] A user labels points or annotates a line with one `plot.text(..)` call,
      identical in PNG, window and SVG.

## Risk Assessment

- **Medium**: the per-resolve CPU cost grows with label count, and zooming
  re-lays out every label. _Mitigation_: off-plot rows are skipped before
  layout; AC4 records the cost; the docs state the intended scale (thousands).
- **Low**: a second kind of layer in `Plot` complicates later layer work.
  _Mitigation_: the annotation trait is small and private; `plot.rs` touches are
  confined to three places.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (`gup_core/text_annotations`)
- [ ] `mask wasm-browser` passes (new scene item kind in every target)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
