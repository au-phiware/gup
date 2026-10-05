# GUP-400: RFC-001 S2 — Extract `gup-text`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-06)
**Created**: 2026-10-05

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §11's "Survive (move)" row lists
`text/{font,layout,msdf,atlas,sdf_tuning,style}` → `gup-text` as code that moves
out of the frozen old path into a new leaf crate, used by every `gup-core`
render target so that PNG, window and SVG output measure text with one
implementation. This is RFC-001's S2 story (§11 migration table): "Extract
`gup-text` (leaf crate), one `TextSystem` per `Context`, measuring API,
glyph-run draw into a pass, Inter bundled."

[GUP-395](GUP-395_Gup_Core_Vertical_Slice_Headless.md) deliberately did not do
this extraction. It built a **temporary, internal-only** ~300-line text module
(`crates/gup-core/src/text.rs`) — one embedded font (`fontdue` rasterisation,
one bitmap atlas, straight-line horizontal layout) — explicitly scoped to shape
and draw the title and tick-label `TextRun`s its exit criteria needed, and
explicitly marked as superseded by this story. Building the general `gup-text`
API during an already-large spike would have pulled unrelated API-design work
into S0a; GUP-395's retrospective records this as a deferred follow-up directly
naming this story.

The old path's `src/text/` tree is 12,175 lines across ten files. Per RFC-001
§11, only six of those files "survive" into `gup-text`: `font.rs` (1,447 lines),
`layout.rs` (3,157 lines), `msdf.rs` (3,090 lines), `atlas.rs` (829 lines),
`sdf_tuning.rs` (344 lines) and `style.rs` (349 lines) — `hover_reveal.rs`,
`tooltip_bg.rs`, `ui_quad.rs` and `renderer.rs` are widget-shaped or tied to the
old pipeline and are **not** ported (GUP-395's Context section already excludes
the first three for the same reason; this story keeps that exclusion). "Survive"
does not mean every line of those six files is needed verbatim: this story only
needs to replace the temporary module's capability (measure + draw
title/tick-label runs into a pass) with a real, externally-usable crate —
advanced layout features (wrapping, rotation, multi-line alignment) that no
current `gup-core` caller needs can stay unported until a later story needs
them. See the Risk Assessment for how to decide the bitmap-vs-MSDF rasterisation
question without over-scoping this story.

**Font coordination.** `gup-core`'s temporary text module embeds the exact same
asset the old path embeds — `assets/fonts/default.ttf` via
`include_bytes!("../../../assets/fonts/default.ttf")` — currently "Squada One".
[GUP-392](GUP-392_Correct_Visual_Defaults.md) (📋 Planned, old path, independent
of this story) replaces that same file with Inter (SIL OFL 1.1) and adds its
licence file under `assets/fonts/`. Because both paths point at the same
physical asset, whichever story lands first does the actual font swap, and
whichever lands second inherits it for free but must re-bless its own golden
image if the rendered glyphs changed. Do not duplicate the font swap or the
licence file if GUP-392 has already landed.

## User Story

> "As a Gup implementer validating RFC-001, I want `gup-core`'s temporary
> internal text module replaced by a real `gup-text` leaf crate — one
> `TextSystem` per `Context`, a measuring API, and glyph-run drawing into an
> existing render pass — so that text rendering is no longer a throwaway spike
> and is ready for S3's `Renderer` to draw through the same one-pass path as
> marks and guides."

## Acceptance Criteria

### AC1: `gup-text` exists as an isolated leaf crate

- [x] `crates/gup-text` is a new workspace member with zero dependency on `gup`
      or `gup-core` (`cargo tree -p gup-text` lists neither); `gup-core` depends
      on it, not the other way round.
- [x] Font loading, atlas management and layout/rasterisation logic adapted from
      `src/text/{font,layout,msdf,atlas,sdf_tuning,style}.rs` is moved into
      `gup-text`, with every dependency on old-path types (`GupContext`,
      `RenderContext`, the old `Mark`/`Selection` types) removed. The old
      `src/text/` tree itself has **zero diff** from `main` at the start of this
      story (verified the same way GUP-395's AC1 verified the rest of the frozen
      path) — this is an extraction into a new crate, not a rewrite of the old
      one in place.
