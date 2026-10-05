# GUP-401: RFC-001 S3 — Scene, Renderer and RenderTarget

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-05

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §7 specifies a `RenderTarget`
trait (`desc`, `acquire`, `present`) implemented by `ImageTarget` (offscreen +
async readback), `WindowTarget` (winit surface), `TextureTarget` (host-owned
texture, for egui/bevy-style embedding), plus a draw-in-pass path
(`Renderer::prepare` / `Prepared::draw`) for hosts that own their own render
pass, and a `VectorTarget` trait for SVG/PDF guides. This is RFC-001's S3 story
(§11 migration table): "`Scene`, `Renderer`, `RenderTarget` (Image, Window,
Texture, draw-in-pass); `SvgTarget` for guides; units and colour policy; MSAA."

[GUP-395](GUP-395_Gup_Core_Vertical_Slice_Headless.md) (S0a) built `ImageTarget`
and a one-pass `encode_scene` function for a single mark (Circle) and a
title/tick-label `Scene`, already delivering several of §7's hard guarantees:
one render pass, premultiplied output, scissor clipping, and a non-sRGB view
(`Rgba8Unorm`) for colour parity with browsers/SVG/PDF.
[GUP-396](GUP-396_Gup_Core_Vertical_Slice_Window_Performance.md) (S0b, in
progress at the time this story was written) adds a second target —
`WindowTarget` — to prove window/PNG visual parity and the zero-column-byte zoom
claim, for the same bespoke `ImageTarget`/`WindowTarget` call sites. This story
is where those two spike-quality targets, built to prove specific exit criteria,
are turned into the general trait-based family RFC-001 §7 specifies, with
`TextureTarget` and the draw-in-pass path added for the first time. (If GUP-396
has not yet landed when this story starts, treat its `WindowTarget` as useful
prior art to generalise from, not a hard blocker — this story's listed
prerequisites are S1 and S2, since `RenderTarget` needs `Context::shared()`'s
lock-order guarantees and `gup-text`'s pass-drawing API, not GUP-396
specifically.)

