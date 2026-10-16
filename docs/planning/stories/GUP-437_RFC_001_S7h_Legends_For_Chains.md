---
implementer: sonnet
implementer-reason:
  the chain-legend rule (ticks in the data domain, positions through the first
  links' mirrors) and the user hook are settled in Decisions
---

# GUP-437: RFC-001 Step S7h: Legends for Chains and User Colour Functions

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-16

## Context

[GUP-426](GUP-426_RFC_001_S7c_Legend_Placement.md) places legends for channels
encoded through one built-in colour scale, found through a provided
`ShaderFn::legend()`. A chain such as `Linear.then(Pow::sqrt()).then(viridis)`
has no legend: its colour link's domain is the image of the data through the
earlier links, so the colour scale's own ramp ticks would be labelled in the
wrong units. RFC-001 "S5c findings → Proposed adjustments" records: "A chain has
no legend: the aftershock golden places a ramp by hand, and `Log.then(viridis)`
(S5a) could not have one. Legends for chains, and a `legend()` a user's colour
function can supply, belong with S7's legend guides." Split out of GUP-426 on
2026-10-16.

`Ramp` holds a LUT, stops in the colour scale's domain and ticks
(`RampTick { t, value, label }`, `scale/legend.rs` lines 60–80), with `t` the
position along the bar. A chain's legend can keep the last link's LUT and stops
and replace the ticks: nice values of the data extent, placed at the image of
each value through the earlier links. Every link before the colour has a CPU
mirror (`Then` requires `A: CpuMirror`, `encoding.rs` lines 400–430), so the
image is computable.

## Read First

- RFC-001 "S5c findings" → "Proposed adjustments to S6, S7 and later" (lines
  2993–3014).
- `crates/gup-core/src/scale/legend.rs` (all).
- `crates/gup-core/src/encoding.rs` lines 280–320 (`then`) and 395–440 (`Then`).
- `crates/gup-core/src/scale/mod.rs` lines 229–300 (`tick_step`,
  `format_with_step`).
- `crates/gup-core/tests/wgsl_function.rs` lines 180–225 (the aftershock
  golden's hand-placed ramp).
- GUP-426's `Layer::legends` and `guide.rs`.

## User Story

> "As a visualization developer, I want a legend for any colour encoding,
> including chains and my own colour functions, so that every coloured chart
> explains itself."

## Acceptance Criteria

### AC1: Chains

- [ ] `Then<A, B>::legend()` returns a ramp when `B`'s legend is a ramp: a unit
      test on `Linear.then(Pow::sqrt()).then(viridis)` over data 0..4e7 gives
      tick values that are 1-2-5 nice values in 0..4e7, labels formatted with
      `format_with_step`, and each `t` equal to the normalised position of the
      tick's image in the viridis stops, within 1e-9.
- [ ] A chain ending in a swatch legend (`Categorical`) returns `None`.

### AC2: User colour functions

- [ ] A `#[wgsl_function]` colour type (`Out = Color`) with `CpuMirror` and no
      `legend` gets a sampled ramp: 256 LUT entries from its mirror over its
      fitted extent, five nice ticks. A user who writes
      `fn legend(&self) ->     Option<Legend>` in the impl block overrides it
      (test both).

### AC3: The aftershock golden

- [ ] `wgsl_function_aftershocks` uses `Plot`'s automatic legend instead of its
      hand-placed ramp; re-blessed and read by eye: tick labels are in the
      data's units.

## Decisions

- **Hook**: `EncodeFn` gains `fn legend(&self) -> Option<Legend>`. The blanket
  impl for `S: ShaderFn` forwards to `ShaderFn::legend`. `Then<A, B>` takes
  `B`'s legend; for `Legend::Ramp`, it replaces the ticks as in AC1 using the
  data extent the encoding was fitted to (stored by `Then::fit`), and
  `t = (image - stop_lo) / (stop_hi - stop_lo)` piecewise over the three stops.
  `Layer::legends` (GUP-426) asks the encoding, not the single link.
- **Sampled default**: `#[wgsl_function]` emits, for `Out = Color` types with a
  mirror, a default `legend` that builds a `Ramp` with a new
  `pub fn Ramp::sampled(domain: (f64, f64), f: impl Fn(f64) -> Color, ticks: usize) -> Ramp`;
  a user-written `legend` in the impl wins.
- **Ticks**: `tick_step(lo, hi, 5)` over the data extent, labels via
  `format_with_step`.

## Escalation Triggers

- `Then::fit` cannot keep the data extent without changing `EncodeFn`'s public
  signatures beyond adding `legend`.
- A trybuild snapshot under `tests/wgsl_function_fail/` changes.

## Out of Scope

- Swatch legends for chains into categorical scales: not planned.
- Legends for size or opacity: not yet a story.

## Technical Tasks

- [ ] `EncodeFn::legend`, `Then`'s ramp rewrite, `Ramp::sampled`.
- [ ] `#[wgsl_function]`'s default `legend`.
- [ ] Tests for AC1–AC2; migrate the aftershock golden.

## Dependencies

### Prerequisite Stories

- GUP-426: Legend Placement 📋 — the hook and placement this extends.
- GUP-420: `#[wgsl_function]` v2 ✅.

## Testing Strategy

- **Unit tests**: AC1 tick values and positions; `Ramp::sampled`.
- **Integration tests**: user-function legend default and override.
- **Visual validation**: re-blessed `wgsl_function_aftershocks`.
- **Performance**: none.

## Success Metrics

- [ ] No gup-core golden places a legend by hand.

## Risk Assessment

- **Low**: a non-monotonic user mirror makes ticks out of order on the bar.
  _Mitigation_: ticks keep data order; document that the bar shows the mirror's
  colours, which stay correct.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
