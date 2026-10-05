# GUP-401: RFC-001 S3 — Scene, Renderer and RenderTarget

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-06)
**Created**: 2026-10-05

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

- [x] The `RenderTarget` trait (`desc`, `acquire(cx) -> Frame`,
      `present(cx, frame, cmd)`, RFC-001 §7) exists. `ImageTarget` (GUP-395) and
      the window target (GUP-396, if landed — otherwise built fresh here) are
      refactored to implement it, replacing any bespoke
      `render_to_png`/`render_to_window`-shaped functions with one trait-driven
      call site.
- [x] `TextureTarget` (host-owned `wgpu::Texture`, for egui/bevy-style embedding
      per RFC-001 §7) is added, implementing the same trait.
- [x] A draw-in-pass path (`Renderer::prepare(cx, scene, desc) -> Prepared`,
      `Prepared::draw(pass)`) lets a caller record Gup's draw calls into a
      `wgpu::RenderPass` it already owns, without Gup submitting a command
      buffer itself — the rule RFC-001 §2 states for `from_wgpu`-constructed
      contexts ("Gup never submits on the host's behalf in draw-in-pass mode").

### AC2: `Scene`/`Renderer` separate "what to draw" from "where"

- [x] GUP-395's `encode_scene` is split into `Renderer::prepare` (build
      pipelines/bind groups/uniforms for a `Scene` against a `TargetDesc`) and
      `Prepared::draw` (record the draw calls into a pass), so one prepared
      scene can be drawn into an `ImageTarget`'s offscreen pass, a
      `WindowTarget`'s surface pass, a `TextureTarget`'s pass, or a host's pass
      via draw-in-pass — with no per-target duplication of the encoding logic.
- [x] GUP-395's existing guarantees carry over unchanged and are re-verified:
      one render pass, premultiplied output, scissor clipping, non-sRGB view.
      `cargo test -p gup-core --lib --test scatter_png` passes unmodified in its
      assertions (only the call path to produce the PNG may change).

### AC3: MSAA

