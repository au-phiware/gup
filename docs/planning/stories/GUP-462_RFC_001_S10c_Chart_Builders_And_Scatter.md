---
implementer: sonnet
implementer-reason:
  the builder shape, every public signature, defaults and the materialisation
  rule are fixed in Decisions; it is sugar over `Plot` adders and scales
---

# GUP-462: RFC-001 Step S10c: Chart Builders and `gup_core::scatter`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-18

## Context

RFC-001 S10 ports the `scatter`, `line` and `bar` builders onto `Plot` and
`layer_mut`. The strategic review's north star is:

```rust
let chart = gup::scatter(&data)
    .x(|d: &Row| d.gdp).y(|d: &Row| d.life_exp)
    .color(|d: &Row| d.continent.as_str())
    .size(|d: &Row| d.population);
```

`gup-core` already has everything underneath:

- `Plot::circles(rows)` with position closures that infer `|r| r.a` (GUP-447);
- `Categorical`, `Sequential` and `Pow::sqrt()` with legends;
- `Chart`/`ChartExt` (`save_png`, `render`) and `gup_core::show`;
- `Plot::layer_mut`.

What is missing is the short form: one call per encoding, with default scales
for colour and size. GUP-447 deliberately left "closures on colour and size,
with default scales" to a later story. This story adds them in the builders, not
in `Selection`.

The old builders (`src/chart_builder/builders/{scatter,line,bar}.rs`) take
`AccessorFunction`s returning `AccessorValue`, NDC scales, and
`build_with_data(rows, ctx)`. They stay frozen until GUP-466 deletes them. This
story sets the pattern that `line` (GUP-463) and `bar` (GUP-464) follow: the
module, the shared plot-level setters, the materialisation rule and the escape
hatches.

`gup-core` is not re-exported by `gup` until RFC-001 S14, so the functions are
`gup_core::scatter` etc. now and become `gup::scatter` at the flip.

## Read First

- RFC-001 §10 (lines 667–735): the API sketches this realises, and §12 risks 4,
  5 and 16 (lines 813–866).
- [GUP-447](GUP-447_RFC_001_S8d_Mark_Adders_And_Default_Axes.md) Decisions and
  Retrospective: position closures and why styling setters take
  `impl IntoEncoding`.
- `crates/gup-core/src/plot.rs` lines 380–560 (`new`, setters, adders,
  `layer_mut`) and `src/chart.rs` (whole).
- `crates/gup-core/src/lib.rs` (exports, prelude) and `src/marks.rs` (`Circle`
  channels).
- `crates/gup-core/tests/common/scatter.rs` (the reference scatter, which AC2
  reproduces).
- `src/chart_builder/builders/scatter.rs` method list: `grep -n "pub fn"` only.

## User Story

> "As a new user, I want `gup_core::scatter(rows).x(|r| r.a).y(|r| r.b)` to give
> me a finished chart with axes, colours, sizes and a legend, so that I write
> five lines, not fifty."

## Acceptance Criteria

### AC1: API

- [ ] `gup_core::scatter` and `ScatterChart<T>` exist as in Decisions, exported
      at the crate root (not in the prelude). Every public item has docs. The
      module docs open with the north-star example as a doctest that renders
      with `ChartExt::render` and asserts a non-empty image.
- [ ] Every closure in AC2–AC4 is written without type annotations (`|r| r.a`,
      `|r| r.segment.as_str()`).

### AC2: Thin sugar, byte for byte

- [ ] In a new `crates/gup-core/tests/builders.rs`, this renders byte-identical
      to the existing `gup_core/scatter` golden's plot
      (`common::scatter::plot()`) at 720 × 450:

      ```rust
      gup_core::scatter(countries())
          .x(|c| c.gdp_per_capita).y(|c| c.population).y_scale(Log::new())
          .color_value(|c| c.life_expectancy).radius(4.5)
          .title(TITLE).legends(false)
      ```

