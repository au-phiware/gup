---
implementer: opus
implementer-reason:
  a rendering-semantics and GPU design choice with real trade-offs (passes,
  memory, WebGPU limits); the implementer must decide and record it
touches:
  [
    crates/gup-core/src/render.rs,
    crates/gup-core/src/shaders/,
    crates/gup-core/src/selection.rs,
    crates/gup-core/tests/line_translucent.rs,
    tests/golden/gup_core/line_translucent.png,
    crates/gup-core/wasm-size/,
    scripts/wasm_browser.sh,
  ]
---

# GUP-585: Translucent Lines Blend Once per Series, Not per Layer

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 💡
New **Created**: 2026-11-02

## Context

GUP-439 draws a translucent `Line` layer into an offscreen `Rgba8Unorm` target
with Max blend and composites it once, so a line's joins and self-crossings are
blended once (its acceptance criteria). But the unit is the whole layer: in
`tests/golden/gup_core/line_translucent.png` twelve series in one layer cross
many times and the crossings are no darker than a single line. Translucency in a
many-series line chart is usually there to show where series pile up; per layer,
that cue is gone, and the result also differs from SVG, which GUP-439 writes as
one `<g opacity>` per layer (same semantics, so parity holds, but the semantics
are the question).

GUP-439's retrospective and RFC-001 "GUP-439 findings" record the design (Max
blend, per-layer pass, cost 1.2x, 4 B/px per translucent layer) and the rejected
stencil approach.

Also: the composite shader has only been parsed by naga; no browser test draws a
translucent line, so it has never been compiled by a WebGPU implementation.

## User Story

> "As an analyst drawing 50 translucent series, I want the places where many
> series overlap to look denser, while each single series still has even opacity
> along its own joins and self-crossings."

## Acceptance Criteria

- [ ] **AC1, decide and record**: choose the semantics and mechanism and write
      them into this story's Decisions before implementing. Candidates: (a) per
      series: each series blended once, series composited over each other with
      normal alpha (mechanism options: one offscreen per series with
      clear+composite per series, series id in a second channel with Max only
      within the same id, or a sorted per-series draw that resets coverage); (b)
      keep per layer as the default and add an opt-in
      `.blend(Blend::PerSeries)`; (c) keep per layer and document the cost of a
      layer per series. State the cost per option at 12 and 200 series (passes,
      memory, GPU ms on the GUP-439 benchmark).
- [ ] **AC2**: the chosen behaviour has a pixel test: two crossing series in one
      layer at alpha 0.5 show combined coverage 0.75 (±2/255) at their crossing
      if per series is chosen; each series' own self-crossing still shows 0.5.
      Seeded failure recorded.
- [ ] **AC3**: SVG output matches the chosen semantics (per-series groups if per
      series), with the SVG parity check passing.
- [ ] **AC4**: browser coverage: `scripts/wasm_browser.sh`'s page (or a second
      page) draws one translucent line layer, and the smoke test asserts it
      compiled and drew (CPU counter or `render_rgba` readback, as the existing
      steps do). This closes GUP-439's untested WebGPU path whatever AC1
      decides.
- [ ] **AC5**: opaque lines stay byte-identical and cost nothing extra;
      `scripts/perf_budget.sh --skip-wasm` passes; WASM growth at most +2,000 B
      gz on the scatter.

## Escalation Triggers

- The chosen mechanism needs a WebGPU feature beyond the current baseline.
- Per-series cost at 200 series exceeds 3x the per-layer pass.

## Out of Scope

- Translucent areas and other marks.
- `Prepared::encode` for host passes (GUP-584).

## Definition of Done

- [ ] ACs met; `mask merge-check --dogfood` passes; story ✅ in INDEX.md with
      Implementation Summary and Retrospective.