- [x] `TargetDesc.samples` is part of the pipeline cache key (RFC-001 §6: key is
      `(MarkId, EncodingSignature, TargetDesc{format, samples}, Variant)`).
      Gup-owned targets (`ImageTarget`, `WindowTarget`) default to 4 samples; a
      pick-variant pipeline (even if no pick path exists yet — S9's job) is
      documented as always using 1 sample.
- [x] **AC (user-visible, required)**: a rendered image containing a primitive
      with a diagonal or curved edge (the existing Circle mark qualifies) is
      compared at 1×and 4× MSAA, and the 4× render is verified by eye to show
      visibly smoother edge antialiasing — described in the retrospective with
      both images referenced, not just "pixel count changed".

### AC4: Rects and gradients

- [x] `ItemKind::Rects` (RFC-001 §7: plot background, legend swatches) renders
      flat-coloured rectangles as part of the one-pass `Scene` encoding.
- [x] `ItemKind::Gradient` (colour legend bar) renders a linear gradient
      sampling the same LUT texture approach `Sequential`'s fill already uses
      (GUP-395), so the legend and the data fill are provably driven by the same
      palette data.
- [x] **AC (user-visible, required)**: a rendered PNG shows a plot-background
      rect behind the marks and a gradient legend bar whose start/end colours
      match the `Sequential` scale's domain extremes — verified by eye and
      described in the retrospective.

### AC5: `SvgTarget` for guides

- [x] `SvgTarget` implements `VectorTarget` (RFC-001 §7:
      `fn render(&mut self, scene: &Scene) -> Result<()>`) and renders the
      `Scene`'s `Rules` (axis lines/ticks) and `Text` (tick labels, title) as
      SVG `<line>`/`<text>` elements. `ItemKind::Marks` is explicitly **not**
      rendered by this story's `SvgTarget` (see Context "Scope note") — a clear
      error or a documented no-op, not a silent drop, if a `Scene` containing
      marks is passed to it.
- [x] A written SVG file produced by this story's test is opened/read and
      described in the retrospective: the expected axis lines are present as
      `<line>` elements, and tick-label/title text is present as `<text>`
      elements with the correct string content.

### AC6: WASM binary-size budget (RFC-001 §12 risk 10)

- [x] `gup-core` is built for `wasm32-unknown-unknown` (release profile,
      matching this project's existing `wasm-pack`/WASM tooling conventions) and
      its gzipped size is measured.
- [x] The naga_oil-attributable portion of that size is estimated — for example
      by comparing against a build with naga_oil's library-module composition
      stubbed/disabled, or by inspecting the naga_oil + naga dependency
      closure's contribution via a size-profiling tool already available in the
      flake, or added if none is (record which approach was used and why).
- [x] The measurement (gzipped KB, and the naga_oil-attributable delta) is
      recorded in this story's retrospective and appended to RFC-001 as a new
      dated subsection (matching GUP-395's naga_oil-timing write-up), compared
      against the §12 risk 10 budget (≤ +400 KB gz). If the budget is exceeded,
      that is recorded as a finding requiring a follow-up decision on the §6
      "Fallback" import-only-concatenation path — this story does not need to
      implement the fallback, only to measure and report honestly.

## Technical Tasks

- [x] Define `TargetDesc`/`Frame`/the `RenderTarget` trait in `gup-core` (or
      confirm/refine GUP-395's existing `render.rs`/`target.rs` shapes against
      RFC-001 §7's exact signatures).
- [x] Refactor `ImageTarget` (and `WindowTarget`, if GUP-396 has landed) to
      implement the trait; remove any bespoke per-target encode functions in
      favour of `Renderer::prepare`/`Prepared::draw`.
- [x] Implement `TextureTarget`.
- [x] Implement the draw-in-pass path (`Renderer::prepare`/`Prepared::draw`) and
      a test that records into a caller-owned pass without Gup submitting.
- [x] Add `samples: u32` to `TargetDesc` and thread it into the pipeline cache
      key; set Gup-owned targets to 4 samples by default, 1 for pick variants.
- [x] Add `ItemKind::Rects` and `ItemKind::Gradient` to `Scene`, with their
      WGSL/pipeline support in the one-pass encoder.
- [x] Implement `SvgTarget` for guides only, per AC5's scope note.
- [x] Measure and record the WASM gzipped size and the naga_oil-attributable
      delta; append the RFC-001 subsection (AC6).
- [x] Verify the old `gup` crate's frozen `src/` tree still has zero diff from
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

- [x] `RenderTarget` is implemented by `ImageTarget`, a window target and
      `TextureTarget`, plus a working draw-in-pass path.
- [x] MSAA is on by default for Gup-owned targets and verified by eye to improve
      edge quality.
- [x] `Scene` can render plot-background rects and a colour-legend gradient.
- [x] `SvgTarget` produces correct guide-only SVG, verified by reading the file.
- [x] The WASM gzipped-size delta attributable to naga_oil is measured and
      recorded against the ≤ +400 KB gz budget in RFC-001.
- [x] `git diff main -- <old-path files>` remains empty.

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] MSAA comparison, rect/gradient PNG, and guides-only SVG each verified by
      eye/by reading the file and described in the retrospective
- [x] WASM gzipped-size measurement recorded in the retrospective and appended
      to RFC-001
- [x] No diff against `main` in the old `gup` path (same file list as GUP-395)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Everything in scope was delivered, nothing was split out. The WASM measurement
found the naga_oil path over budget; the follow-up decision is GUP-406. Numbers
and evidence are in RFC-001 "S3 findings".

- **Targets** (`crates/gup-core/src/target.rs`, `window.rs`, `render.rs`):
  - `TargetDesc` gains `dpr`.
  - `TargetOptions` (dpr, samples; default 1.0 and 4) sets up `ImageTarget` and
    the new `TextureTarget`. `WindowTarget` defaults to 4 samples
    (`set_samples`).
  - `Frame` carries the multisampled view; `Frame::color_attachment` resolves it
    into the frame's texture.
  - `Renderer::prepare` validates dpr and samples and documents the draw-in-pass
    contract.
- **Counted submits** (`context.rs`): `Context::submit`/`submissions` and the
  `every_submit_is_counted` source check. `Context::pipeline_stats` exposes
  `PipelineStats`, which was public but unreachable before.
- **Scene items** (`scene.rs`, `shaders/rect.wgsl`, `shaders/gradient.wgsl`):
  - `ItemKind::Rects(Vec<RectPrim>)` and `ItemKind::Gradient(GradientBar)`;
    `GradientBar::sequential` shares the scale's LUT `Arc`.
  - `GradientDirection`, `TextRole::Legend`, `Z_GRID`, `Scene::guides()`.
  - Items and scenes are `Clone`.
  - One LUT sampler (`scale::sequential::sample_lut`) serves the CPU mirror and
    `GradientBar::color_at`.
- **`SvgTarget` and `VectorTarget`** (`svg.rs`). In gup-text:
  `Font::baseline_origin` and the public `INTER_REGULAR`.
- **Browser fixes**: `web-time` replaces `std::time::Instant`. Uniform `Params`
  structs are padded to 16 bytes (`scale_linear.wgsl`, `color_sequential.wgsl`,
  `LinearParams`, `SequentialParams`), the glue emitter asserts it and pads
  constants, and `DynShaderFn::params_size` is new.
- **WASM**: naga_oil without its default `glsl` feature. Harnesses are in
  `crates/gup-core/wasm-size/{baseline,scatter}`, with new mask tasks
  `wasm-size` and `wasm-browser`.
- **Tests**:
  - new integration files `tests/targets.rs` (4 tests), `tests/scene_items.rs`
    (2), `tests/msaa.rs` (2) and `tests/svg.rs` (1);
  - shared `tests/common/{vr,legend}.rs`;
  - 4 new goldens (`scene_items`, `msaa_1x`, `msaa_4x`, `scene_guides_svg`);
  - the glue fixtures re-blessed for the padding;
  - new unit tests: SVG helpers, the WGSL round-trip layout check, the submit
    counter and `baseline_origin`.
  - gup-core lib + integration: 63 passed, 0 failed. Doctests 4, compile-fail 1,
    gup-text 11. `window_parity` (needs a display) passes with ΔE 0.
  - The visual-regression CI workflow runs the new test targets.
- **Old path frozen**: `git diff 630bc6d -- src/ examples/ gup-macros/ assets/`
  is empty, and old-path LOC is still 28,935.

## Retrospective

**Completed**: 2026-10-06

### Key Technical Learnings

#### MSAA only matters for geometric edges

- **Challenge**: The AC asked for the Circle mark to look smoother at 4×. It
  can't: circles (like rules' ends, text and points) antialias analytically. The
  fragment shader computes coverage once per pixel, and MSAA averages four
  identical samples.
- **Solution**: Compare a scene with circles, a fan of diagonal 1.5 px rules and
  a rect on fractional coordinates. On the rules and the rect, 1× has **0**
  partially covered pixels and 4× has **597** and **71**. Circles differ by at
  most 2/255. The 8× side-by-side crop
  (`$CARGO_TARGET_DIR/visual-regression/gup_core/msaa_compare.png`) shows
  stair-stepped 1× rules on the left and blended 4× edges on the right. The
  circle crop (`msaa_compare_circle.png`) looks the same in both. The goldens
  `tests/golden/gup_core/msaa_1x.png` and `msaa_4x.png` keep both renders.
- **Pattern**: Measure partially covered pixels, not "pixels changed". A pixel
  that is neither background nor ink is an antialiased edge, and the count is 0
  by construction without antialiasing.

#### Two rasterisers never agree on glyph pixels

- **Challenge**: The resvg raster of the SVG put 151 ink pixels outside the
  GPU's text boxes.
- **Solution**: Half of the gap was real: SVG placed the baseline at y = 435.5
  where the GPU snaps to 436. The SVG now snaps pen start and baseline to whole
  pixels as the GPU does, shifting the `text-anchor` x by the same rounding. The
  remaining 23 pixels were one faint antialiased column from outline
  rasterisation at sub-pixel positions. The SVG adapter's text boxes are 1 px
  wider, with the reason in a comment. Pixels are compared outside text, and ink
  per label inside it.
- **Pattern**: When two renderers legitimately differ in one region, compare
  that region by an aggregate (ink) and require exact agreement everywhere else,
  rather than loosening a whole-image tolerance.

#### The browser is a different backend, and only running in it finds that

- **Challenge**: gup-core "built for wasm32" since S1, and every native test
  passed. The first real browser run panicked (`Instant::now`). After that fix
  it flooded the plot with one colour.
- **Solution**: wgpu's WebGPU backend turns the naga module back into WGSL, and
  naga's WGSL writer drops `@align`. With `uniform_buffer_standard_layout`,
  Chrome accepted the unaligned natural offsets, so radius was read from
  `y.range_start`. Padding members would break WGSL's "`roundUp(16, size)` after
  a struct member" rule (naga enforces it too). The structs themselves are
  padded to 16 bytes instead. A native unit test re-parses naga's WGSL output
  and compares every struct's layout, and `mask wasm-browser` runs the browser
  path.
- **Pattern**: Never rely on IR-only attributes surviving a backend round trip.
  If a value crosses an IR → source → IR boundary, test the round trip.

### Architectural Decisions

#### SVG fonts are referenced, not embedded

- **Decision**: `font-family="Inter, sans-serif"`; text placed with Inter's
  metrics and `text-anchor`.
- **Reasoning**: The full Inter is ~550 KB of base64 per file, and resvg ignores
  `@font-face` anyway. Viewers with Inter match the PNG; others keep centred and
  right-aligned labels in place.
- **Trade-off**: Viewers without Inter show a different face.
- **Future**: A 24 KB-gzip subset makes optional embedding cheap (GUP-407).

#### `TextureTarget` resolves into the host's texture

- **Decision**: It requires a single-sample texture with `RENDER_ATTACHMENT` and
  keeps its own MSAA texture. A multisampled host texture is an error that
  points to `Prepared::draw`.
- **Reasoning**: The host samples the result as an ordinary texture. Hosts that
  want their own multisampling own the pass.
- **Trade-off**: An sRGB host texture must list its non-sRGB twin in
  `view_formats`. wgpu offers no way to check this up front, so it is documented
  rather than validated.

#### Submissions are counted like uploads

- **Decision**: `Context::submit` is the only way gup-core submits, enforced by
  a source scan.
- **Reasoning**: "Never submits on the host's behalf" (§2) became a testable
  claim at almost no cost.

#### Measure naga_oil's cost by adding it, not by removing it

- **Decision**: A bare-wgpu harness with a `naga-oil` feature that composes the
  same shader through naga_oil. gup-core can't run without its composer, so
  stubbing it out was not an option.
- **Trade-off**: The delta also includes naga's WGSL writer, which wgpu's WebGPU
  backend needs only because of `ShaderSource::Naga`. That is part of the cost
  of the naga path, so it is counted.

### Development Workflow Insights

- Running in the real browser took about 30 minutes, with miniserve, headless
  Chromium, `--enable-logging=stderr` for the console, and a PNG data URL logged
  and decoded. It found two bugs that compile checks and the native suite could
  not, and it is now `mask wasm-browser`.
- `pkill -f <pattern>` matches the invoking shell's own command line. It killed
  the tool shell once.
- `/tmp/gup-target` reached 16 GB, 7.9 GB of it incremental caches. Deleting
  `debug/incremental` is safe and freed 8 GB. The wasm release directory was
  deleted after each measurement.
- `zoom_bench` in a small, unfocused window reports ~1 fps: the compositor stops
  sending frame callbacks. Measure it fullscreen as S0b did.
- The orchestrator's workflow fix (548131a) went in as its own commit, between
  story commits.

### Follow-up Stories

1. **GUP-406: WGSL-only shader path on wasm** — Decide on, then build, §6's
   import-only concatenation fallback for wasm32. It passes `ShaderSource::Wgsl`
   and drops naga and naga_oil from the browser build: −897 KB gz against a +400
   KB budget.
2. **GUP-407: Subset the bundled Inter; optional SVG font embedding** — A Latin
   and chart-symbol subset (23.8 KB gz against 198.3 KB) with `kern`, `tnum` and
   `lnum` kept, and `SvgOptions` to embed it.
3. **GUP-408: Run gup-core's browser smoke test in CI** — `mask wasm-browser`
   needs a GPU. Try Chromium's SwiftShader WebGPU on GitHub runners so the
   browser-only bug class is caught on every push.

Noted for later RFC steps, without stories: title overflow at narrow widths and
a legend layout slot (S7); SVG mark export (S4/S5, `MarkBatch::vector()`);
starting S8's wasm entry point from the `wasm-size/scatter` harness.
