---
implementer: sonnet
implementer-reason:
  API shape, binding point and type erasure are specified in Decisions; follows
  ColumnEncoding and DynPositionScale patterns
---

# GUP-447: RFC-001 Step S8d: Mark Adders and Default Axes on `Plot`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-17

## Context

RFC-001 S8 ends with "`hello_world` in ≤10 lines", and the strategic review's
north star is "a user writes ~5–10 lines and gets a publication-quality chart".
Today the shortest scatter in `gup-core` reads:

```rust
let mut plot = Plot::new();
let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
plot.add(Selection::<(f64, f64), Circle>::new(points))
    .x(x.encode(|p: &(f64, f64)| p.0))
    .y(y.encode(|p: &(f64, f64)| p.1));
```

A newcomer has to learn four things before seeing a dot: that a plot has no
scales until given two, that `plot.x(..)` returns a handle to encode through,
that the mark is a type parameter of `Selection`, and that a position is
`scale.encode(accessor)`. `Plot::resolve` fails without both scales (`plot.rs`
line 262). The builders (`gup::scatter(..)`, RFC-001 S10) will hide some of
this. But `Plot` is the type users drop down to, and pre-alpha is the time to
make it read naturally (owner decision 2: breaking API reset approved).

This story gives `Plot` default linear axes, one adder per library mark, and
plain closures on position channels that encode through the plot's own x or y
scale:

```rust
let mut plot = Plot::new();
plot.circles(points).x(|p: &(f64, f64)| p.0).y(|p: &(f64, f64)| p.1);
```

The explicit form keeps working unchanged. GUP-448 builds the 10-line hello
world on this, plus GUP-446's tooltip.

## Read First

- RFC-001 §8 "Builders" and "Drop-down access" bullets (lines 603–613).
- `crates/gup-core/src/plot.rs` lines 99–110 (fields), 158–238 (`new`, `x`, `y`,
  `add`, `layer_mut`) and 258–305 (the start of `resolve`, through the fit
  pass).
- `crates/gup-core/src/encoding.rs` lines 160–210 (`EncodeFn`), 580–610
  (`ColumnValue`), 860–1064 (`IntoEncoding` impls, `Encoding`,
  `DynColumnEncoding`, `ColumnEncoding`).
- `crates/gup-core/src/scale/mod.rs` lines 85–126 (`ScaleRef`) and 203–286
  (`DynPositionScale`).
- `crates/gup-core/src/selection.rs` lines 256–266 (`attr`) and 540–572
  (`Layer::begin_fit`, `fit_domains`).
- `crates/gup-core-macros/src/mark.rs` lines 523–560 (the generated setters).
- `crates/gup-core/tests/compile_fail/bare_number.stderr`.

## User Story

> "As a newcomer, I want to add a layer of marks to a plot and say which field
> is x and which is y with plain closures, so that my first chart takes a few
> readable lines."

## Acceptance Criteria

### AC1: Adders and default axes

- [ ] `Plot::{circles, rects, segments, line}` exist as in Decisions, and each
      returns the new layer.
- [ ] `Plot::new()` has automatic `Linear` x and y scales. A plot that never
      calls `plot.x(..)`/`plot.y(..)` resolves, with axes fitted to the data.
- [ ] `plot.x(scale)`/`plot.y(scale)` still replace the scale and return a
      handle. Existing tests and goldens pass unchanged.

### AC2: Closures on position channels

- [ ] The `Plot` doctest (an external crate) is rewritten to the short form:
      `plot.circles(rows).x(|r: &Row| r.a).y(|r: &Row| r.b)` with four rows,
      rendered at 320×200, asserting that the pixel at each row's position has
      the default fill (`#0072B2`, ΔE < 2), with positions read by interpolating
      `layout.x_ticks`/`y_ticks`. The old long form stays in `Selection`'s docs.
- [ ] Golden equivalence: a test renders the same rows once in the long form
      (explicit `Linear` handles, `encode`) and once in the short form. The two
      images are byte-identical, and so are the two `Layout`s.
- [ ] `rects` with closures on `x`, `y`, `x_end` and `y_end` (edge mode, width
      and height 0) and `segments` with closures on all four endpoints render
      the expected boxes and segments (pixel checks at two corners each). `line`
      with closures renders through the plot's scales.
