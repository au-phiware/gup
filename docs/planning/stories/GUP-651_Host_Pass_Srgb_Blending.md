---
implementer: sonnet
implementer-reason: one documented caveat and one check, no shader change
touches: [crates/gup-core/src/render.rs, crates/gup-core/src/window.rs]
---

# GUP-651: Translucent Marks in a Host's sRGB Pass

## Story Overview

**Initiative**: Polish after 0.1 **Status**: ✅ Complete (2026-11-06)
**Created**: 2026-11-06

## Context

Found in GUP-616. `ImageTarget` (`Rgba8Unorm`), `TextureTarget` (draws through a
non-sRGB view, `target.rs` 472), `WindowTarget` (`surface.rs` picks a surface
format and draws through `remove_srgb_suffix()`, `window.rs` 19) and the web
canvas (same surface path) all blend translucent marks on the encoded sRGB
values, so they agree with each other and with the legend swatches (GUP-616
measured: 50% `#E69F00` on white is `[243, 207, 128]`).

The one exception is a host that calls `Prepared::draw` (`render.rs` 1922) into
its own pass: if that pass's colour view is an `*Srgb` format, the GPU blends in
linear light and a translucent mark is lighter than in PNG (the same 50% orange
gives green about 214 and blue about 188), and no longer matches its swatch.
Opaque marks are identical.

## Acceptance Criteria

- [x] `Prepared::draw` documents that the pass's colour view must be non-sRGB
      for translucent marks to match Gup's own targets, with the `view_formats`
      recipe `TextureTarget` documents.
- [x] `Prepared::draw` (or `Renderer::prepare`) returns a configuration error
      when given a pass whose format is `*Srgb`, if the format is known at
      prepare time; otherwise a test renders a translucent area into an
      `Rgba8UnormSrgb` view and asserts the documented difference.

## Decision

Do not convert in the shader: the sRGB-encoded convention is Gup's (LUTs and
colours are encoded values), and a per-target shader switch would fork every
mark. Keep non-sRGB views everywhere and make the host contract explicit.

## Out of Scope

Changing target formats; SVG (viewers blend in sRGB already).

## Definition of Done

- [x] Tests pass, `mask all-fix` clean, retrospective added.

## Implementation Summary

- `Prepared::draw` documents the non-sRGB contract with the `view_formats`
  recipe `TextureTarget` uses, and the measured difference (50% `#E69F00` on
  white: green 214 instead of 207).
- `Renderer::prepare` returns a configuration error for an `*Srgb`
  `TargetDesc::format` (the format is known at prepare time, so the story's
  first option applies; no render-difference test is needed). It also checks
  `Renderer::render`'s target descriptors, which are all non-sRGB; gup-egui and
  gup-bevy (an `Rgba8UnormSrgb` texture through a non-sRGB view) are unaffected.
  `TargetDesc::format` docs point to it.
- Test: `tests/targets.rs` `prepare_rejects_srgb_formats`.

## Retrospective

**Completed**: 2026-11-06

### Key Technical Learnings

#### Reject, don't convert

- **Challenge**: a host's sRGB pass draws translucent marks lighter, silently.
- **Solution**: following the Decision (no shader switch), the format is checked
  where it is first known. The error names the fix.
- **Pattern**: make a host contract a prepare-time error when the input that
  breaks it is visible there.

### Architectural Decisions

#### Error also for scenes with only opaque marks

- **Decision**: the check does not look at the scene, so an sRGB format fails
  even when nothing is translucent.
- **Reasoning**: one rule is simpler to document and a scene gains translucent
  marks without the host changing.
- **Trade-off**: a host that only draws opaque marks into an sRGB view must make
  a non-sRGB view too.

### Follow-up Stories

None.
