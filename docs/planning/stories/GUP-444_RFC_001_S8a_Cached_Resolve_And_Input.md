---
implementer: sonnet
implementer-reason:
  trait, input types and cache key are settled in Decisions; the rest is
  mechanical caller migration and tests
---

# GUP-444: RFC-001 Step S8a: Cached `resolve` and `Chart::handle`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-17

## Context

RFC-001 step S8 is "`GupApp`/`show` on `Chart`; `Input` dispatch; CPU `pick`;
tooltips; `hello_world` in ≤10 lines" (RFC-001 §11). It is split into six
stories, GUP-444 to GUP-449. This one reshapes the `Chart` trait that the rest
build on.

[GUP-434](GUP-434_RFC_001_S7e_Chart_Trait.md) defined `Chart` with today's
`resolve` signature, which returns an owned `Resolved`. It deferred two things
from RFC-001 §8 to S8: `handle(input)` and a cached `resolve`. Every call to
`Plot::resolve` (`crates/gup-core/src/plot.rs` line 258) refits domains,
re-measures text, rebuilds the scene and writes every layer's uniforms. That is
about 0.2 ms, but an interactive host redraws on hover and on every frame, and
S8c's tooltips need the last layout to stay put between frames. Caching is
blocked because `ScaleRef` handles can be changed outside the plot
(`x.write()`), so the plot cannot tell whether its scales changed.

`gup_core::show` (`crates/gup-core/src/show.rs`) zooms on the wheel by calling
`Plot::zoom` directly. RFC-001 §9 says hosts translate input into `gup::Input`
and call `chart.handle(..)`. This story adds that path and moves the existing
wheel zoom behind it. Drag pan, brush and configurable zoom stay in S9.

## Read First

- RFC-001 §8 "Design" (lines 578–617) and §9 "Layout, picking and events" (lines
  618–666).
- RFC-001 "S0b findings → What changed in the S0a design" items 3–4 (lines
  1294–1322).
- [GUP-434](GUP-434_RFC_001_S7e_Chart_Trait.md) "Context" and "Decisions".
- `crates/gup-core/src/chart.rs` (all of it).
- `crates/gup-core/src/plot.rs` lines 36–67 (`Layout`, `Resolved`), 99–258
  (`Plot` fields, setters, `add`, `layer_mut`, `zoom`), 258–300 (start of
  `resolve`) and 640–660 (its end).
- `crates/gup-core/src/show.rs` lines 226–240 (the wheel arm).
- `crates/gup-core/src/scale/mod.rs` lines 85–126 (`ScaleRef`, `Shared`) and
  203–232 (`DynPositionScale`).
- `crates/gup-core/src/encoding.rs` lines 40–160 (`ShaderFn`) and 1066–1100
  (`DynShaderFn` and its blanket impl).

## User Story

> "As a host (a window, a canvas, a widget), I want to pass pointer input to any
> chart and re-resolve it every frame for free when nothing changed, so that
> interaction is cheap and every host shares one behaviour."

## Acceptance Criteria

### AC1: Trait and types

- [ ] `gup_core::{Input, Button, Response}` exist as in Decisions, at the crate
      root. `Chart` has the signatures in Decisions and `Plot` implements them
      (its inherent `resolve`, `layout`, `handle` and `title_text` share them).
- [ ] The `Chart` doctest in `chart.rs` boxes a `Plot`, resolves it through
      `&mut dyn Chart`, reads `chart.layout()`, sends
      `Input::Wheel { at, lines: 1.0 }` and asserts `Response { redraw: true }`.
- [ ] Every caller compiles: `cargo check --workspace --all-targets`,
      `cargo test -p gup-core --doc` and the wasm harness
      (`./scripts/wasm_size.sh`) build.

### AC2: Cache hits do nothing

In a new `crates/gup-core/tests/resolve_cache.rs`:

- [ ] Resolving an unchanged plot a second time at the same size on the same
      `Context` returns the same `Resolved` (`std::ptr::eq` on the two
      references), leaves `Plot::resolve_generation()` unchanged, and leaves
      every `Upload` kind's count and bytes in `cx.upload_stats()` unchanged.

### AC3: Every change misses

- [ ] Each of these, applied between two resolves, increments
      `resolve_generation()` by exactly 1, and the second `Resolved` reflects
      it: a different size; a different `Context`; `title`; `x_title`;
      `y_title`; `theme`; `legends(false)`; `add`; `layer_mut` then `attr`;
      `zoom`; `handle(Input::Wheel { .. })`; `x.write().set_domain(..)` on the
      plot's x handle; and `c.write()` on a colour `ScaleRef` that appears only
      in a layer's `FILL` encoding.
- [ ] A failed resolve (a layer with z 150) clears the cache: fixing the z and
      resolving again succeeds and increments the generation.

### AC4: Wheel input