- [ ] Replacing the x scale after binding (`plot.x(Log::new())` between two
      resolves) re-binds: the second render uses log positions (the pixel check
      moves accordingly) and re-uploads the columns once.
- [ ] A zoom (`handle(Wheel)`) on a short-form plot writes 0 `Column` bytes (the
      binding is not redone).

### AC3: Errors

- [ ] A closure on `radius` (role `Size`) or `width` (no role) fails at resolve
      with `Error::Config` whose message names the mark, the channel, and the
      fix in Decisions.
- [ ] A closure on `x` when the plot's x is `Band` or `Point` fails at resolve
      with the key-scale message in Decisions.
- [ ] `.fill(|r: &Row| r.v)` (a `Color` channel) does not compile: a new
      trybuild case `tests/compile_fail/closure_on_color.rs` whose `.stderr`
      shows `IntoEncoding`'s `on_unimplemented` message.

### AC4: Changed default, searched

- [ ] `grep -rn "set both position scales\|\"plot scales\"" crates/` finds
      nothing left that expects the old error. Each test that asserted it now
      asserts a successful default-axis resolve, and the list is in the
      Retrospective.
- [ ] `compile_fail/bare_number.stderr` changes only by the new `IntoEncoding`
      candidate (`AxisMarker`). No other `.stderr` changes.
- [ ] `mask wasm-browser` passes. `mask wasm-size` passes (re-record
      `wasm.scatter.gz_bytes` if it moved).

## Decisions

```rust
impl Plot {
    /// Add a layer of circles over `rows` and return it.
    pub fn circles<T: Send + Sync + 'static>(&mut self, rows: impl IntoIterator<Item = T>)
        -> &mut Selection<T, Circle>;
    pub fn rects<T: Send + Sync + 'static>(&mut self, rows: impl IntoIterator<Item = T>)
        -> &mut Selection<T, Rect>;
    pub fn segments<T: Send + Sync + 'static>(&mut self, rows: impl IntoIterator<Item = T>)
        -> &mut Selection<T, Segment>;
    /// One polyline through `rows`, in order.
    pub fn line<T: Send + Sync + 'static>(&mut self, rows: impl IntoIterator<Item = T>)
        -> &mut Selection<T, Line>;
}

/// Marker for a plain closure on a position channel (see `IntoEncoding`).
pub struct AxisMarker<D>(PhantomData<fn() -> D>);

impl<T: 'static, F, D> IntoEncoding<T, Px, AxisMarker<D>> for F
where
    F: Fn(&T) -> D + Send + Sync + 'static,
    D: ColumnValue,
{ /* Encoding::Axis(Arc::new(move |r| f(r).to_f64())) */ }
```

- Each adder is
  `self.add(Selection::new(rows.into_iter().collect::<Vec<_>>()))`.
- **Default axes**: `Plot::new()` sets `x` and `y` to
  `ScaleRef::new(Linear::new())` (automatic domain). `Plot` stops deriving
  `Default` for those fields; `Default` calls `new`. Resolve's "set both
  position scales" error goes away.
- **New variant** `Encoding::Axis(Arc<dyn Fn(&T) -> f64 + Send + Sync>)`. `attr`
  stores it like any encoding. Matches that list variants exhaustively treat an
  unbound `Axis` like `None`.
- **Binding** happens in `Plot::resolve` before `begin_fit`: the new
  `Layer::bind_axes(&mut self, generation: u64, x: &dyn DynPositionScale, y: &dyn DynPositionScale) -> Result<()>`.
  `Plot.axes_generation: u64` starts at 1 and is bumped by `x()` and `y()`.
  `Selection` keeps the closures in
  `axes: Vec<Option<Arc<dyn Fn(&T) -> f64 + Send + Sync>>>` (per slot, set by
  `attr` with `Axis`, cleared by `attr` with anything else) and the generation
  it last bound, `axes_bound: u64`. When they differ, each axis slot's encoding
  becomes `Encoding::Column(Box::new(AxisEncoding { .. }))` over the plot's
  scale for the channel's role, and the columns are invalidated (as `attr`
  does). Otherwise nothing changes, so zooming never re-uploads.