- [x] The default bundled font is Inter if GUP-392 has already landed (shared
      asset, see Context), or stays whatever `assets/fonts/default.ttf`
      currently is if GUP-392 has not yet landed — this story does not perform
      its own separate font swap or add a second licence file. **Changed at
      implementation (orchestrator, owner decision in the strategic review,
      Decisions §4):** `gup-text` bundles Inter Regular 4.1 and its OFL licence
      in `crates/gup-text/fonts/`. It is the only copy of Inter in the
      repository. `assets/fonts/default.ttf` and the frozen old path are
      unchanged, and GUP-392 records that its font item is done.

### AC2: One `TextSystem` per `Context`, with a measuring API

- [x] `gup_text::TextSystem` owns one glyph atlas and font per device
      (constructed from a `&wgpu::Device`/`&wgpu::Queue`, not from `gup-core`
      types, since `gup-text` cannot depend on `gup-core`). `gup_core::Context`
      holds exactly one `Mutex<gup_text::TextSystem>`, replacing its internal
      `text: Mutex<TextSystem>` from the temporary module
      (`crates/gup-core/src/text.rs`, which is deleted by this story).
- [x] `TextSystem` exposes a measuring API returning the same shape of
      information (width, cap-height, descent, ink bounds per run) that
      `gup_core::Layout` already surfaces to the GUP-388 visual-regression
      harness adapter. The harness adapter's existing contract with
      `gup_core::Layout` does not change as an observable effect of this story
      (GUP-395's retrospective: "Keep `measure` and the ink bounds the harness
      adapter uses").

### AC3: Glyph-run drawing into an existing pass

- [x] `TextSystem` (or a value it returns, e.g. a prepared glyph batch) can
      record glyph-quad draw calls into an existing `wgpu::RenderPass` rather
      than opening its own pass or submitting its own command buffer — the "one
      render path" principle (RFC-001 §1, §7) that S3's `Renderer`/ `Prepared`
      abstraction depends on.
- [x] `gup-core`'s scene encoder is switched from the temporary module's draw
      path to `gup-text`'s pass-drawing API. GUP-395's title and tick-label
      `TextRun`s still render; `cargo test -p gup-core --lib --test scatter_png`
      still passes.

### AC4: Golden output still correct after the extraction (user-visible)

- [x] **AC (user-visible, required)**: `tests/golden/gup_core/scatter.png` is
      re-blessed if the rasteriser or font changed, and verified by eye: title
      centred above the plot, tick labels on both axes legible, glyphs sharp
      with no garbling, matching (or, if the font changed to Inter, improving
      on) GUP-395's description. A written note (what changed, what was checked)
      goes in the retrospective.
- [x] If GUP-392 has not yet landed, this story's font stays unchanged from
      GUP-395's golden — re-blessing happens only if the rasteriser (e.g. bitmap
      vs. MSDF, per the Risk Assessment) measurably changes pixels, not merely
      because the code moved crates. **Changed at implementation:** the font did
      change, to Inter (see AC1), so the `gup_core` golden was re-blessed. The
      old path's goldens did not change.

### AC5: `gup-text` is exercised from outside its own crate

- [x] A test under `crates/gup-text/tests/` (external-crate style, matching
      GUP-395's AC7 pattern) constructs a `TextSystem`, measures a run, and
      records its glyph quads into a throwaway render pass, proving the public
      API is usable without reaching into `gup-text`'s internals.

## Technical Tasks

- [x] Add `crates/gup-text` as a new workspace member; depend on whatever subset
      of `fontdue`/`ttf-parser`/`fontdb`/`image`/`bytemuck` the ported code
      actually needs (GUP-395 found `fontdb`/`ttf-parser` unnecessary for its
      minimal slice — re-check whether the fuller `gup-text` needs them for
      system-font-adjacent logic in `font.rs`, or whether that stays deferred).
- [x] Port `font.rs` (loading, fallback, metrics), `atlas.rs`
      (packing/eviction), the load-bearing subset of `layout.rs` (straight-line
      horizontal layout, kerning — defer wrapping/rotation/multi-line unless a
      current caller needs them), `sdf_tuning.rs` and `style.rs`, dropping
      `GupContext`/`RenderContext` coupling.
- [x] Decide bitmap vs. MSDF rasterisation (`msdf.rs`) for this extraction;
      record the decision and its reasoning in the retrospective, the same way
      GUP-395 recorded the naga_oil go/no-go decision (see Risk Assessment).
- [x] Design and implement `TextSystem::new`, `TextSystem::measure`, and the
      pass-drawing API.
- [x] Wire `gup_core::Context` to hold `gup_text::TextSystem`; delete
      `crates/gup-core/src/text.rs`.
- [x] Update `gup_core::plot`/`scene` resolve and encode code to call the new
      measuring/drawing API; confirm `Layout`'s public shape is unchanged for
      the harness adapter.
- [x] Re-bless `tests/golden/gup_core/scatter.png` if pixels changed; verify by
      eye per AC4.
- [x] Write the external-crate integration test for `gup-text` (AC5).
- [x] Verify the old `gup` crate's `src/text/` tree (and the rest of the frozen
      old-path list) has zero diff from `main`.