- [ ] After one resolve, `handle(Input::Wheel { at, lines: 2.0 })` with `at` the
      plot rect's centre returns `redraw: true`. After the next resolve the x
      and y domains are each `0.85²` of their old width (relative error ≤ 1e-9),
      and the data value under `at` (the scale's `invert`, through the handle
      returned by `plot.x(..)`) is unchanged to 1e-9 relative.
- [ ] `lines` of `0.0`, `NaN` or `±∞`, or any wheel before the first resolve,
      returns `redraw: false` and changes no domain.
- [ ] `PointerMoved`, `PointerLeft`, `PointerDown` and `PointerUp` return
      `redraw: false` (S8c gives them behaviour).
- [ ] `show` sends wheel events through `Plot::handle`. `show.rs` no longer
      calls `Plot::zoom`.

### AC5: Nothing regresses

- [ ] `zoom_uploads` passes unchanged. `mask perf-budget` passes, or fails only
      on timing and passes on a second run.
- [ ] `mask wasm-browser` passes. `mask wasm-size` passes; re-record
      `wasm.scatter.gz_bytes` in `crates/gup-core/PERF_BUDGETS.md` if it moved.

## Decisions

New file `crates/gup-core/src/input.rs`:

```rust
/// Pointer and wheel input in chart coordinates: logical pixels from the
/// chart's top-left corner, y down (the coordinates of `Layout`).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Input {
    PointerMoved { at: Point },
    PointerLeft,
    PointerDown { at: Point, button: Button },
    PointerUp { at: Point, button: Button },
    /// `lines > 0` scrolls up (away from the user), which zooms in.
    Wheel { at: Point, lines: f32 },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Button { Primary, Secondary, Middle }

/// What the host should do after `Chart::handle`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// The chart looks different: resolve and render again.
    pub redraw: bool,
}
```

`crates/gup-core/src/chart.rs`:

```rust
pub trait Chart: 'static {
    /// Resolve for a `width × height` logical-pixel chart. Cached: returns the
    /// previous result, doing no work and no GPU writes, when nothing changed.
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<&Resolved>;
    /// The layout of the last successful resolve.
    fn layout(&self) -> Option<&Layout>;
    /// React to input; never fails.
    fn handle(&mut self, input: Input) -> Response;
    /// The chart's title, if any (a window title).
    fn title_text(&self) -> Option<&str>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

- `ChartExt` keeps its methods and signatures. `render_resolved` clones the
  layout. `Resolved` derives `Clone` so tests can keep a result across a later
  `&mut` call.
- **Cache key**: the `ContextId`, `width.to_bits()`, `height.to_bits()` and a
  revision sum. The sum is `Plot.revision` plus the revision of the x and y
  scales plus the revision of every link of every column encoding of every
  layer. All counters only increase, so any change raises the sum. Compute the
  sum at the start of `resolve`. Store it after a successful full resolve,
  because resolve itself writes the scales.
- **`Plot.revision: u64`**: `wrapping_add(1)` in every `&mut self` method except
  `resolve` and `handle`: `title`, `x_title`, `y_title`, `legends`, `theme`,
  `x`, `y`, `add`, `layer_mut` and `zoom`. Bumping in `add` and `layer_mut`
  covers every change made to a layer, because a `Selection` is reachable only
  through them.
- **`ScaleRef` revision**: `Shared` gains `revision: AtomicU64`.
  `ScaleRef::write` does `fetch_add(1, Relaxed)` before returning the guard;
  `read` does not. `ShaderFn` gains
  `#[doc(hidden)] fn revision(&self) -> u64 { 0 }`, which `ScaleRef` overrides,
  and `DynShaderFn` gains a forwarding `revision`. `DynPositionScale` gains
  `fn revision(&self) -> u64`. `Layer` gains `fn revision(&self) -> u64`, the
  wrapping sum over its links.
- **Hit path**: call `cx.take_gpu_errors()?` (a browser reports errors late),
  then return the cached `&Resolved`. A full resolve that fails sets the cache
  to `None`.
- `#[doc(hidden)] pub fn resolve_generation(&self) -> u64` on `Plot` counts full
  resolves. Tests use it, and S8b uses it to invalidate its pick index.
- **Wheel**: `Plot::handle(Input::Wheel { at, lines })` calls
  `self.zoom(at, 0.85f64.powf(lines as f64))` and returns `redraw: true` when
  `lines` is finite and non-zero and a resolve has happened. Otherwise it
  returns the default `Response`. It zooms anywhere on the canvas, as `show`
  does today. `ZOOM_PER_LINE` moves from `show.rs` into `plot.rs`.
- `show` keeps its signature (`show(plot: Plot)`); S8e generalises it. Its wheel
  arm builds `Input::Wheel` (`PixelDelta` as `y / 40`, as today) and calls
  `request_redraw` when `redraw` is set.
- `Derived`'s documentation changes from "re-evaluated each resolve" to
  "re-evaluated each resolve that is not cached. A `Derived` reading state other
  than the plot's scales and layers should be paired with a change that
  invalidates the cache".
