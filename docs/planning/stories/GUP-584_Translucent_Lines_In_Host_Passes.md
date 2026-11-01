---
implementer: sonnet
implementer-reason:
  one public method over GUP-439's crate-private encode, a guard so draw stays
  correct without it, and a zoom_bench flag; the design is settled below
touches:
  [
    crates/gup-core/src/render.rs,
    crates/gup-core/examples/zoom_bench.rs,
    crates/gup-core/tests/targets.rs,
  ]
---

# GUP-584: Translucent Lines in Host-Owned Passes

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 💡
New **Created**: 2026-11-02

## Context

GUP-439 draws a translucent `Line` layer once into an offscreen target (`Max`
blending) and composites it in the scene pass, so its joins and self-crossings
have the arms' opacity. That pass must be recorded before the scene pass.
`Renderer::render` does it for every Gup-owned target (image, texture, window,
web), and `gup-egui` and `gup-bevy` render through `TextureTarget`, so they have
it.

A host that draws in its own pass (`Renderer::prepare` + `Prepared::draw`)
cannot: `prepare` builds no offscreen targets and `draw` records into a pass
that has already begun, so its translucent lines still blend twice at every
join. `zoom_bench` is such a host, so it cannot time a translucent line either:
GUP-439's AC3 numbers came from a local prototype patch (RFC-001 "GUP-439
findings", "Cost").

The escalation trigger in GUP-439 asked for the smallest API change to be
reported before it was made. This story is that report.

## User Story

> "As a host that draws Gup charts inside my own render pass, I want translucent
> lines to look as they do in Gup's own targets, by recording one extra call
> before my pass."

## Acceptance Criteria

### AC1: One call before the host's pass

- [ ] `Prepared::encode(&self, encoder: &mut wgpu::CommandEncoder)` is public:
      it records each translucent line layer's offscreen pass, and records
      nothing for a scene without one. `Renderer::prepare` builds the offscreen
      targets (as `Renderer::render` does now).
- [ ] A host that calls `encode` before its pass gets GUP-439's result: a test
      in `tests/targets.rs` draws a translucent bow tie through
      `prepare`/`encode`/`draw` into its own pass and matches
      `Renderer::render`'s image byte for byte.

### AC2: Correct without the call

- [ ] A host that never calls `encode` still draws translucent lines (directly,
      blending twice at overlaps, as today), never an uncleared texture: `draw`
      composites only if `encode` was recorded for this `Prepared` (for example
      an `AtomicBool` set by `encode`), else draws the layer directly with the
      mark pipeline that `prepare` also fetched.
- [ ] `prepare` and `draw` still never submit (the existing `targets.rs` check).

### AC3: Timed in zoom_bench

- [ ] `zoom_bench --line linear --translucent` draws the random walk at alpha
      0.5, calls `encode`, and times from the start of the first offscreen pass
      to the end of the scene pass (the offscreen pass writes the beginning
      timestamp, the scene pass only the end). The RFC-001 "GUP-439 findings"
      cost table is re-measured with it, replacing the prototype's note.

## Decisions

- The API is `Prepared::encode(&self, &mut wgpu::CommandEncoder)`; no change to
  `TargetDesc`, `Renderer::prepare`'s signature or `Prepared::draw`.
- Timestamps for `zoom_bench` go through a crate-public variant or an optional
  argument only if a public one is unavoidable; prefer
  `Prepared::encode_with_timestamp(&self, encoder, Option<(&QuerySet, u32)>)`
  documented as a benchmarking aid. Report before adding it if it can be done
  another way with less surface.
- The pooled offscreen textures stay in the `Renderer`.

## Escalation Triggers

- A host's draw would need a change to `Prepared::draw`'s signature.
- Byte identity with `Renderer::render` (AC1) fails for a reason other than the
  pass's clear colour.

## Out of Scope

- Sharing one texture between several translucent layers.
- Translucent marks other than `Line`.

## Technical Tasks

- [ ] Make `encode` public with docs and an example in `Prepared`'s docs.
- [ ] `prepare` builds offscreen targets; `draw` falls back to the direct draw
      when `encode` was not recorded.
- [ ] `zoom_bench --translucent`; re-measure and update RFC-001.
- [ ] Update `Renderer::prepare` and `Line` docs (they say host passes blend
      twice).

GUP-439's prototype (local, uncommitted) added
`Prepared::encode_timed(encoder, Option<(&QuerySet, u32)>) -> bool` (whether it
wrote the beginning timestamp), `Renderer::prepare_offscreen`, a `--translucent`
flag on `zoom_bench` and a `GUP_DIRECT` switch for the direct draw; about 50
lines.

## Dependencies

### Prerequisite Stories

- GUP-439: Translucent lines without double-blended joins ✅.

## Testing Strategy

- **Integration tests**: `tests/targets.rs` (host pass with and without
  `encode`; no submit).
- **Performance**: `zoom_bench --line linear --translucent`, alternating with
  the direct draw.

## Success Metrics

- [ ] A host's translucent lines match `Renderer::render`'s byte for byte.

## Risk Assessment

- **Low**: an additive method. _Mitigation_: `draw` stays correct without it.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
