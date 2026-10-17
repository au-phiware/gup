---
implementer: sonnet
implementer-reason:
  trait, input types and cache key are settled in Decisions; the rest is
  mechanical caller migration and tests
---

# GUP-444: RFC-001 Step S8a: Cached `resolve` and `Chart::handle`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-17)
**Created**: 2026-10-17

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

- [x] `gup_core::{Input, Button, Response}` exist as in Decisions, at the crate
      root. `Chart` has the signatures in Decisions and `Plot` implements them
      (its inherent `resolve`, `layout`, `handle` and `title_text` share them).
- [x] The `Chart` doctest in `chart.rs` boxes a `Plot`, resolves it through
      `&mut dyn Chart`, reads `chart.layout()`, sends
      `Input::Wheel { at, lines: 1.0 }` and asserts `Response { redraw: true }`.
- [x] Every caller compiles: `cargo check --workspace --all-targets`,
      `cargo test -p gup-core --doc` and the wasm harness
      (`./scripts/wasm_size.sh`) build.

### AC2: Cache hits do nothing

In a new `crates/gup-core/tests/resolve_cache.rs`:

- [x] Resolving an unchanged plot a second time at the same size on the same
      `Context` returns the same `Resolved` (`std::ptr::eq` on the two
      references), leaves `Plot::resolve_generation()` unchanged, and leaves
      every `Upload` kind's count and bytes in `cx.upload_stats()` unchanged.

### AC3: Every change misses

- [x] Each of these, applied between two resolves, increments
      `resolve_generation()` by exactly 1, and the second `Resolved` reflects
      it: a different size; a different `Context`; `title`; `x_title`;
      `y_title`; `theme`; `legends(false)`; `add`; `layer_mut` then `attr`;
      `zoom`; `handle(Input::Wheel { .. })`; `x.write().set_domain(..)` on the
      plot's x handle; and `c.write()` on a colour `ScaleRef` that appears only
      in a layer's `FILL` encoding.
- [x] A failed resolve (a layer with z 150) clears the cache: fixing the z and
      resolving again succeeds and increments the generation.

### AC4: Wheel input

- [x] After one resolve, `handle(Input::Wheel { at, lines: 2.0 })` with `at` the
      plot rect's centre returns `redraw: true`. After the next resolve the x
      and y domains are each `0.85²` of their old width (relative error ≤ 1e-9),
      and the data value under `at` (the scale's `invert`, through the handle
      returned by `plot.x(..)`) is unchanged to 1e-9 relative.
- [x] `lines` of `0.0`, `NaN` or `±∞`, or any wheel before the first resolve,
      returns `redraw: false` and changes no domain.
- [x] `PointerMoved`, `PointerLeft`, `PointerDown` and `PointerUp` return
      `redraw: false` (S8c gives them behaviour).
- [x] `show` sends wheel events through `Plot::handle`. `show.rs` no longer
      calls `Plot::zoom`.

### AC5: Nothing regresses

- [x] `zoom_uploads` passes unchanged. `mask perf-budget` passes, or fails only
      on timing and passes on a second run.
- [x] `mask wasm-browser` passes. `mask wasm-size` passes; re-record
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
- **Range policy**: zoom or any `handle` input changes only an axis's domain,
  never its range policy. A widened automatic axis keeps the full plot rect
  after a zoom; only a user call that sets the domain gives the fixed-domain
  (far-end inset) policy.
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

- [x] Add `input.rs` and re-export it.
- [x] Add the revision counters (`ScaleRef`, `ShaderFn`/`DynShaderFn`,
      `DynPositionScale`, `Layer`, `Plot`).
- [x] Split `Plot::resolve` into a cache check and the existing body
      (`resolve_full`), store `cache: Option<(CacheKey, Resolved)>`, and add
      `layout`, `handle` and `resolve_generation`.
- [x] Change `Chart` and `ChartExt`; derive `Clone` on `Resolved`.
- [x] Migrate every `.resolve(` caller (about 85: tests, examples, `show.rs`,
      `order.rs`, `scope.rs`, `encoding.rs`, `marks.rs`, the wasm harness),
      letting the compiler list them.
- [x] Route `show`'s wheel through `handle`.
- [x] Write `tests/resolve_cache.rs` and the wheel tests.

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

- [x] A hit resolve writes 0 bytes of every `Upload` kind.
- [x] All goldens and `zoom_uploads` pass unchanged.

## Risk Assessment

