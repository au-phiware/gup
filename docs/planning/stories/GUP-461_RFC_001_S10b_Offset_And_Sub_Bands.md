---
implementer: sonnet
implementer-reason:
  a two-line WGSL scale following `scale_pow`, plus two helpers on
  `ScaleRef<Band>`; formulas, names and module path are fixed in Decisions
---

# GUP-461: RFC-001 Step S10b: An `Offset` Link and Sub-Bands for Grouped Bars

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-18

## Context

Dogfood task 2 needs grouped bars (4 quarters × 4 regions), and RFC-001 S10's
bar builder (GUP-464) must draw them. Today a `Rect` bar's x goes through a
`Band` scale (`x.encode_key(..)`, centre of the band) with width
`x.band_width_px()`. A grouped bar needs the same centre moved by a pixel offset
that depends on the bar's group and on the band's current width. The width
changes with the plot rect and the key count, so the offset cannot be a fixed
number.

`then` chains (S5a) already let a `Band`'s pixel output feed an `f32` link, and
every link's `Params` are written at each prepare, alongside `Derived` constants
(`selection.rs` `prepare`). So a link that adds a `Derived<Px>` value to its
input gives a grouped bar's x:
`x.then(Offset::by(derived)).encode_key(|r| r.quarter)`. The chain's first link
is the plot's own `ScaleRef<Band>`, so the dictionary, the axis ticks and the
fitted keys are shared as they are for any band encoding (GUP-433).

## Read First

- RFC-001 "S5a findings" → "`then`: chains in the glue" (lines 2505–2541).
- `crates/gup-core/src/scale/pow.rs` (whole): a library scale with `Params`,
  `ShaderFn`, `CpuMirror` and `image`.
- `crates/gup-wgsl/library/scale_pow.wgsl` and how `gup-wgsl`'s build registers
  library modules (grep `scale_pow` in `crates/gup-wgsl` and
  `crates/gup-core/src/shader`).
- `crates/gup-core/src/scale/band.rs` lines 130–240 (`Band`, `band_width`,
  `ScaleRef<Band>::band_width_px`).
- `crates/gup-core/src/scale/conformance.rs`
  `band_and_point_gpu_match_cpu_mirror` and
  `a_three_link_chain_matches_its_mirrors_in_order` (lines 470–640).
- `crates/gup-core/tests/rect_png.rs` (a bar golden to copy).

## User Story

> "As a developer drawing grouped bars, I want each group's bars to sit side by
> side inside their band, so that I can compare groups within a category."

## Acceptance Criteria

### AC1: `Offset`

- [ ] `Offset` exists as in Decisions, in `crates/gup-core/src/scale/offset.rs`,
      exported at the crate root and in the prelude, with a doctest.
- [ ] Conformance: `Linear.then(Offset::by(Derived::new(|| Px(12.5))))` and
      `Band.then(Offset::by(..))` match their CPU mirrors within 1e-4 px over
      the conformance inputs (new tests beside the chain test).
- [ ] An `Offset` whose `Derived` changes between two resolves moves the marks
      with 0 column bytes written and only uniform writes (count them).

### AC2: Sub-bands

- [ ] `ScaleRef<Band>::sub_band` and `sub_band_width_px` exist as in Decisions,
      with doctests. Unit tests check the centres and widths against the
      formulas for `count` 1, 2 and 4, on a normal and a reversed range.
- [ ] `sub_band(0, 1)` places bars exactly where `encode_key` on the band does
      (byte-identical render).

### AC3: Grouped bars, by hand

- [ ] A golden, `gup_core/grouped_bars`: 3 quarters × 2 groups, two `Rect`
      layers built with `Plot` (no builder). Each layer's x is
      `x.sub_band(i, 2).encode_key(..)`, its width is
      `x.sub_band_width_px(2, 0.1)`, and its fill comes from one shared
      `ScaleRef<Categorical>` through `encode_key`. The implementer reads it:
  - two bars side by side in each band, with a visible gap between them;
  - the x tick labels under each band's centre;
  - one legend with both groups.