Three pieces of RFC-001 §7 remain entirely unbuilt after S0a/S0b: **MSAA** (§7:
"Polygons and areas use MSAA 4× by default on Gup-owned targets... Pick passes
use 1 sample... part of the pipeline key"), **`ItemKind::Rects` and
`ItemKind::Gradient`** (plot background, legend swatches, colour-legend bar —
`Scene`'s other item kinds besides `Marks` and `Text`), and a **`SvgTarget`**
for guides (§7: "Vector output draws guides directly... Above
`SvgOptions::max_vector_marks`... the marks layer is rasterised"). This story
also carries RFC-001 §12 risk 10 ("WASM... naga_oil adds binary size... measure
binary size in S3 with a budget (≤ +400 KB gz)"), which S0a's findings
explicitly deferred to this story.

**Scope note on `SvgTarget`.** RFC-001 §7's full vector-output design (CPU
mirrors evaluating retained columns to emit mark paths, with a rasterised
fallback above `max_vector_marks`) is `MarkBatch::vector()` — substantial work
tied to the column store and scale CPU mirrors that land in S4/S5, not this
story. This story's `SvgTarget` renders only the `Scene`'s **guides** (axis
`Rules` and `Text` runs) as SVG; mark export is explicitly out of scope and is a
later story once S4/S5 exist.

## User Story

> "As a Gup implementer validating RFC-001, I want the ad hoc `ImageTarget` and
> `encode_scene` from the S0 spike turned into the general
> `RenderTarget`/`Renderer` trait family — with `TextureTarget` and draw-in-pass
> added, MSAA and new `Scene` item kinds (rects, gradients) working, an
> `SvgTarget` for guides, and `gup-core`'s WASM binary-size cost measured — so
> that later stories can add render targets and item kinds without duplicating
> encoding logic per target."

## Acceptance Criteria

### AC1: `RenderTarget` trait generalises the spike's targets

- [ ] The `RenderTarget` trait (`desc`, `acquire(cx) -> Frame`,
      `present(cx, frame, cmd)`, RFC-001 §7) exists. `ImageTarget` (GUP-395) and
      the window target (GUP-396, if landed — otherwise built fresh here) are
      refactored to implement it, replacing any bespoke
      `render_to_png`/`render_to_window`-shaped functions with one trait-driven
      call site.
- [ ] `TextureTarget` (host-owned `wgpu::Texture`, for egui/bevy-style embedding
      per RFC-001 §7) is added, implementing the same trait.
- [ ] A draw-in-pass path (`Renderer::prepare(cx, scene, desc) -> Prepared`,
      `Prepared::draw(pass)`) lets a caller record Gup's draw calls into a
      `wgpu::RenderPass` it already owns, without Gup submitting a command
      buffer itself — the rule RFC-001 §2 states for `from_wgpu`-constructed
      contexts ("Gup never submits on the host's behalf in draw-in-pass mode").

### AC2: `Scene`/`Renderer` separate "what to draw" from "where"

- [ ] GUP-395's `encode_scene` is split into `Renderer::prepare` (build
      pipelines/bind groups/uniforms for a `Scene` against a `TargetDesc`) and
      `Prepared::draw` (record the draw calls into a pass), so one prepared
      scene can be drawn into an `ImageTarget`'s offscreen pass, a
      `WindowTarget`'s surface pass, a `TextureTarget`'s pass, or a host's pass
      via draw-in-pass — with no per-target duplication of the encoding logic.
- [ ] GUP-395's existing guarantees carry over unchanged and are re-verified:
      one render pass, premultiplied output, scissor clipping, non-sRGB view.
      `cargo test -p gup-core --lib --test scatter_png` passes unmodified in its
      assertions (only the call path to produce the PNG may change).

### AC3: MSAA

- [ ] `TargetDesc.samples` is part of the pipeline cache key (RFC-001 §6: key is
      `(MarkId, EncodingSignature, TargetDesc{format, samples}, Variant)`).
      Gup-owned targets (`ImageTarget`, `WindowTarget`) default to 4 samples; a
      pick-variant pipeline (even if no pick path exists yet — S9's job) is
      documented as always using 1 sample.
- [ ] **AC (user-visible, required)**: a rendered image containing a primitive
      with a diagonal or curved edge (the existing Circle mark qualifies) is
      compared at 1×and 4× MSAA, and the 4× render is verified by eye to show
      visibly smoother edge antialiasing — described in the retrospective with
      both images referenced, not just "pixel count changed".

### AC4: Rects and gradients

- [ ] `ItemKind::Rects` (RFC-001 §7: plot background, legend swatches) renders
      flat-coloured rectangles as part of the one-pass `Scene` encoding.
- [ ] `ItemKind::Gradient` (colour legend bar) renders a linear gradient
      sampling the same LUT texture approach `Sequential`'s fill already uses
      (GUP-395), so the legend and the data fill are provably driven by the same
      palette data.
- [ ] **AC (user-visible, required)**: a rendered PNG shows a plot-background
      rect behind the marks and a gradient legend bar whose start/end colours
      match the `Sequential` scale's domain extremes — verified by eye and
      described in the retrospective.

### AC5: `SvgTarget` for guides

- [ ] `SvgTarget` implements `VectorTarget` (RFC-001 §7:
      `fn render(&mut self, scene: &Scene) -> Result<()>`) and renders the
      `Scene`'s `Rules` (axis lines/ticks) and `Text` (tick labels, title) as
      SVG `<line>`/`<text>` elements. `ItemKind::Marks` is explicitly **not**
      rendered by this story's `SvgTarget` (see Context "Scope note") — a clear
      error or a documented no-op, not a silent drop, if a `Scene` containing
      marks is passed to it.
- [ ] A written SVG file produced by this story's test is opened/read and
      described in the retrospective: the expected axis lines are present as
      `<line>` elements, and tick-label/title text is present as `<text>`
      elements with the correct string content.

### AC6: WASM binary-size budget (RFC-001 §12 risk 10)

- [ ] `gup-core` is built for `wasm32-unknown-unknown` (release profile,
      matching this project's existing `wasm-pack`/WASM tooling conventions) and
      its gzipped size is measured.
- [ ] The naga_oil-attributable portion of that size is estimated — for example
      by comparing against a build with naga_oil's library-module composition
      stubbed/disabled, or by inspecting the naga_oil + naga dependency
      closure's contribution via a size-profiling tool already available in the
      flake, or added if none is (record which approach was used and why).
- [ ] The measurement (gzipped KB, and the naga_oil-attributable delta) is
      recorded in this story's retrospective and appended to RFC-001 as a new
      dated subsection (matching GUP-395's naga_oil-timing write-up), compared
      against the §12 risk 10 budget (≤ +400 KB gz). If the budget is exceeded,
      that is recorded as a finding requiring a follow-up decision on the §6
      "Fallback" import-only-concatenation path — this story does not need to
      implement the fallback, only to measure and report honestly.

## Technical Tasks

- [ ] Define `TargetDesc`/`Frame`/the `RenderTarget` trait in `gup-core` (or
      confirm/refine GUP-395's existing `render.rs`/`target.rs` shapes against
      RFC-001 §7's exact signatures).
- [ ] Refactor `ImageTarget` (and `WindowTarget`, if GUP-396 has landed) to
      implement the trait; remove any bespoke per-target encode functions in
      favour of `Renderer::prepare`/`Prepared::draw`.
- [ ] Implement `TextureTarget`.
- [ ] Implement the draw-in-pass path (`Renderer::prepare`/`Prepared::draw`) and
      a test that records into a caller-owned pass without Gup submitting.
- [ ] Add `samples: u32` to `TargetDesc` and thread it into the pipeline cache
      key; set Gup-owned targets to 4 samples by default, 1 for pick variants.
- [ ] Add `ItemKind::Rects` and `ItemKind::Gradient` to `Scene`, with their
      WGSL/pipeline support in the one-pass encoder.
- [ ] Implement `SvgTarget` for guides only, per AC5's scope note.
- [ ] Measure and record the WASM gzipped size and the naga_oil-attributable
      delta; append the RFC-001 subsection (AC6).
- [ ] Verify the old `gup` crate's frozen `src/` tree still has zero diff from
      `main`.

## Dependencies

### Prerequisite Stories

- GUP-399: RFC-001 S1 — One `gup::Context` 📋 — `Renderer`/`RenderTarget` is
  built against the production `Context`, including its documented lock order
  and `Context::shared()`.
- GUP-400: RFC-001 S2 — Extract `gup-text` 📋 — the one-pass `Renderer` draws
  text through `gup-text`'s pass-drawing API (AC2), not the temporary module S2
  deletes.

### Enables Stories

- RFC-001 S4 (column store v1) and S5 (full scale family) will add the mark
  types and encodings that make `SvgTarget`'s deferred `MarkBatch::vector()`
  work meaningful.
- RFC-001 S13 (re-wire `gup-egui`/`gup-bevy`) depends on `TextureTarget` and the
  draw-in-pass path this story adds.
- Any later story porting a builder (S10+) depends on `RenderTarget`'s general
  shape rather than S0a/S0b's bespoke targets.

## Testing Strategy

- **Unit tests**: `TargetDesc`/pipeline-cache-key behaviour with varying
  `samples`; `Scene` construction with `Rects`/`Gradient` items.
- **Integration tests**: the same `Scene` drawn through `ImageTarget`,
  `TextureTarget` and the draw-in-pass path produces equivalent output (within
  the existing ΔE tolerance GUP-396 establishes for window/PNG parity, if
  landed).
- **Visual validation**: MSAA edge-smoothness comparison (AC3), the rect/
  gradient PNG (AC4), and the guides-only SVG (AC5) are each verified by eye or
  by reading the file directly, and described in the retrospective.
- **Performance**: the WASM gzipped-size measurement (AC6) is recorded as a
  number, compared against the ≤ +400 KB gz budget.

## Success Metrics

- [ ] `RenderTarget` is implemented by `ImageTarget`, a window target and
      `TextureTarget`, plus a working draw-in-pass path.
- [ ] MSAA is on by default for Gup-owned targets and verified by eye to improve
      edge quality.
- [ ] `Scene` can render plot-background rects and a colour-legend gradient.
- [ ] `SvgTarget` produces correct guide-only SVG, verified by reading the file.
- [ ] The WASM gzipped-size delta attributable to naga_oil is measured and
      recorded against the ≤ +400 KB gz budget in RFC-001.
- [ ] `git diff main -- <old-path files>` remains empty.

## Risk Assessment

- **Medium**: generalising two spike-quality targets into a trait family while
  also adding two new targets (`TextureTarget`, draw-in-pass) and three new
  rendering features (MSAA, rects/gradients, SVG guides) is a lot of surface for
  one story. _Mitigation_: each AC is independently verifiable and the technical
  tasks are ordered so the trait refactor (AC1/AC2) lands and is tested before
  the additive features (AC3-AC5); if the story proves too large once underway,
  split the WASM-size measurement (AC6, which has no code dependency on the
  others) into its own follow-up rather than cutting corners on the rendering
  ACs.
- **Medium**: RFC-001 §12 risk 10's WASM budget may already be exceeded by
  naga*oil alone, independent of anything this story adds — in which case the
  honest outcome is "over budget, needs a decision", not a passing checkbox.
  \_Mitigation*: AC6 explicitly requires recording an over-budget result rather
  than treating it as a story failure; it feeds a decision for a later story
  (the §6 fallback), which this story does not have to implement.
- **Low**: no existing tool in the flake may measure WASM binary size
  attribution cleanly (e.g. `twiggy`/`cargo bloat` for a
  `wasm32-unknown-unknown` target). _Mitigation_: a coarse comparison (gzipped
  size with vs. without the naga_oil-composed library modules, if that's
  feasible to stub) is acceptable if a precise attribution tool isn't readily
  available; record the method used.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] MSAA comparison, rect/gradient PNG, and guides-only SVG each verified by
      eye/by reading the file and described in the retrospective
- [ ] WASM gzipped-size measurement recorded in the retrospective and appended
      to RFC-001
- [ ] No diff against `main` in the old `gup` path (same file list as GUP-395)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