- **Type erasure**: `pub(crate) trait DynEncodeFn: Send + Sync` with `links`,
  `fit`, `fit_dictionary`, `image`, `encode_legend`, `encode_legend_over` and
  `apply_theme`, the object-safe half of `EncodeFn`, blanket-implemented for
  every `E: EncodeFn`. `DynPositionScale` gains
  `fn dyn_encode(&self) -> Box<dyn DynEncodeFn>` (a clone of the `ScaleRef`, so
  state is shared).
  `AxisEncoding<T> { accessor, func: Box<dyn DynEncodeFn>, extent: Option<(f64, f64)> }`
  implements `DynColumnEncoding<T>` with the same bodies as `ColumnEncoding`.
- **Roles**: `Role::X` channels (`x`, `x_end`) bind to the plot's x, and
  `Role::Y` to y. Any other channel with an `Axis` encoding fails with
  `Error::config("channel", ..)`, and if the bound scale's first link has
  `ColumnFormat::U32` input (`Band`, `Point`) it fails with
  `Error::config("plot x", ..)` (or `"plot y"`). The messages, with the mark,
  channel and axis substituted:

  ```text
  `Circle::RADIUS` has no plot scale to encode a closure through; give it a
  scale, e.g. `.radius(Pow::sqrt().range(Px(2.0), Px(12.0)).encode(|r: &T| …))`,
  or a constant such as `Px(4.0)`

  the x scale reads dictionary keys (Band or Point); encode keys with
  `x.encode_key(|r: &T| r.key.as_str())`
  ```

- Only `Px` channels get the closure form. Closures for colour and size, with
  default `Categorical`/`Sequential`/`Sqrt` scales, are a later story.
- `Plot`'s docs lead with the short form. The prelude does not change.

## Escalation Triggers

- A closure needs more annotation than `|r: &T|` to infer, in any AC test.
- Binding needs a change to `EncodeFn`'s or `ShaderFn`'s public signatures.
- A short-form plot's image or `Layout` differs from the long form's (AC2's
  byte-identity fails).
- Any `.stderr` snapshot changes beyond the new candidate impl.
- A test outside `crates/gup-core` depended on the "set both position scales"
  error.

## Out of Scope

- `gup::scatter`/`line`/`bar` builders: RFC-001 S10.
- Closures on colour and size channels, and key closures on `Band` axes: a
  follow-up (record it in the Retrospective).
- The hello world itself: GUP-448.

## Technical Tasks

- [ ] `AxisMarker`, the `IntoEncoding` impl and `Encoding::Axis`.
- [ ] `DynEncodeFn`, `DynPositionScale::dyn_encode` and `AxisEncoding`.
- [ ] `Selection::axes`/`axes_bound`, `Layer::bind_axes`, and binding in
      `Plot::resolve`.
- [ ] Default scales in `Plot::new`, the adders, `axes_generation`.
- [ ] Tests for AC2–AC3, the trybuild case, the `bare_number` re-bless, and the
      default-change search (AC4).

## Dependencies

### Prerequisite Stories

- GUP-424: RFC-001 S7a multi-layer `Plot` ✅ — the fit pass binding runs before.
- GUP-433: shared key dictionaries ✅ — the `ScaleRef` sharing that `dyn_encode`
  relies on.

None of GUP-444 to GUP-446 is required; this story can run in parallel with
them.

### Enables Stories

- GUP-448 (S8e `GupApp` and hello world) — the 10 lines use `circles` and
  closures.
- RFC-001 S10 builders become thin wrappers over the adders.

## Testing Strategy

- **Unit tests**: binding and re-binding by generation; error messages.
- **Integration tests**: long form vs short form byte-identity, edge rects,
  segments, line, zoom uploads.
- **Compile-fail**: `closure_on_color.rs`.
- **Browser**: `mask wasm-browser`.

## Success Metrics

- [ ] The two-line scatter in Context renders identically to the five-line one.
- [ ] Zooming a short-form plot writes 0 column bytes.

## Risk Assessment

- **Medium**: a new `Encoding` variant touches many matches in `selection.rs`
  (27 sites). _Mitigation_: most are `if let Column`. Binding before fit means
  later stages only ever see `Column`.
- **Low**: an unregistered `ScaleRef` used for an explicit X encode now draws
  next to a default axis, where it used to be an error. _Mitigation_: this was
  already possible with an explicit `plot.x(..)`. It is documented on `Plot::x`.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