## Dependencies

### Prerequisite Stories

- GUP-399: RFC-001 S1 — One `gup::Context` 📋 — this story attaches
  `gup_text::TextSystem` to `gup_core::Context` and relies on S1's documented
  `Mutex` lock order (`pipelines` → `shaders`; `text` never held across either)
  when wiring the new `TextSystem` in.

### Enables Stories

- RFC-001 S3 (`Scene`/`Renderer`/`RenderTarget`) — needs `gup-text`'s
  pass-drawing API so text draws through the same one-pass `Renderer` as marks
  and guides.
- GUP-392 (old path font swap) — coordinated, not blocking, per the Context
  section's font-sharing note.

## Testing Strategy

- **Unit tests**: `TextSystem` construction, `measure` against known glyph
  metrics, atlas packing.
- **Integration tests**: external-crate-style test in `crates/gup-text/tests/`
  (AC5); `gup-core`'s existing scatter PNG test continues to pass through the
  new text path (AC3).
- **Visual validation**: the re-blessed (or unchanged) golden PNG is read by eye
  and described in the retrospective (AC4).
- **Regression**: confirm the GUP-388 harness adapter's reading of
  `gup_core::Layout`'s text/ink-bounds data is unaffected.

## Success Metrics

- [x] `gup-text` exists as a dependency-free leaf crate with a public,
      externally-tested API.
- [x] `gup-core`'s temporary `text.rs` is deleted.
- [x] The golden PNG remains correct (title, tick labels, legible glyphs),
      re-blessed only if pixels actually changed.
- [x] `git diff main -- src/text/` (and the rest of the frozen old-path file
      list) is empty.

## Risk Assessment

- **Medium**: `msdf.rs` is 3,090 lines and tied to the old pipeline's bind
  groups and shaders; porting it in full to prove MSDF rendering in `gup-text`
  could be disproportionate to this story's actual need (title + tick labels at
  the sizes GUP-395 already proved legible with plain bitmap rasterisation).
  _Mitigation_: this story may keep bitmap rasterisation (as GUP-395's spike
  did) if MSDF porting would dominate the story's size, as long as the decision
  and its reasoning are recorded in the retrospective — this mirrors how GUP-395
  recorded the naga_oil go/no-go decision rather than silently picking an
  approach. MSDF can be ported later when a builder needs sharper text at
  extreme zoom or rotation.
- **Medium**: `layout.rs` (3,157 lines) likely contains wrapping, multi-line and
  rotation logic no current `gup-core` caller needs yet. _Mitigation_: port only
  the straight-line horizontal layout and kerning GUP-395's temporary module
  already proved sufficient; do not pre-build features without a caller, per the
  project's own anti-pattern lesson (scope the class of problem the story
  actually needs, not every feature the old file happens to have).
- **Low**: if GUP-392 lands concurrently with (or just after) this story, both
  stories may touch `assets/fonts/default.ttf` and its licence file.
  _Mitigation_: the Context section's coordination note — check GUP-392's status
  before touching the font asset, and let whichever story is still open inherit
  the other's change rather than reverting it.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-text -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Golden PNG verified by eye and described in the retrospective, with the
      bitmap-vs-MSDF decision recorded