- [ ] Pixel check: along a row through all bars, `dogfood/src/pixels.rs`'s
      `count_runs` rule (non-background runs ≥ 3 px) counts exactly 6 runs.
      Reimplement it in the test; don't depend on the dogfood crate.
- [ ] `pick_row` at each bar's centre returns that bar's row, and `pick_row_gpu`
      agrees.

## Decisions

- **`Offset`**:

  ```rust
  /// Adds a pixel offset, read each time the plot resolves, to its input.
  /// For chains: `band.then(Offset::by(dx))`.
  #[derive(Clone)]
  pub struct Offset(Derived<Px>);
  impl Offset {
      pub fn by(offset: Derived<Px>) -> Self;
      /// A fixed offset.
      pub fn px(offset: f32) -> Self;
  }
  // ShaderFn: In = f32, Out = Px, MODULE "gup::scale::offset", ENTRY "map",
  // Params { offset: f32 } from the Derived at params() time.
  // CpuMirror: eval(v) = v + offset. image((a, b)) = (a + o, b + o).
  // fit_domain: none (no domain).
  ```

  WGSL, in `crates/gup-wgsl/library/scale_offset.wgsl`:

  ```wgsl
  struct Params { offset: f32 }
  fn map(v: f32, p: Params) -> f32 { return v + p.offset; }
  ```

- **Sub-bands**, on `impl ScaleRef<Band>`:

  ```rust
  /// The centre of slot `index` of `count` equal slots across each band.
  pub fn sub_band(&self, index: u32, count: u32) -> Then<ScaleRef<Band>, Offset>;
  /// A bar's width in such a slot: the slot's width less `padding` (0..1) of it.
  pub fn sub_band_width_px(&self, count: u32, padding: f64) -> Derived<Px>;
  ```

  With `w = band_width()` (signed, as `band_width` is), the offset is
  `((index + 0.5) / count − 0.5) · w`, and the width is
  `|w| / count · (1 − padding)`. Both read the band each resolve. Panics if
  `index >= count` or `count == 0` (documented under `# Panics`).

- **The padding for bars** is the caller's. GUP-464 passes 0.1, which leaves a
  gap of about 3 px at 800 px wide with 4 bands of 4.

## Escalation Triggers

- `then` cannot follow a `ScaleRef<Band>` (key input) with an `f32` link, or the
  chain does not share the band's dictionary, so keys get different codes from
  the axis.
- The `Band` axis shows no tick labels when every layer encodes x through a
  sub-band.
- `Offset`'s params are not re-read on a resolve that only changes the plot
  rect, so the bars lag a resize.

## Out of Scope

- The bar builder and stacking: GUP-464.
- Horizontal grouped bars (a `Band` on y).

## Technical Tasks

- [ ] `scale_offset.wgsl` registered in `gup-wgsl`'s library; `offset.rs`.
- [ ] `sub_band`, `sub_band_width_px` and unit tests.
- [ ] Conformance tests (AC1); golden and pick tests (AC3).
- [ ] `mask wasm-size`: no change expected (the harness doesn't use it).

## Dependencies

### Prerequisite Stories

- GUP-418: RFC-001 S5a ShaderFn v2 ✅ — `then` chains.
- GUP-419: RFC-001 S5b Band and Point ✅.
- GUP-429: RFC-001 S6c Rect Mark and `Derived` ✅.
- GUP-433: RFC-001 S7d Shared Key Dictionaries ✅.

### Enables Stories

- GUP-464: `gup_core::bar(..).group(..)`.

## Testing Strategy

- **Conformance**: GPU equals the CPU mirror for `Offset` chains.
- **Unit**: sub-band formulas on normal and reversed ranges.
- **Visual**: `gup_core/grouped_bars.png` plus a run count.

## Success Metrics

- [ ] Grouped bars render correctly from public API, without a builder.

## Risk Assessment

- **Low**: `Derived` params in a chain are new (`Derived` was a channel value
  only). _Mitigation_: AC1's resolve-to-resolve test, and an escalation trigger.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: `gup_core/grouped_bars.png`
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