- [ ] `into_plot()` of the same builder resolves to the same `Layout`.

### AC3: Defaults, with a golden

- [ ] A golden, `gup_core/builder_scatter`, at 720 × 450: 300 deterministic rows
      with `.x`, `.y`, `.color(|r| r.segment.as_str())` over 4 keys,
      `.size(|r| r.weight)`, `.opacity(0.7)`, `.legend_title("Segment")`,
      `.grid(Axes::Both)` and `.title(..)`. The implementer reads it:
  - title, tick labels and grid;
  - four Okabe-Ito colours;
  - disc areas growing with weight;
  - a titled swatch legend at the right;
  - every disc inside the plot rect.
- [ ] Pixel checks: the smallest-weight disc's radius is 2 px (±0.5) and the
      largest's 12 px (±0.5), measured by ink extent at isolated rows. A disc's
      centre colour is its segment's Okabe-Ito colour at opacity 0.7 over white
      (±3/255).
- [ ] `.color_scale(Categorical::from_palette([..4 colours..]))` uses those
      colours, whether called before or after `.color(..)`.

### AC4: Escape hatches and picking

- [ ] `layer_mut().stroke(Color::BLACK).stroke_width(Px(1.0))` before the first
      render draws stroked discs (pixel check on one edge).
- [ ] `pick(at)` returns `Some(&row)` for the row under a disc's centre, and
      `None` in empty space.
- [ ] Calling a layer setter (`.x(..)`) after the first `render` panics with the
      message in Decisions (`#[should_panic(expected = ...)]`). Calling a
      plot-level setter (`.title(..)`) afterwards works and changes the next
      render.
- [ ] `gup_core::show(chart)` compiles for a `ScatterChart` (doctest `no_run`).

### AC5: External crate

- [ ] `crates/gup-core/tests/compile_pass/` gains `builder_scatter.rs`, the
      north star with owned rows, which compiles with no annotations.

## Decisions

- **Where**: `crates/gup-core/src/builder/mod.rs` (shared code, the
  `plot_setters!` macro, module docs) and `src/builder/scatter.rs`. GUP-463 and
  GUP-464 add `line.rs` and `bar.rs` beside it.
- **Shape**: a builder is a chart.

  ```rust
  pub fn scatter<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> ScatterChart<T>;

  #[must_use]
  pub struct ScatterChart<T> { /* plot: Plot, rows: Option<Vec<T>>, spec */ }

  impl<T: Send + Sync + 'static> ScatterChart<T> {
      // Positions: plain closures, bound to the plot's scales (as `Selection::x`).
      pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn x_scale<S: PositionScale>(self, scale: S) -> Self;   // default Linear::new()
      pub fn y_scale<S: PositionScale>(self, scale: S) -> Self;
      // Colour: the last of color / color_value / fill wins.
      pub fn color(self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self;
      pub fn color_scale(self, scale: Categorical) -> Self;        // default Categorical::okabe_ito()
      pub fn color_value<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn color_value_scale(self, scale: Sequential) -> Self;   // default Sequential::viridis()
      pub fn fill(self, color: Color) -> Self;
      pub fn legend_title(self, title: impl Into<String>) -> Self;
      // Size: the last of size / radius wins.
      pub fn size<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn size_scale(self, scale: Pow) -> Self;  // default Pow::sqrt().range(Px(2.0), Px(12.0))
      pub fn radius(self, px: f32) -> Self;         // default: Circle's 3.5
      pub fn opacity(self, opacity: f32) -> Self;   // fill_opacity; panics outside 0..=1
      pub fn tooltip(self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self;
      // Plot-level, from plot_setters!: title, x_title, y_title, theme, grid(Axes), legends(bool).
      // Escape hatches (each materialises):
      pub fn layer_mut(&mut self) -> &mut Selection<T, Circle>;
      pub fn plot_mut(&mut self) -> &mut Plot;
      pub fn into_plot(self) -> Plot;
      pub fn pick(&mut self, at: geom::Point) -> Option<&T>;  // not the `Point` scale
  }
  impl<T: Send + Sync + 'static> Chart for ScatterChart<T> { /* materialise, then delegate to the Plot */ }
  ```

