---
implementer: sonnet
implementer-reason:
  trait signatures settled in Decisions (a reduced RFC-001 §8 shape); Plot
  implements it by moving existing methods
---

# GUP-434: RFC-001 Step S7e: `Chart` and `ChartExt` Traits

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-17)
**Created**: 2026-10-16

## Context

RFC-001 §8 specifies an object-safe `Chart` trait and a `ChartExt` blanket
extension so that builders, multi-panel layouts and the egui widget can hold
`Box<dyn Chart>`. Today `Plot` has inherent `resolve`, `render`,
`render_resolved` and `save_png` (`crates/gup-core/src/plot.rs` lines 197 and
405–436) and nothing else implements a chart interface. This was split out of
[GUP-424](GUP-424_RFC_001_S7a_Multi_Layer_Plot_And_Shared_Scales.md) on
2026-10-16.

RFC-001 §8's sketch also has `handle(&Input, &mut InputCx)` and a cached
`resolve(&mut ResolveCx) -> Result<&Resolved>`. Neither has a consumer yet:
input dispatch is RFC-001 S8, and caching needs dirty tracking across `ScaleRef`
handles mutated outside the plot, while `resolve` already reuses GPU state and
writes only uniforms when nothing structural changed. This story therefore
defines the trait with today's `resolve` signature and leaves `handle` and
caching to S8, where their consumer arrives. Changing the trait then is cheap:
gup-core is unpublished and `Plot` is its only implementor.

## Read First

- RFC-001 §8 Design (lines 578–617).
- `crates/gup-core/src/plot.rs` lines 74–82 (`Resolved`), 113–176 (`Plot`,
  `add`), 197–200 (`resolve` signature) and 403–447 (`render`,
  `render_resolved`, `save_png`, `glue_sources`).
- `crates/gup-core/src/lib.rs` lines 94–120 (re-exports) and 250–257 (prelude).

## User Story

> "As a visualization developer, I want to hold any chart as a `Box<dyn Chart>`
> and render or save it through one interface, so that code that displays charts
> does not depend on the concrete chart type."

## Acceptance Criteria

### AC1: Traits

- [x] `gup_core::{Chart, ChartExt}` exist with the signatures in Decisions, both
      in the prelude; `Plot` implements `Chart`.
- [x] A doctest on `Chart` builds a `Plot`, boxes it as `Box<dyn Chart>`,
      renders it with `ChartExt::render`, and downcasts back with
      `as_any_mut().downcast_mut::<Plot>()`.

### AC2: `Plot::layer_mut`

- [x] `Plot::layer_mut::<T, M>()` returns the first layer of type
      `Selection<T, M>`; a test with two layers of different row types gets each
      and changes an attribute.

### AC3: Callers migrate

- [x] `Plot`'s inherent `render`, `render_resolved` and `save_png` are removed;
      every caller (tests, doctests, examples, `show.rs`) compiles through
      `ChartExt`. `cargo check --examples` and `cargo test -p gup-core --doc`
      pass.

## Decisions

```rust
pub trait Chart: 'static {
    /// Resolve domains, layout and GPU state for a `width × height`
    /// logical-pixel chart.
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<Resolved>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait ChartExt: Chart {
    #[cfg(not(target_arch = "wasm32"))]
    fn render(&mut self, cx: &Context, width: u32, height: u32) -> Result<image::RgbaImage>;
    #[cfg(not(target_arch = "wasm32"))]
    fn render_resolved(&mut self, cx: &Context, width: u32, height: u32)
        -> Result<(image::RgbaImage, Layout)>;
    #[cfg(not(target_arch = "wasm32"))]
    fn save_png(&mut self, cx: &Context, path: impl AsRef<Path>, width: u32, height: u32)
        -> Result<()>;
}
impl<C: Chart + ?Sized> ChartExt for C { /* the bodies moved from Plot */ }
```

- Lives in a new `crates/gup-core/src/chart.rs`.
- No `Send`/`Sync` supertrait: `Plot` is not `Send` on wasm32 (wgpu handles);
  callers needing `Send` write `Box<dyn Chart + Send>`.
- `Plot::resolve` stays inherent too (same signature) and the trait impl
  forwards to it.
- `layer_mut<T: Send + Sync + 'static, M: Mark>(&mut self) -> Option<&mut Selection<T, M>>`
  is inherent on `Plot`, using `Layer::as_any_mut`.
- `show(plot: Plot)` keeps taking `Plot` (it needs `zoom`, which is not on
  `Chart` until S8's `handle`).

## Escalation Triggers

- A caller outside gup-core (another workspace crate) uses `Plot::render` and
  cannot import `ChartExt`.
- `ChartExt`'s generic `save_png` makes `dyn Chart` unusable with it.
- A trybuild `.stderr` snapshot (`tests/compile_fail/`) changes other than in
  line numbers.

## Out of Scope

- `handle`, `Input`, `ResolveCx`, resolve caching, `save_svg`, async `to_rgba`:
  RFC-001 S8 (not yet a story).
- Builders over `Plot`: RFC-001 S10.

## Technical Tasks

- [x] Add `chart.rs`; implement `Chart` for `Plot`; move the render methods.
- [x] Add `Plot::layer_mut`; export and prelude.
- [x] Migrate callers; doctest and tests for AC1–AC2.

## Dependencies

### Prerequisite Stories

- GUP-424: Multi-Layer Plot 📋 — `layer_mut` is meaningful with several layers.

## Testing Strategy

- **Unit tests**: `layer_mut` type matching.
- **Integration tests**: doctest through `Box<dyn Chart>`.
- **Visual validation**: none; existing goldens must be unchanged.
- **Performance**: none.

## Success Metrics

- [x] Rendering code can be written against `dyn Chart`.

## Risk Assessment

- **Low**: wide but mechanical caller migration. _Mitigation_: `ChartExt` in the
  prelude covers most callers.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/chart.rs`: `Chart` (resolve, `as_any`, `as_any_mut`) and
  `ChartExt` (blanket `render`, `render_resolved`, `save_png`), both in the
  crate root and prelude; `Plot` implements `Chart` by forwarding to its
  inherent `resolve`.
- `Plot::layer_mut::<T, M>()`; the inherent `render`, `render_resolved` and
  `save_png` are removed. Callers needed only a `ChartExt` import in
  `window_parity.rs` and the plot unit tests.
- Tests: `layer_mut_matches_layers_by_type` and the `Chart` doctest (boxes a
  `Plot`, renders, downcasts). gup-core: all tests and 20 doctests pass; goldens
  unchanged; `mask old-path-loc` 28,910.

## Retrospective

**Completed**: 2026-10-17

### Key Technical Learnings

#### Blanket extension traits

- **Challenge**: Moving inherent methods behind a trait.
- **Solution**: Default method bodies on `ChartExt` plus
  `impl<C: Chart + ?Sized> ChartExt for C {}` works for `dyn Chart`, and a
  generic `save_png` is fine because `ChartExt` is not used as a trait object.
- **Pattern**: Callers in the prelude need no changes; only files importing
  `gup_core::Plot` without the prelude do.

### Architectural Decisions

#### Reduced RFC-001 §8 trait

- **Decision**: `resolve` returns an owned `Resolved`; `handle` and caching are
  left to S8.
- **Trade-off**: One more allocation per resolve than a cached `&Resolved`.

### Development Workflow Insights

- A repo-wide grep for `.render(` is far too noisy; compile errors list the
  callers exactly.
- `perf-budget` was not run because machine load stayed above 4 (4.85); no
  rendering path changed.

### Follow-up Stories

None.