- [x] No diff against `main` in the old `gup` path (same file list as GUP-395,
      including `src/text/`)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`gup-text` is a new leaf crate (`crates/gup-text`, about 1,240 lines of source
and 260 lines of external test). It depends only on wgpu, fontdue, bytemuck and
thiserror. `gup-core` now draws all of its text through it. The temporary module
(`crates/gup-core/src/text.rs`, 315 lines) and its shader are deleted.
`gup-core` is 408 lines smaller (+126 −534).

**`gup-text`'s public API** (`crates/gup-text/src/lib.rs`):

- `Font`: `Font::inter()` returns the bundled Inter Regular 4.1, parsed once per
  process and shared. `Font::from_bytes(name, data)` returns an error that names
  the font. `line_metrics(size)` gives ascent, descent, line gap and cap height.
  `measure(text, size)` returns `TextMetrics` (width, cap height, descent).
  `ink_bounds(&Run)` returns `Bounds`.
- `Run { text, size, at, anchor }`. `Anchor`, `HAlign` and `VAlign` moved here,
  and `gup_core::scene` re-exports them.
- `TextSystem::new(&Device)` / `with_font`: one font and one atlas per device.
  It also has `measure`, `ink_bounds`, `layout(&Run, color, &mut Glyphs)` and
  `prepare(&impl Uploader, &mut GlyphBuffer, &Glyphs, &DrawTarget) -> Option<GlyphBatch>`.
- `GlyphBatch::draw(&mut RenderPass)` records into the caller's pass. It
  allocates nothing, takes no locks and binds its atlas at group 0.
- `Glyphs` holds glyph quads in physical pixels for one scale (the dpr).
  `GlyphBuffer` is a grow-only instance buffer per draw slot.
- `Uploader` is the trait every GPU write goes through. It is implemented for
  `wgpu::Queue`. `gup-core` implements it as `TextUploads`, which counts glyph
  instances as `Upload::Instances` and atlas texels as `Upload::Texture`.

**Rasterisation and atlas** (`atlas.rs`, `system.rs`, `glyph.wgsl`): fontdue
coverage bitmaps are rasterised at the physical size (`size × scale`) and placed
on whole physical pixels. They are drawn 1:1 with a nearest sampler, so text
stays sharp at any dpr. A single R8 atlas is shelf-packed from 1024², doubles
when full (keeping glyph positions) up to `max_texture_dimension_2d`, and
returns `Error::AtlasFull` beyond that. Each upload writes only the dirty
rectangle, packed tight, so the counted bytes are the bytes copied. Instances
carry atlas coordinates in texels, which the vertex shader divides by
`textureDimensions`, so quads laid out before the atlas grows stay valid. Glyph
pipelines are cached by `(format, samples)` in the `TextSystem`.

**`gup-core` wiring:**

- `Context` holds `Mutex<gup_text::TextSystem>` under the existing
  `LockRank::Text`. No new lock was added.
- `RendererGpu::text` lays out a text item and prepares its batch while holding
  only that lock. `Draw::Text(GlyphBatch)` draws without it.
- `Prepared::draw` rebinds the view at group 0 for marks and rules, because the
  glyph batch binds its atlas there.
- `Layout`'s `PlacedText::bounds` comes from `TextSystem::ink_bounds`, through
  `TextRun::layout_run()` and `From<Bounds> for Rect`. Its shape is unchanged.
- `Error::Text` wraps `gup_text::Error`.
- The non-filtering texture binding and the text pipeline kind are removed.

**Key files:** `crates/gup-text/**` (new),
`crates/gup-core/src/{context, render,scene,geom,plot,error,lib}.rs`,
`crates/gup-core/src/shader/mod.rs`, `tests/golden/gup_core/scatter.png`
(re-blessed), `.github/workflows/visual-regression.yml`, `maskfile.md`
(`ci visual-regression` runs `cargo test -p gup-text`) and
`scripts/test_pre_commit.sh` (two scoping cases for gup-text, and the corrected
`assets/fonts/default.ttf` case, which no longer reaches `gup-core`).

**Tests:**

- gup-text: 10 unit tests (font, layout, atlas) and 2 external integration tests
  (`tests/draw_into_pass.rs`, AC5).
- gup-core: 48 lib tests, plus scatter_png, zoom_uploads, compile_fail and 3
  doctests.
