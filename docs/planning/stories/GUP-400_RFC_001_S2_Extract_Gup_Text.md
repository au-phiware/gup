# GUP-400: RFC-001 S2 — Extract `gup-text`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-05

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

- [ ] `crates/gup-text` is a new workspace member with zero dependency on `gup`
      or `gup-core` (`cargo tree -p gup-text` lists neither); `gup-core` depends
      on it, not the other way round.
- [ ] Font loading, atlas management and layout/rasterisation logic adapted from
      `src/text/{font,layout,msdf,atlas,sdf_tuning,style}.rs` is moved into
      `gup-text`, with every dependency on old-path types (`GupContext`,
      `RenderContext`, the old `Mark`/`Selection` types) removed. The old
      `src/text/` tree itself has **zero diff** from `main` at the start of this
      story (verified the same way GUP-395's AC1 verified the rest of the frozen
      path) — this is an extraction into a new crate, not a rewrite of the old
      one in place.
- [ ] The default bundled font is Inter if GUP-392 has already landed (shared
      asset, see Context), or stays whatever `assets/fonts/default.ttf`
      currently is if GUP-392 has not yet landed — this story does not perform
      its own separate font swap or add a second licence file.

### AC2: One `TextSystem` per `Context`, with a measuring API

- [ ] `gup_text::TextSystem` owns one glyph atlas and font per device
      (constructed from a `&wgpu::Device`/`&wgpu::Queue`, not from `gup-core`
      types, since `gup-text` cannot depend on `gup-core`). `gup_core::Context`
      holds exactly one `Mutex<gup_text::TextSystem>`, replacing its internal
      `text: Mutex<TextSystem>` from the temporary module
      (`crates/gup-core/src/text.rs`, which is deleted by this story).
- [ ] `TextSystem` exposes a measuring API returning the same shape of
      information (width, cap-height, descent, ink bounds per run) that
      `gup_core::Layout` already surfaces to the GUP-388 visual-regression
      harness adapter. The harness adapter's existing contract with
      `gup_core::Layout` does not change as an observable effect of this story
      (GUP-395's retrospective: "Keep `measure` and the ink bounds the harness
      adapter uses").

### AC3: Glyph-run drawing into an existing pass

- [ ] `TextSystem` (or a value it returns, e.g. a prepared glyph batch) can
      record glyph-quad draw calls into an existing `wgpu::RenderPass` rather
      than opening its own pass or submitting its own command buffer — the "one
      render path" principle (RFC-001 §1, §7) that S3's `Renderer`/ `Prepared`
      abstraction depends on.
- [ ] `gup-core`'s scene encoder is switched from the temporary module's draw
      path to `gup-text`'s pass-drawing API. GUP-395's title and tick-label
      `TextRun`s still render; `cargo test -p gup-core --lib --test scatter_png`
      still passes.

### AC4: Golden output still correct after the extraction (user-visible)

- [ ] **AC (user-visible, required)**: `tests/golden/gup_core/scatter.png` is
      re-blessed if the rasteriser or font changed, and verified by eye: title
      centred above the plot, tick labels on both axes legible, glyphs sharp
      with no garbling, matching (or, if the font changed to Inter, improving
      on) GUP-395's description. A written note (what changed, what was checked)
      goes in the retrospective.
- [ ] If GUP-392 has not yet landed, this story's font stays unchanged from
      GUP-395's golden — re-blessing happens only if the rasteriser (e.g. bitmap
      vs. MSDF, per the Risk Assessment) measurably changes pixels, not merely
      because the code moved crates.

### AC5: `gup-text` is exercised from outside its own crate

- [ ] A test under `crates/gup-text/tests/` (external-crate style, matching
      GUP-395's AC7 pattern) constructs a `TextSystem`, measures a run, and
      records its glyph quads into a throwaway render pass, proving the public
      API is usable without reaching into `gup-text`'s internals.

## Technical Tasks

- [ ] Add `crates/gup-text` as a new workspace member; depend on whatever subset
      of `fontdue`/`ttf-parser`/`fontdb`/`image`/`bytemuck` the ported code
      actually needs (GUP-395 found `fontdb`/`ttf-parser` unnecessary for its
      minimal slice — re-check whether the fuller `gup-text` needs them for
      system-font-adjacent logic in `font.rs`, or whether that stays deferred).
- [ ] Port `font.rs` (loading, fallback, metrics), `atlas.rs`
      (packing/eviction), the load-bearing subset of `layout.rs` (straight-line
      horizontal layout, kerning — defer wrapping/rotation/multi-line unless a
      current caller needs them), `sdf_tuning.rs` and `style.rs`, dropping
      `GupContext`/`RenderContext` coupling.
- [ ] Decide bitmap vs. MSDF rasterisation (`msdf.rs`) for this extraction;
      record the decision and its reasoning in the retrospective, the same way
      GUP-395 recorded the naga_oil go/no-go decision (see Risk Assessment).
- [ ] Design and implement `TextSystem::new`, `TextSystem::measure`, and the
      pass-drawing API.
- [ ] Wire `gup_core::Context` to hold `gup_text::TextSystem`; delete
      `crates/gup-core/src/text.rs`.
- [ ] Update `gup_core::plot`/`scene` resolve and encode code to call the new
      measuring/drawing API; confirm `Layout`'s public shape is unchanged for
      the harness adapter.
- [ ] Re-bless `tests/golden/gup_core/scatter.png` if pixels changed; verify by
      eye per AC4.
- [ ] Write the external-crate integration test for `gup-text` (AC5).
- [ ] Verify the old `gup` crate's `src/text/` tree (and the rest of the frozen
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

- [ ] `gup-text` exists as a dependency-free leaf crate with a public,
      externally-tested API.
- [ ] `gup-core`'s temporary `text.rs` is deleted.
- [ ] The golden PNG remains correct (title, tick labels, legible glyphs),
      re-blessed only if pixels actually changed.
- [ ] `git diff main -- src/text/` (and the rest of the frozen old-path file
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

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-text -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Golden PNG verified by eye and described in the retrospective, with the
      bitmap-vs-MSDF decision recorded
- [ ] No diff against `main` in the old `gup` path (same file list as GUP-395,
      including `src/text/`)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