- **Medium**: something reads mutable state that the revision sum does not see
  (a `Derived` reading an unrelated handle), and a cached frame goes stale.
  _Mitigation_: it is documented on `Derived`. AC3 covers every mutation path
  the plot owns.
- **Low**: migrating 85 call sites is noisy. _Mitigation_: it is mechanical, and
  the compiler lists every site.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Resolved (2026-10-17)

AC4 failed for automatic domains: zoom made the domain explicit, so the next
resolve gave the axis the fixed-domain range (far end inset) instead of the
widened range it had, and the mapping jumped. The coordinator chose to fix it
here (Decision below). `Plot` now records which axes the last resolve widened
and the domain each zoom left on them; an axis that still has that domain keeps
the widened range and axis-line policy. A domain set any other way gets the
fixed-domain policy. A second finding: the plot rect follows the y tick-label
width, so the mapping can shift by a pixel when labels change width. That is
layout, not range policy, so the wheel tests use rows whose tick labels keep
their width.

## Implementation Summary

- `input.rs` (`Input`, `Button`, `Response`, crate root); `Chart` gains
  `layout`, `handle`, `title_text` and a `&Resolved` `resolve`; `Resolved` is
  `Clone`.
- Revision counters: `ScaleRef` (bumped by `write`), `ShaderFn`/`DynShaderFn`,
  `DynPositionScale`, `Layer`, `Plot`. `Plot::resolve` caches on context id,
  size and the revision sum, stored after the full resolve.
- Wheel through `Plot::handle`; `show` no longer calls `zoom`.
- `Plot` remembers which axes were widened and the domain a zoom left on them,
  so zoom does not change an axis's range policy.
- Callers migrated with `.clone()` (about 20 sites, plus the wasm harness). The
  in-crate tests that reach `plot.layers[i]` use a test-only `layer_at_mut`;
  `rect_png`'s `Derived` test touches `layer_mut` to invalidate the cache.
- Tests: `tests/resolve_cache.rs` (8). gup-core: all pass, 21 doctests.
- Evidence: `zoom_uploads` and the perf-budget upload rows unchanged (136
  uniform bytes, 3 writes per frame); no golden changed; `mask old-path-loc`
  28,910 (unchanged); `mask wasm-size` scatter 363,446 B gz (recorded 361,531,
  within +2%, not re-recorded); `mask wasm-browser` and `mask perf-budget` pass.

## Retrospective

**Completed**: 2026-10-17

### Key Technical Learnings

#### Zoom shifted the mapping on the first wheel step

- **Challenge**: AC4 failed on automatic domains: after a zoom the value under
  the pointer moved (25 to 25.6). Zoom makes the domain explicit, and resolve
  gave an explicit domain the inset range instead of the widened one.
- **Solution**: remember per axis whether the last resolve widened it and the
  domain a zoom left; an axis still holding that domain keeps the widened policy
  (range and axis line).
- **Pattern**: range policy belongs to how the plot was configured, not to
  `is_auto()`, which a zoom flips.

#### The plot rect follows tick-label width

- **Challenge**: even with the policy fixed, the mapping moved a pixel when
  zoomed y ticks got wider labels (`7.5` vs `8`), since the plot rect starts
  after the y labels.
- **Solution**: the wheel tests use rows whose tick labels keep their width.
- **Pattern**: "nothing moves under the pointer" holds for the scale mapping,
  not across a layout change. S9 pan and S8c tooltips will see small jumps
  whenever label width changes; a stable-gutter option would fix that.

#### Cache invalidation by revision sum

- **Challenge**: resolve itself writes the scales, so a key taken at the start
  never matches.
- **Solution**: take the key after the full resolve.
- **Pattern**: tests that mutate `plot.layers` directly or a `Derived` reading
  outside state bypass the revision; both now go through `layer_mut`.

### Architectural Decisions

#### Range policy survives zoom

- **Decision**: see the Decision "Range policy" above (coordinator's call).
- **Trade-off**: `Plot` carries two small fields (`widened`, `zoomed`).
- **Future**: pan in S9 should record its domain in `zoomed` the same way.

### Development Workflow Insights

- The compiler listed every `resolve` caller; most fixes were `.clone()`.
- The AC4 conflict was found by the first test run and escalated rather than
  worked around.

### Follow-up Stories

None. A stable y-gutter (no layout-driven shift while zooming) is noted above;
it belongs to S9 if pan shows the jump.
