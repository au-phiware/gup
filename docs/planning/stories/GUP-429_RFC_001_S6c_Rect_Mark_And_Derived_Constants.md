---
implementer: sonnet
implementer-reason:
  channel shape, Derived API and WGSL settled in Decisions; the mark follows
  Circle and gup::coverage's rect_quad, the encoding follows Encoding::Const
---

# GUP-429: RFC-001 Step S6c: `Rect` Mark and Scale-Derived Constants

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-16

## Context

RFC-001's S6 step lists "Circle, Rect, Rule, Segment/Line". This story adds
`Rect`, split out of [GUP-422](GUP-422_RFC_001_S6a_Derive_Mark_V2.md) on
2026-10-16. RFC-001's "S5b findings → Proposed adjustments" records the need it
must meet: "A `Rect` or bar mark needs a width that follows
`Band::band_width()`. That is a value from the scale, not from each row, so it
should update with the domain like any other uniform", and "S10 (bar builder)
needs `Band::band_width()` and `KeyEncoded::domain` for sorted categories, and
S6's width-from-scale above".

Today a channel is driven by a constant (`Encoding::Const`, a uniform field
written in `Selection::prepare`) or a column (`Encoding::Column`). Neither fits
a bar width: a constant is frozen at `attr` time, and a per-row column wastes
bytes and must be rewritten when a key is appended. `Selection::prepare` already
rebuilds the `Encodings` uniform bytes on every resolve (zoom, pan and appends
write uniforms only), after `Plot::resolve` has set the scale ranges (step 5). A
third encoding, a closure evaluated there, gives a uniform that follows the
scale.

