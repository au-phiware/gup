---
implementer: sonnet
implementer-reason:
  fit-pass union, z-order API and resolve loop settled in Decisions; dictionary
  sharing and the Chart trait were split out (GUP-433, GUP-434)
---

# GUP-424: RFC-001 Step S7a: Multi-Layer Plot and Shared Scales

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-16)
**Created**: 2026-10-11

## Context

`Plot::resolve` (`crates/gup-core/src/plot.rs` lines 197–270) refuses any layer
count but one: `let [layer] = self.layers.as_mut_slice() else { … }`. The
storage is already multi-layer (`layers: Vec<Box<dyn Layer>>`, `Plot::add`
pushes), and `Scene::push` already orders items by `z` with insertion order as
the tiebreak (`scene.rs` lines 56–59). What is missing is resolving every layer,
unioning their domains, and drawing their items in z-order.

Domains do not union today, even within one layer. Every fit goes through
`ShaderFn::fit_domain`; a shared scale is a `ScaleRef<S>` whose `fit_domain`
forwards to `S::fit_domain` (`scale/mod.rs` lines 136–138), and
`Linear::fit_domain` (`scale/linear.rs` line 112) replaces the domain: the last
channel fitted wins. That breaks two layers sharing `x`, and also one layer with
two channels on one scale ([GUP-423](GUP-423_RFC_001_S6b_Segment_Mark.md)'s
`x`/`x2`, [GUP-429](GUP-429_RFC_001_S6c_Rect_Mark_And_Derived_Constants.md)'s
`x`/`x2`). RFC-001 §8: "Layers that bind the same role share a scale, and its
domain is the union of their f64 chunk stats." Because every shared scale is a
`ScaleRef` (cloned into each encoding, sharing one `Arc`), the union can live
there: each resolve starts a fit pass, and every fit in the pass unions into it.

On 2026-10-16 this story was split to fit one session. Shared key dictionaries
(two layers giving one key different codes) moved to
[GUP-433](GUP-433_RFC_001_S7d_Shared_Key_Dictionaries.md); the
`Chart`/`ChartExt` trait moved to [GUP-434](GUP-434_RFC_001_S7e_Chart_Trait.md);
resolve caching is deferred to RFC-001 S8.

## Read First

- RFC-001 §8 "Chart trait and composition", Design (lines 578–617): shared
  domains and z-order bullets.
- `crates/gup-core/src/plot.rs` lines 1–9 (module doc), 113–119 (`Plot`),
  197–300 (`resolve` steps 1–7) and 360–372 (the marks item).
- `crates/gup-core/src/scale/mod.rs` lines 84–150 (`ScaleRef`).
- `crates/gup-core/src/encoding.rs` lines 75–104 (`ShaderFn::fit_domain`,
  `fit_keys`) and 811–870 (`DynShaderFn` and its blanket impl).
- `crates/gup-core/src/selection.rs` lines 405–468 (`Layer` trait,
  `fit_domains`, `overhang`).
- `crates/gup-core/src/scene.rs` lines 14–20 (`Z_GRID`, `Z_AXES`, `Z_TITLE`) and
  56–59 (`Scene::push`).

## User Story

> "As a visualization developer, I want to add more than one data layer to a
> `Plot`, with domains unioned across every layer and channel that shares a
> scale, so that a composite chart (for example a forecast line plus observed
> points) renders correctly instead of erroring."

## Acceptance Criteria

### AC1: Fit passes union domains

- [x] Unit test on `ScaleRef<Linear>`: `begin_fit`, `fit_domain((0, 10))`,
      `fit_domain((5, 30))` gives domain `(0, 30)`; a second pass with
      `begin_fit`, `fit_domain((2, 3))` gives `(2, 3)` (a domain can shrink when
      data does).
- [x] An explicit domain (`Linear::new().domain(..)`) is unaffected by fits, as
      today.

### AC2: Several layers resolve