- **By value**: every setter takes `self` and returns `Self`, so a chain on a
  temporary works. Rendering needs `let mut chart` (§12 risk 5).
- **Rows**: owned, `impl IntoIterator<Item = T>`, as `Plot::circles`. The
  north-star `&data` becomes `data` or `data.iter().cloned()`. A borrowed row
  cannot meet `Selection`'s `'static`, and an `IntoRows` trait would overlap
  with iterators (coherence).
- **Materialisation**: plot-level setters apply to the inner `Plot` at once.
  Layer settings are recorded and applied, in call order, when the chart is
  first resolved, handled, picked or its plot or layer is borrowed. After that,
  a layer setter panics with
  `"set layer encodings before the chart is first drawn; change a drawn chart through layer_mut()"`.
  Each builder documents this under `# Panics`.
- **Colour and size** are fields (accessor plus scale), applied at
  materialisation, so `color` and `color_scale` (and `size` and `size_scale`)
  work in either order. `color` encodes through one `ScaleRef<Categorical>`
  created then (`encode_key`), so the legend comes from the existing hook.
- **Not in this story**: horizontal orientation, custom tick formatters, per-row
  explicit colours. The old builders' versions of these are listed in GUP-466's
  migration table.

## Escalation Triggers

- Any AC2–AC4 closure needs a type annotation.
- AC2 is not byte-identical.
- `color` needs a change to `EncodeFn::encode_key`'s bounds to accept the stored
  key closure.
- A `Chart` method other than the ones listed would need to materialise, for
  example a `&self` method that must see layers.

## Out of Scope

- `line` and `bar`: GUP-463, GUP-464.
- Deleting the old builders and migrating their users: GUP-465, GUP-466.
- Closures for colour and size on `Selection` itself (GUP-447's parked
  follow-up).

## Technical Tasks

- [ ] `builder/mod.rs`: module docs, key and number closure aliases, the
      `plot_setters!` macro, materialisation helper.
- [ ] `builder/scatter.rs`: `scatter`, `ScatterChart`, `Chart` impl.
- [ ] `lib.rs` exports; `tests/builders.rs` (AC2–AC4); golden; compile-pass
      case.

## Dependencies

### Prerequisite Stories

- GUP-447: RFC-001 S8d Mark Adders and Default Axes ✅ — position closures.
- GUP-434: RFC-001 S7e Chart Trait ✅ — `Chart`, `ChartExt`.
- GUP-460: RFC-001 S10a Grid Lines 📋 — the `grid` plot-level setter.

### Enables Stories

- GUP-463, GUP-464: `line` and `bar` follow this module's pattern.
- GUP-465: dogfood task 3.

## Testing Strategy

- **Integration**: byte-identity with the reference scatter; golden with
  defaults; pixel checks; panics; picking (`tests/builders.rs`).
- **Compile**: a `compile_pass` case and doctests.
- **Visual**: `gup_core/builder_scatter.png`, read by the implementer.

## Success Metrics

- [ ] The north-star scatter is five lines, with no annotations.
- [ ] Builder output equals hand-built `Plot` output byte for byte.

## Risk Assessment

- **Medium**: a stored `for<'a> Fn(&'a T) -> &'a str` closure re-wrapped for
  `encode_key` may hit higher-ranked closure inference limits. _Mitigation_:
  store it as `Arc<dyn for<'a> Fn(&'a T) -> &'a str + Send + Sync>` and pass a
  closure whose signature is fixed by `encode_key`'s bound. Escalate if that
  fails.
- **Low**: the panic-after-draw rule surprises users. _Mitigation_: documented,
  with a message naming the fix.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: `gup_core/builder_scatter.png`
- [ ] `mask wasm-size` passes (no change expected: the harness does not use the
      builders)
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