- `window_parity` (needs a display): the window frame is still byte-identical to
  the new golden.
- Old path: `mask visual-regression` 16/16 and `mask smoke-examples` 2/2.

**Verification of the frozen path:** `git diff d6e523b -- src/ assets/` is
empty. `src/text/` and `assets/fonts/default.ttf` are untouched.

**Old-path LOC:** 28935 before and after (Δ 0). `src/text/` is not part of the
metric. The 28935 baseline was measured at the start of this story; it is 59
higher than the 28876 that INDEX recorded after GUP-399, from changes between
the two stories (it predates this story's first change).

## Retrospective

**Completed**: 2026-10-06

### Key Technical Learnings

#### What the golden shows after the font change

- **Challenge**: The bundled face changed from Squada One (a condensed display
  face) to Inter. The golden had to be re-blessed and checked by eye, not just
  accepted.
- **Solution**: The scatter golden (720×450) was re-blessed and read at 1× and
  at 3× crops.
  - The title "Wealth, population and life expectancy" is centred above the plot
    in Inter Regular, dark grey on white.
  - The y labels (1G, 500M … 100k) are right-aligned to their ticks and
    vertically centred on them.
  - The x labels (0 … 60000) are centred under their ticks. The last one ends
    inside the image.
  - Glyphs are sharp, with grey anti-aliased edges and no garbling, missing
    glyphs or clipping.
  - Inter is wider than Squada One, so the y-label column is wider. The plot
    rect starts a few pixels further right and the points moved with it.
  - The harness's structural checks all passed before the re-bless: text
    present, marks confined, colours present. Only the golden diff failed (7.3%
    of pixels, ΔE ≤ 93), which is expected when every glyph and the plot rect
    move.
  - The `scatter_window` example's 640×480 window capture shows the same layout.
- **Pattern**: When a font changes, run the structural checks first, then
  re-bless. If they pass, the golden diff is only the glyphs and layout moving.

#### Inter has no legacy `kern` table, and its default digits are proportional

- **Challenge**: fontdue reads kerning only from the `kern` table. Inter 4 keeps
  its kerning in GPOS. `horizontal_kern` returns `None` for "AV", "To" and "11".
- **Solution**: Kerning goes through fontdue's `horizontal_kern`, which still
  works for fonts that have a `kern` table. For Inter, layout is unkerned. That
  is acceptable for numeric tick labels. Digits are proportional: at 16 px, "1"
  advances 6.5 px and "0" advances 10.1 px. Tabular figures need GSUB (`tnum`).
- **Pattern**: "Kerning supported" depends on the font as well as the code.
  Check the actual face. Follow-up GUP-405 adds shaping.

#### Lazy font parse

- **Challenge**: fontdue parses every outline when it loads a face: Inter takes
  18 ms in release and 120 ms in debug. S1 made every old-path `GupContext` go
  through `gup_core::Context`, so every process would have paid this, including
  every old-path test binary.
- **Solution**: `Font::inter()` is a process-wide `OnceLock`. `TextSystem::new`
  stores the font in a `OnceLock` and parses Inter only when `font()`, `measure`
  or `layout` first needs it.
- **Pattern**: When a device-scoped cache is created for every context, make it
  cost nothing until it is used.

#### Counting uploads across a crate boundary

- **Challenge**: S0b's contract is that every gup-core GPU write is counted, and
  a source-scanning test enforces it. A leaf crate can't call
  `Context::write_*`.
- **Solution**: `gup_text::Uploader` is a two-method trait (`write_buffer`,
  `write_texture`). It is implemented for `wgpu::Queue`, and gup-core implements
  it with a counting adapter. The dirty atlas rectangle is packed tight before
  upload, so the counted bytes are the bytes copied, not padded rows: 220 B in 2
  writes over a 60-frame zoom, compared with full-width rows before.
- **Pattern**: Let a leaf crate's caller supply its writes, rather than
  returning byte counts after the fact.

### Architectural Decisions

#### Bitmap rasterisation; MSDF not ported

- **Decision**: Keep fontdue coverage bitmaps. `msdf.rs` (3,090 lines) and
  `sdf_tuning.rs` (MSDF-only) are not ported.
- **Reasoning**: Every current caller draws horizontal title and tick-label text
  at fixed sizes. Rasterising at `size × dpr` and drawing 1:1 is sharper than an
  SDF at these sizes and is resolution-correct for any dpr. Porting MSDF would
  have meant its generator, its shader and the tuning profile (over 3,400
  lines), with no caller that needs them. The old `atlas.rs` was MSDF-specific
  too (RGBA, `MsdfGenerator`). Its packing and "atlas full" error were adapted;
  the rest was not.
- **Trade-off**: Text drawn at many sizes (continuous zoom of labels) fills the
  atlas with a bitmap per size, up to `Error::AtlasFull`. Arbitrary rotation
  needs either rotated quads (exact for 90°) or MSDF.
- **Future**: Port MSDF when a caller needs free rotation or scaled text. 90°
  axis titles (S7) can use rotated bitmap quads.

#### What was ported from `src/text/`

- **Decision**: Ported, adapted:
  - `font.rs`: loading from bytes with an error naming the font, the bundled
    face and line metrics.
  - `atlas.rs`: shelf packing with padding and an "atlas full" error, plus
    growth.
  - `layout.rs`: the anchor and measure subset only.

  Not ported:
  - `font.rs`'s fontdb system-font database and multi-atlas manager. No caller
    picks a family.
  - `layout.rs`'s clipping, wrapping, truncation, collision and reposition
    strategies. These are the six `#[expect(too_many_arguments)]` functions from
    GUP-398's retrospective.
  - `style.rs`: `gup_core::scene::TextStyle` and `plot::style` already own size
    and colour, and the old style's weight, rotation and letter spacing have no
    caller.

- **Reasoning**: The story says not to pre-build features without a caller. None
  of the six `too_many_arguments` signatures came across, so there is nothing to
  fix in gup-text. They are deleted with `src/text/` at S14.
- **Trade-off**: When wrapping or clipping is needed, it is written fresh, using
  a request struct rather than 7–10 positional parameters.

#### Glyph batches bind group 0 and carry clip-space quads

- **Decision**: gup-text owns its pipeline (plain WGSL, no naga_oil), with the
  atlas at group 0. `prepare` converts physical-pixel quads to clip space on the
  CPU, so there is no view uniform. gup-core rebinds its view for its own draws.
- **Reasoning**: The crate stays independent of gup-core's `View` layout, and
  the batch works in any host pass. It adds no uniform write per frame.
- **Trade-off**: A resize needs a new `prepare`, which every frame does anyway.
  S3 can still move text onto the shared view uniform if it wants one layout for
  all draws.

#### Inter lives in the crate, not in `assets/fonts/`

- **Decision**: `crates/gup-text/fonts/Inter-Regular.ttf` and `Inter-OFL.txt`.
  `assets/fonts/default.ttf` is unchanged.
- **Reasoning**: This is the narrowest change that keeps the old path's goldens
  green, and the crate is self-contained. It is the only copy of Inter.
  GUP-392's note tells it to reuse this file.
- **Trade-off**: For now the old path still renders Squada One.

### Development Workflow Insights

- **No debugging was needed.** The external pass test (AC5) passed on its first
  run, and gup-core needed no fixes beyond compile errors. Its assertions (dark
  pixels only inside `ink_bounds`, and at least 100 of them) make it a real
  check.
- **The hook test caught a stale expectation.** `test_pre_commit.sh` expected
  `assets/fonts/default.ttf` to drag in gup-core. Once gup-core stopped
  embedding it, the hook's dynamic scoping was right and the test was wrong.
- **Disk.** With `CARGO_TARGET_DIR=/tmp/gup-target` shared, `mask all-check` and
  `mask smoke-examples` took /tmp from 14 GB to 5.6 GB free. Run them last.
- **Display-dependent checks.** `window_parity` and the two gup-core examples
  ran under niri. `zoom_bench`'s frame interval was 1 s because the window was
  not visible (niri throttles frame callbacks), so it was used only as a smoke
  run, not as a measurement.

### Follow-up Stories

1. **GUP-405: gup-text OpenType Shaping (Kerning and Tabular Figures)**: shape
   runs with rustybuzz so Inter's GPOS kerning and `tnum` digits apply. Tick
   labels then align digit-for-digit, and the strategic review's "neutral UI
   face with tabular figures" is fully met.