- [x] Two `Circle` layers sharing `x`/`y`, x data 0..10 and 5..30: after
      `resolve`, `x.read().current_domain()` covers 0..30 and every disc of both
      layers lies inside `layout.plot` (the `sqrt_radius_scatter` assertion,
      lines 70–82 of `tests/scales_png.rs`).
- [x] The plot inset uses the largest overhang across layers:
      `layout.mark_overhang` equals the larger layer's radius.
- [x] A plot with no layers fails with `Error::config("plot layers", …)` saying
      to add one with `plot.add(..)`.

### AC3: Per-layer z-order

- [x] `Selection::z(&mut self, z: i32) -> &mut Self` (default 0). A test
      overlaps two opaque discs from two layers at one pixel: with default z the
      second-added layer's colour shows; with the first layer at `z(1)` the
      first layer's colour shows.
- [x] `z` outside `-99..=99` fails `resolve` with
      `Error::config("layer z",     …)` naming the allowed range (marks must
      stay between the grid and the axes).

### AC4: Golden `composite_two_layers`

- [x] Golden `gup_core/composite_two_layers` (720×450): a "forecast" layer of
      small grey circles (r 2, every 0.25 on x) and an "observed" layer of
      larger blue circles (r 5) at `z(1)`, sharing `x` and `y`, auto domains.
- [x] Read by eye: both layers visible, correctly placed relative to each other,
      observed points on top, all inside the plot rect. The retrospective
      describes the PNG.

### AC5: One-layer charts are unchanged

- [x] Every existing gup-core golden passes without re-blessing.
- [x] `plot.rs`' module doc no longer says "exactly one layer".

## Decisions

- **`ScaleRef` internals**: `ScaleRef<S>(Arc<Shared<S>>)` with
  `struct Shared<S> { scale: RwLock<S>, fit: Mutex<Option<(f64, f64)>> }`
  (private). `Clone`, `Debug`, `Default`, `new`, `read`, `write` keep their
  signatures. GUP-433 will add a field to `Shared`.
- **`begin_fit`**: a provided `fn begin_fit(&self) {}` on `ShaderFn` (doc:
  "Start a fit pass: the next fits union from scratch"), forwarded by
  `DynShaderFn`. `ScaleRef` overrides it to clear `fit`.
  `ScaleRef::fit_domain(extent)` unions `extent` into `fit` and calls
  `self.write().fit_domain(union)`. `fit_keys` is unchanged here.
- **`Layer::begin_fit(&self)`**: for every `Encoding::Column`, calls `begin_fit`
  on each of `links()`. `Plot::resolve` calls it on every layer before any
  `fit_domains`, so a scale shared by many channels is cleared once per pass
  before any fit.