`Band` maps a key to the **centre** of its band (`band.rs` lines 107–111: "a bar
(`band_width` wide) is centred on it"), and a bar's value axis runs from the
value to a baseline, `y(0)`. So `Rect` takes a centre plus an extent on an axis
whose extent is non-zero, and two edges otherwise.
[GUP-421](GUP-421_Analytic_AA_Rules_Rects_1x_MSAA.md) provides the coverage:
`gup::coverage::rect_quad(lo, hi, vertex_index, dpr)` and
`box_coverage(local, half)`, exact pixel area for axis-aligned boxes.

## Read First

- RFC-001 "GUP-421 findings" (lines 3016–3055), the `gup::coverage` bullet.
- `crates/gup-wgsl/library/coverage.wgsl` (all) and
  `crates/gup-wgsl/library/circle.wgsl` (all; the mark contract).
- `crates/gup-core/src/marks.rs` (after GUP-422: `Circle` with the derive).
- `crates/gup-core/src/encoding.rs` lines 658–735 (`ConstMarker`, `IntoEncoding`
  impls, `Encoding`).
- `crates/gup-core/src/selection.rs` lines 322–342 (`glue()`), 437–468
  (`overhang`) and 470–498 (`prepare`'s `Encodings` bytes).
- `crates/gup-core/src/scale/band.rs` lines 107–200 (`Band`, `band_width`).
- `crates/gup-core/src/plot.rs` lines 819–890 (test
  `a_new_key_grows_the_domain_with_a_uniform_write_only`, the pattern for AC3).

## User Story

> "As a visualization developer, I want a `Rect` mark whose width can follow a
> band scale's `band_width()`, so that bar charts and heatmaps size their bars
> from the scale and stay correct when a category is appended."

## Acceptance Criteria

### AC1: `Rect` mark

- [ ] `gup_core::Rect` (re-exported at the crate root and in the prelude with
      `RectAttrs`) is defined with `#[derive(Mark)]` over
      `crates/gup-wgsl/library/rect.wgsl` (`gup::marks::rect`), with the
      channels in Decisions.
- [ ] A render test draws one rect with `width`/`height` (centre form) and one
      with `x2`/`y2` (edge form, corners given in reverse order) at known pixel
      coordinates; for each, the pixels 1 px inside every edge are the fill
      within 2/255 and the pixels 1 px outside are background within 2/255.

### AC2: `Derived` constants

- [ ] `Derived<V>` drives any channel: a test binds `Circle::RADIUS` to
      `Derived::new(move || Px(*r.lock().unwrap()))` reading a shared
      `Arc<Mutex<f32>>`, resolves, changes the value, resolves again, and the
      disc's ink radius (measured along a row of pixels) follows within 0.5 px;
      `UploadStats` shows no column bytes on the second resolve.
- [ ] `ScaleRef<Band>::band_width_px()` returns a `Derived<Px>` whose value is
      `|band.band_width()|` at evaluation time; a doctest on it compiles and
      runs.

### AC3: A bar width follows an appended key

- [ ] A test (in `plot.rs`' tests, after the AC3 pattern above) draws bars over
      a `Band` x with keys `A`, `B`, `C`, `width = x.band_width_px()`; it
      appends rows with a new key `D` and resolves again. The second resolve's
      column writes are only the appended rows' bytes (no column exists for
      `width`), and each bar's measured ink width equals
      `round(x.read().band_width())` within 1 px both before (3 keys) and after
      (4 keys), the latter being smaller.

### AC4: Bar-chart golden

- [ ] Golden `gup_core/rect_bars` (720×450): six monthly values on a `Band` x
      (`domain([Jan..Jun])`), `Linear` y with explicit domain `0..max`, `Rect`
      with `x = x.encode_key(..)`, `width = x.band_width_px()`,
      `y = y.encode(..)`, `y2 = Derived` of `y.read().eval(0.0)`.
- [ ] Read by eye: six equal-width bars, each centred on its month label,
      bottoms within 2 px of the x axis line, tops at their values, no bar
      outside the plot rect. The retrospective describes the PNG.
- [ ] A pixel test asserts each bar's measured left/right edges are within 1 px
      of `centre ∓ band_width / 2`.

### AC5: Horizontal bars

- [ ] A render test draws horizontal bars (`y` a `Band`,
      `height =     y.band_width_px()`, `x` the value, `x2` a `Derived`
      baseline) and checks each bar's measured extent along both axes within 1
      px.

## Decisions

- **Channels**, in this order (the derive's slot order and `RectIn`'s field
  order):
  - `x`: `Px`, role `X`, default 0
  - `y`: `Px`, role `Y`, default 0
  - `width`: `Px`, no role, default `Px(0.0)`
  - `height`: `Px`, no role, default `Px(0.0)`
  - `x2`: `Px`, role `X`, default 0
  - `y2`: `Px`, role `Y`, default 0
  - `fill`: `Color`, role `Color`, default `Rect::DEFAULT_FILL` (`#0072B2`)

  Horizontal span: if `width != 0`, `x ± |width| / 2` (centre form); otherwise
  `min(x, x2)..max(x, x2)` (edge form). Vertical likewise with `height`, `y`,
  `y2`. A zero extent cannot be drawn anyway, so 0 is a safe "unset" value; NaN
  is not used because WGSL implementations may assume no NaNs. `width`/`height`
  have no role, so they add nothing to the plot inset.

- **WGSL** (`crates/gup-wgsl/library/rect.wgsl`): `RectIn` as above; `Varyings`
  with `@builtin(position) clip` and, at locations 0–3, `local: vec2<f32>`,
  `half: vec2<f32>`, `fill: vec4<f32>` and a flat `row: u32`; `vertex` computes
  `lo`/`hi` in logical pixels, calls
  `coverage::rect_quad(lo, hi, vertex_index, view.dpr)` and `px_to_clip`;
  `shade` returns premultiplied `fill.rgb * a, a` with
  `a = fill.a * coverage::box_coverage(local, half)`, discarding `a <= 0`. Not
  the guide shader `crates/gup-core/src/shaders/rect.wgsl`.
- **`Derived`** (in `crates/gup-core/src/encoding.rs`, re-exported at the crate
  root):

  ```rust
  pub struct Derived<V>(Arc<dyn Fn() -> V + Send + Sync>);
  impl<V: Visual> Derived<V> {
      pub fn new(f: impl Fn() -> V + Send + Sync + 'static) -> Self;
  }
  impl<V> Clone for Derived<V> { /* Arc clone */ }
  pub enum DerivedMarker {}
  impl<T, V: Visual> IntoEncoding<T, V, DerivedMarker> for Derived<V> { /* Encoding::Derived */ }
  ```

  `Encoding` gains `Derived(Arc<dyn Fn() -> ConstValue + Send + Sync>)`. The
  glue treats it as `ChannelSource::Const` (the existing `_` arm);
  `Selection::prepare` writes `f().bytes()` where it writes a constant;
  `overhang` evaluates it like a constant for `Size` channels; `Debug` prints
  `Derived`. It is evaluated on every `prepare`, i.e. after ranges are set.

- **`band_width_px`**:
  `impl ScaleRef<Band> { pub fn band_width_px(&self) -> Derived<Px> }` in
  `crates/gup-core/src/scale/band.rs`, returning
  `Px(band.band_width().abs() as f32)`. Not for `Point` (it has no width).
- **Docs**: `Rect`'s doc comment shows the vertical-bar recipe (AC4's bindings)
  as a doctest.

## Escalation Triggers

- `Encoding` is matched exhaustively outside gup-core (another crate or a public
  API consumer) so the new variant breaks it.
- `prepare` runs before `set_range` on some path (for example `Plot::zoom`), so
  a `Derived` would read a stale range.
- A bar's measured width is off by more than 1 px from `band_width` in AC3 or
  AC4 for a reason other than the 1 px range inset.

## Out of Scope

- Stroke and fill opacity on `Rect` and `Circle`: GUP-430.
- Unioned domains when `x` and `x2` are both columns on one auto-domained scale
  (today the last-fitted channel wins): GUP-424. This story's tests use explicit
  domains or a `Derived` second edge.
- A bar builder (`BarChart`): RFC-001 S10, not yet a story.
- `Derived` values that change between resolves without a resolve (they are read
  only in `prepare`).

## Technical Tasks

- [ ] Add `rect.wgsl` to the library and `Rect` to `marks.rs` with the derive.
- [ ] Add `Derived`, `DerivedMarker` and `Encoding::Derived`; handle it in
      `prepare`, `overhang`, `glue()` and `Debug`.
- [ ] Add `ScaleRef<Band>::band_width_px`.
- [ ] Export `Rect`, `RectAttrs`, `Derived` (crate root and prelude).
- [ ] Tests for AC1–AC3 and AC5; golden `rect_bars` (AC4), read by eye.

## Dependencies

### Prerequisite Stories

- GUP-422: `#[derive(Mark)]` v2 📋 — defines `Rect`.
- GUP-421: Analytic AA ✅ — `gup::coverage`.
- GUP-419: RFC-001 S5b ✅ — `Band`, `encode_key`, `domain`.

### Enables Stories

- GUP-430: stroke and fill opacity on `Circle` and `Rect`.
- RFC-001 S10 bar builder (not yet a story).

## Testing Strategy

- **Unit tests**: `Derived` evaluation in `prepare`; `band_width_px` value.
- **Integration tests**: render tests with measured edges (AC1, AC3, AC5).
- **Visual validation**: `gup_core/rect_bars`, read by eye.
- **Performance**: none expected; `Derived` costs one closure call per channel
  per resolve.

## Success Metrics

- [ ] A bar chart renders with bars sized by the band scale, and appending a key
      rewrites no column bytes for the width.

## Risk Assessment

- **Low**: a reversed y range makes `band_width` negative. _Mitigation_:
  `band_width_px` returns the absolute value and the shader uses `abs`.
- **Low**: the 1 px range inset leaves the bar bottom 1 px above the axis.
  _Mitigation_: accepted (AC4 allows 2 px); the axis is drawn just outside the
  plot rect.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