- `Input`, `Button` and `Response` are re-exported at the crate root, not in the
  prelude (hosts need them, chart authors do not).

## Escalation Triggers

- Two consecutive uncached resolves of an unchanged plot give different
  `Layout`s or scenes, so `resolve` is not idempotent and caching would change
  output.
- A caller cannot be migrated with a `Resolved::clone()` or by narrowing a
  borrow.
- An upload count in `PERF_BUDGETS.md` changes.
- A trybuild `.stderr` snapshot changes other than in line numbers.

## Out of Scope

- CPU picking: GUP-445 (S8b).
- Hover, click, `Selection::on` and tooltips: GUP-446 (S8c).
- `GupApp` and `show` on any `Chart`: GUP-448 (S8e).
- Drag pan, brush, zoom limits or zoom configuration: RFC-001 S9.

## Technical Tasks

- [ ] Add `input.rs` and re-export it.
- [ ] Add the revision counters (`ScaleRef`, `ShaderFn`/`DynShaderFn`,
      `DynPositionScale`, `Layer`, `Plot`).
- [ ] Split `Plot::resolve` into a cache check and the existing body
      (`resolve_full`), store `cache: Option<(CacheKey, Resolved)>`, and add
      `layout`, `handle` and `resolve_generation`.
- [ ] Change `Chart` and `ChartExt`; derive `Clone` on `Resolved`.
- [ ] Migrate every `.resolve(` caller (about 85: tests, examples, `show.rs`,
      `order.rs`, `scope.rs`, `encoding.rs`, `marks.rs`, the wasm harness),
      letting the compiler list them.
- [ ] Route `show`'s wheel through `handle`.
- [ ] Write `tests/resolve_cache.rs` and the wheel tests.

## Dependencies

### Prerequisite Stories

- GUP-434: RFC-001 S7e `Chart` and `ChartExt` ✅ — the trait this reshapes.

### Enables Stories

- GUP-445 (S8b CPU picking) — invalidates its index by `resolve_generation`.
- GUP-446 (S8c hover, click, tooltips) — builds on `handle` and the cache.
- GUP-448 (S8e `GupApp`), GUP-449 (S8f web canvas) — hosts that call `resolve`
  every frame and route input through `handle`.

## Testing Strategy

- **Integration tests**: `resolve_cache.rs` (AC2, AC3) and the wheel sequence
  (AC4), headless through `Context::new_blocking`.
- **Visual validation**: none new. Existing goldens must pass unchanged.
- **Performance**: `zoom_uploads`, `mask perf-budget`.
- **Browser**: `mask wasm-browser`, `mask wasm-size`.

## Success Metrics

- [ ] A hit resolve writes 0 bytes of every `Upload` kind.
- [ ] All goldens and `zoom_uploads` pass unchanged.

## Risk Assessment

- **Medium**: something reads mutable state that the revision sum does not see
  (a `Derived` reading an unrelated handle), and a cached frame goes stale.
  _Mitigation_: it is documented on `Derived`. AC3 covers every mutation path
  the plot owns.
- **Low**: migrating 85 call sites is noisy. _Mitigation_: it is mechanical, and
  the compiler lists every site.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-10-17)

**Question**: AC4 requires that after a wheel zoom and the next resolve the data
value under `at` is unchanged (1e-9). With automatic domains it moves. How
should that be met?

**Evidence**: `tests/resolve_cache.rs::wheel_zooms_about_the_pointer` (the
`#[cfg(any())]`-gated assertion). On a plain linear scatter, the x value under
the plot centre is 25 before the wheel and 25.5955 after the next resolve; y is
8 then 8.3317. The domain widths are right (0.85² of old). Cause, pre-existing
and independent of caching: `Plot::resolve` widens an _automatic_ axis in data
units to cover the mark overhang and gives it the whole plot rect as its range
(`widen`, `set_ranges`, plot.rs about line 522). `zoom` makes the domain
explicit, so the next resolve insets the far end of the range by
`mark_overhang + plot_padding`, which shifts the mapping. The existing
`zoom_narrows_both_domains_about_the_anchor` test only checks the anchor before
re-resolving, so it never saw this. The first wheel step after load therefore
jumps by the inset.

**Options**:

1. Fix in resolve: an axis that was widened keeps its full-rect range once its
   domain is explicit (or `zoom` converts the domain into the inset-range
   frame). Changes `resolve`'s range logic beyond Decisions; may affect goldens
   and zoom uploads.
2. Narrow AC4's test to explicit domains. Hides the jump.
3. Accept the jump, relax AC4 to widths only, and file a follow-up.

**Recommendation**: option 1 as its own small change (or a follow-up story
before S8c), since tooltips and S9 pan will hit it too.

**State**: everything else is implemented and compiles
(`cargo check --workspace --all-targets`, the wasm harness). Remaining: AC4
resolution, then the full test run, lint, `old-path-loc`, `wasm-size`,
`wasm-browser`, `perf-budget`, and the retrospective.