- **Resolve loop**: step 1 fits every layer in insertion order; step 5's inset
  uses the maximum `overhang()`; step 6 prepares every layer inside the existing
  error scope; `release()` runs per layer after its upload; step 7 pushes one
  `ItemKind::Marks` item per layer with `z: layer.z()` and the one plot clip, in
  insertion order (so `Scene::push`'s tiebreak gives insertion order).
- **`z`**: stored on `Selection`, read through a new `Layer::z(&self) -> i32`.
  Valid range `-99..=99` (`Z_GRID < z < Z_AXES`), checked in `resolve`.
- **`Layout`** is unchanged apart from `mark_overhang` meaning the maximum.

## Escalation Triggers

- Any existing golden or test changes (one-layer behaviour must be identical).
- Something other than `Plot::resolve` calls `fit_domain` on a shared `ScaleRef`
  and relies on last-wins.
- `Then` chains need changes to union correctly (they should not: `Then::fit`
  fits the first link, then the next with the image).

## Out of Scope

- One dictionary per shared key scale: GUP-433.
- `Chart`/`ChartExt`, `Plot::layer_mut`: GUP-434.
- Resolve caching: RFC-001 S8 (not yet a story).
- Axis titles, `Theme`: GUP-425. Legends: GUP-426.
- Secondary axes, mixed row types (`ComposedChart` replacement): RFC-001 S11.

## Technical Tasks

- [x] `ScaleRef` → `Arc<Shared<S>>`; `begin_fit` on `ShaderFn`, `DynShaderFn`,
      `ScaleRef`, `Layer`.
- [x] Rewrite `Plot::resolve`'s steps 1, 5, 6, 7 as loops; zero-layer error.
- [x] `Selection::z`, `Layer::z`, range check.
- [x] Tests for AC1–AC3; golden `composite_two_layers`; update the module doc.

## Dependencies

### Prerequisite Stories

- GUP-419: RFC-001 S5b ✅ — the scale family whose fits this unions.
- GUP-414: RFC-001 S4a ✅ — chunk stats.
- GUP-401: RFC-001 S3 ✅ — `Scene`, z constants.

### Enables Stories

- GUP-433: shared key dictionaries across layers.
- GUP-434: `Chart`/`ChartExt` over a multi-layer `Plot`.
- GUP-423: `Segment`'s `x`/`x2` union. GUP-432: `Line`.
- GUP-426: legends from several layers.

## Testing Strategy

- **Unit tests**: fit-pass union and reset; z range check.
- **Integration tests**: two-layer domain and z-order pixel tests.
- **Visual validation**: `gup_core/composite_two_layers`.
- **Performance**: none expected; `begin_fit` is one lock per scale per resolve.

## Success Metrics

- [x] A two-layer chart resolves with one unioned domain per shared scale.

## Risk Assessment

- **Low**: a `Mutex` per `ScaleRef` on wasm. _Mitigation_: `std::sync::Mutex`
  works on wasm32 single-threaded, as `RwLock` already does here.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `ScaleRef<S>` is now `Arc<Shared<S>>` (scale lock plus a fit-pass mutex).
  `ShaderFn::begin_fit` (provided no-op, forwarded by `DynShaderFn`) starts a
  pass; `ScaleRef::fit_domain` unions every finite extent in the pass before
  fitting. `Layer::begin_fit` and `Layer::z`; `Selection::z`.
- `Plot::resolve` fits, prepares and releases every layer, insets by the maximum
  overhang, and pushes one marks item per layer at its `z`. Zero layers and `z`
  outside `-99..=99` are `Error::config`.
- Tests: `fit_passes_union_domains` (unit), `tests/multi_layer_png.rs` (5 tests,
  golden `gup_core/composite_two_layers`). gup-core suite passes with no
  existing golden changed; `mask old-path-loc` 28910; `mask perf-budget`: every
  check passed (zoom cpu median 0.674 ms, gpu pass 2.989 ms, wasm scatter 304361
  B gz).
- Non-finite extents bypass the union so the scale still reports them.

## Retrospective

**Completed**: 2026-10-16

### Key Technical Learnings

#### Union lives in the shared handle

- **Challenge**: `fit_domain` replaces the domain, so shared scales were
  last-wins.
- **Solution**: a fit-pass accumulator inside `ScaleRef`, cleared by `begin_fit`
  once per resolve before any layer fits.
- **Pattern**: shared mutable state with an explicit pass boundary beats
  teaching every scale to union.

### Architectural Decisions

#### Non-finite extents skip the union

- **Decision**: a NaN/inf extent goes straight to the scale.
- **Reasoning**: `f64::min/max` ignore NaN and would hide the scale's error.
- **Trade-off**: none visible.

### Verification

`composite_two_layers.png` (720x450, title "Forecast and observed"): a grey
dotted sinusoid-plus-trend (small r 2 forecast circles every 0.25 on x) with 13
larger blue r 5 observed points scattered about it, drawn on top. Both layers
share one x axis 0..12 and y axis 6..24; all marks sit inside the plot rect.

### Development Workflow Insights

- The agent sandbox refuses compound shell commands (heredocs with python,
  `eval`); write scripts with the Write tool and run them plainly.
- `CARGO_TARGET_DIR` could not be exported via `eval`; builds went to the
  inherited directory, which the per-checkout member artifacts keep safe.

### Follow-up Stories

None.
