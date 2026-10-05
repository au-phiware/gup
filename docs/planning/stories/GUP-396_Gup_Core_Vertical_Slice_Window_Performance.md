# GUP-396: gup-core Vertical Slice, Window and Performance

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-04

## Context

This is the second half of [RFC-001](../rfcs/RFC-001_Core_Architecture.md)'s S0
vertical-slice spike, split by the Orchestrator review (RFC-001, "Orchestrator
review" §3) into S0a (headless PNG, compile-fail suite — GUP-395) and S0b
(window, `show()`, the 100K-point zoom benchmark). GUP-395 proves the core
architecture — `Context`, the GPU column store, typed channels, scales, and the
`Scene`/`ImageTarget` PNG path — for a single Circle mark with Linear, Log and
Sequential scales. This story adds the second `RenderTarget` (`WindowTarget`)
and proves the two RFC-001 goals that only a live, interactive render path can
demonstrate: visual parity between window and PNG output, and the zoom
performance target with zero column re-uploads.

The strategic review's dogfooding audit found that a chart built with the old
builder path cannot be shown in a window at all, because `RenderContext` (used
by charts) and `GupContext` (used by `GupApp`) are separate, incompatible
device-owning types (RFC-001 §2 "Today"). RFC-001's single `Context` is designed
specifically to remove that split. This story is the first point in the
migration where that claim becomes checkable end-to-end: the same `Scene` that
produced GUP-395's golden PNG must also render correctly in a live window.

The RFC's performance goal (§1 Goals) is "100K points at 60 fps at 1080p … with
x linear, y log, colour sequential and size sqrt" and "zooming or panning writes
0 column bytes: uniforms only." This story's benchmark is scoped to the three
scales GUP-395 already implements (Linear x, Log y, Sequential fill); a `Sqrt`
size scale is not in S0a's scope and is not required here — the fps and
zero-byte-write claims are validated against the encodings that exist, and a
note is left for whichever later story (S5, "full scale family") adds `Sqrt` to
also re-check this benchmark with a size channel bound.

**Inputs from GUP-395** (see RFC-001 "S0a findings"):

- Zoom today means replacing the scale through its handle
  (`*x.write() = Linear::new().domain(..)`). Add `PositionScale::set_domain`.
- `ColumnStore::upload` is the only place column bytes are written, so the
  zero-byte counter belongs there.
- `Plot::resolve` re-creates the uniform buffers and bind groups on every call,
  and lays out text again. That is fine for a PNG, but at 60 fps it should write
  into the existing buffers with `queue.write_buffer`. A test already shows that
  a rescale reuses the program, the pipeline and the column buffer.
- `encode_scene` records the whole scene into one render pass on a caller's
  encoder, so `WindowTarget` only needs to supply the surface view. Render into
  a non-sRGB view (`Bgra8Unorm` on a `Bgra8UnormSrgb` surface) to match
  `ImageTarget`'s `Rgba8Unorm` for the ΔE parity check.

## User Story

> "As a Gup implementer validating RFC-001, I want the GUP-395 scatter scene to
> also render in a live window with pixel-level parity to its PNG, and to
> sustain 60 fps while zooming 100K points without re-uploading column data, so
> that RFC-001's `Context`/`RenderTarget` unification and zero-byte-zoom claims
> are proven before any further RFC-001 story is written."

## Acceptance Criteria

### AC1: `WindowTarget` renders the same `Scene` as `ImageTarget`

- [ ] `gup_core::WindowTarget` implements the `RenderTarget` trait from RFC-001
      §7, wrapping a `winit` surface created from an `Arc<Window>` (per this
      project's established `Arc<Window>` surface-lifetime pattern).
- [ ] The same `Scene`-building code used by GUP-395's PNG path (not a duplicate
      or re-derived scene) is drawn into both `ImageTarget` and `WindowTarget`
      by the same `Renderer`/`Prepared` draw call, proving the "one render path"
      claim (RFC-001 §1 Goals, §7).

### AC2: A minimal `gup::show()` displays a gup-core chart

- [ ] A minimal `gup_core::show(chart)` function (RFC-001 §8's `ChartExt`-style
      entry point, scoped down to what this spike's `Chart`-like scatter scene
      needs — a full `Chart` trait implementation is S7/S8's job) opens a
      window, creates a `Context` and `WindowTarget`, and redraws the GUP-395
      scatter scene on resize and on a wheel-zoom input.
- [ ] Running the resulting minimal example headlessly via `GUP_SCREENSHOT_PATH`
      (the project's existing headless-screenshot mechanism) produces a
      window-rendered screenshot.

### AC3: Window and PNG outputs are visually equivalent (user-visible)

- [ ] **AC (user-visible, required)**: the `WindowTarget` screenshot (via
      `GUP_SCREENSHOT_PATH`) and the GUP-395 `ImageTarget` PNG are compared
      pixel-by-pixel (same scene, same dimensions, same dpr) and the maximum
      per-pixel colour difference is **ΔE < 2** (CIE76 or CIE2000, document
      which). Use the GUP-388 visual regression harness's comparison utility if
      it has landed by implementation time; otherwise compute ΔE directly (e.g.
      with the `image`/`palette` crates) and record the computed value — "looks
      the same" is not sufficient, a number is required.
- [ ] A written note in the retrospective states the measured ΔE and includes
      both images' paths.

### AC4: 100K-point zoom sustains ≥60 fps with 0 column bytes written

- [ ] A benchmark (criterion, or a dedicated instrumented binary/test) renders
      100K points (Linear x, Log y, Sequential fill, constant radius — the
      encodings GUP-395 implements) in a `WindowTarget` or an equivalent
      off-screen-but-presenting loop, and simulates a continuous zoom (domain
      changes on every frame) for at least 300 frames.
- [ ] Frame time is measured (e.g. via `gpu_timer.rs`'s pattern, survived per
      RFC-001 §11's migration table, or a CPU-side `Instant` wrapper) and the
      median and p95 fps are recorded; the median must be **≥60 fps**.
- [ ] A **buffer-write counter** — a thin wrapper or instrumented path around
      `queue.write_buffer` calls targeting column sub-ranges — proves **0 column
      bytes** are written during the zoom loop. Only the `Encodings`/view
      uniform buffers may be written per frame. The counter's value (0) and the
      measurement method are recorded in the retrospective.
- [ ] If the ≥60 fps target is not met on the development machine's GPU, record
      the actual number, the GPU used, and whether the shortfall is in CPU
      submission overhead or GPU time (e.g. via the timer), rather than silently
      lowering the bar.

## Technical Tasks

- [ ] Implement `WindowTarget` (`RenderTarget` impl: `desc`, `acquire`,
      `present`) over a `winit` surface held via `Arc<Window>`.
- [ ] Factor GUP-395's scene-building code so it is shared (not duplicated)
      between the `ImageTarget` and `WindowTarget` call sites.
- [ ] Implement a minimal `gup_core::show(..)` that opens a window, builds a
      `Context::new_blocking()` (or reuses one), creates a `WindowTarget`, and
      drives a redraw loop with resize and wheel-zoom input handling that
      mutates the Linear/Log scale domains only (no column re-upload).
- [ ] Wire the existing `GUP_SCREENSHOT_PATH` headless-screenshot mechanism into
      the new minimal example/binary used for this story's window tests.
- [ ] Write the window/PNG ΔE comparison (reuse GUP-388's harness if landed;
      otherwise a small standalone comparison using the `image` crate plus a
      CIE76/CIE2000 ΔE implementation — check if one is already a transitive
      dependency, e.g. via `palette`, before adding a new one).
- [ ] Implement the buffer-write counter: wrap or instrument the `ColumnStore`'s
      `queue.write_buffer` call sites so a test can assert zero column-range
      writes occurred during a domain-only update.
- [ ] Write the 100K-point zoom benchmark, generating synthetic data once and
      reusing the GUP-395 column upload path, then looping a simulated zoom
      (domain shrink/grow) for ≥300 frames while recording frame time and the
      buffer-write counter.
- [ ] Record fps results, ΔE measurement, and the zero-byte-write proof in the
      story retrospective.
- [ ] Verify the old `gup` crate's `src/` tree still has zero diff from `main`
      (carried over from GUP-395's AC1, re-checked here since this story adds
      more code).

## Dependencies

### Prerequisite Stories

- GUP-395: gup-core Vertical Slice, Headless 📋 — provides `Context`,
  `ColumnStore`, the Circle/Linear/Log/Sequential glue, typed channels, and the
  `Scene`/`ImageTarget` PNG path this story extends with a second
  `RenderTarget`.

### Enables Stories

- RFC-001 S1–S13 (not yet written as stories) — the window/PNG parity and
  zero-byte-zoom proofs here are exit criteria for RFC-001's full S0 gate; later
  stories (S3 `Scene`/`RenderTarget`, S4 column store v1, S8 `GupApp`) build on
  both GUP-395 and this story.

## Testing Strategy

- **Unit tests**: `WindowTarget` construction and `acquire`/`present` round trip
  against a headless/offscreen winit surface where supported.
- **Integration tests**: the shared scene-building path is exercised by both
  `ImageTarget` and `WindowTarget` in the same test module, asserting both
  produce non-trivial images.
- **Visual validation**: ΔE comparison between window screenshot and GUP-395's
  PNG (AC3); both images attached/referenced in the retrospective.
- **Performance**: the 100K-point zoom benchmark (AC4) records median/p95 fps
  and the buffer-write counter value; results are recorded, not just checked
  against a pass/fail threshold, so future stories can compare.

## Success Metrics

- [ ] Window screenshot and PNG differ by ΔE < 2, with the measured value
      recorded.
- [ ] 100K-point zoom sustains ≥60 fps median over ≥300 frames, with the
      buffer-write counter at 0 column bytes for the entire run.
- [ ] `gup_core::show(..)` runs headlessly via `GUP_SCREENSHOT_PATH`.
- [ ] `git diff main -- <old-path files>` remains empty.

## Risk Assessment

- **Medium**: achieving ≥60 fps depends on the development machine's GPU and may
  not hold on lower-end integrated GPUs (RFC-001's target is "Iris Xe class").
  _Mitigation_: AC4 requires recording the actual number and GPU used even on
  shortfall, rather than adjusting the target after the fact; a shortfall is
  itself useful evidence for the naga_oil/pipeline-variant risk (RFC-001 §12
  risk 2).
- **Medium**: `WindowTarget`'s headless screenshot path (`GUP_SCREENSHOT_PATH`)
  must work the same way it does for the existing windowed examples; if the
  mechanism assumes details of the old `GupApp` event loop, it may need adapting
  for `gup-core`'s minimal `show()`. _Mitigation_: read the existing
  `GUP_SCREENSHOT_PATH` implementation (`src/export/gallery.rs`) before building
  the new path, and keep the adaptation minimal rather than generalizing early.
- **Low**: no existing ΔE/colour-distance crate may be in the dependency tree,
  requiring either a small new dependency or a hand-rolled CIE76 implementation
  (a handful of lines). _Mitigation_: CIE76 is simple enough to hand-roll if no
  suitable crate is already present; do not add a heavyweight colour-science
  dependency just for this one comparison.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Window/PNG ΔE and the 100K-point zoom fps + zero-byte-write results
      verified by eye/measurement and recorded in the retrospective
- [ ] No diff against `main` in the old `gup` path (same file list as GUP-395)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
