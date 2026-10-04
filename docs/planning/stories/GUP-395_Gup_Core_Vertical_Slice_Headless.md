# GUP-395: gup-core Vertical Slice, Headless

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-04

## Context

[RFC-001: Core Architecture](../rfcs/RFC-001_Core_Architecture.md) was accepted
2026-10-04, with the Orchestrator review amendments at the end of the document
binding. The RFC's first implementation step is "S0: Vertical-slice spike in
`gup-core`, scatter only" (§11), which the Orchestrator review split into
**S0a** (headless PNG path, compile-fail suite) and **S0b** (window, `show()`,
the 100K-point zoom benchmark) because the combined S0 was too large for one
story. This story is S0a.

The strategic review found that Gup's core idea — data encodings as composable
GPU functions — has zero production callers, and that output is broken by
default (no text in PNG output, double gamma-encoded colours, elliptical circles
on non-square canvases). RFC-001 answers this with seven decisions: one
`Context`, a GPU column store, typed channels, one scale family with CPU
mirrors, naga_oil-based shader composition, a resolved `Scene`/`RenderTarget`,
and one `Chart` trait. This story proves the first five of those for a single
mark (Circle) and three scales (Linear, Log, Sequential), ending with an
explicit go/no-go decision on naga_oil (RFC §6, §12 risk 1) that gates every
later S-story.

Per the RFC's migration plan (§11) and the Orchestrator review's point 1
("parallel-system risk"), `gup-core` is a new workspace member that the old
`gup` crate does not re-export, and which must not reach into old modules. This
enforces the boundary the project has repeatedly failed to hold (four scale
systems, three composition systems, three pipeline caches, none ever deleted).
GUP-389 (delete unwired dead subsystems) and GUP-390 (quarantine culling/LOD and
exclude non-compiling integration crates from the default workspace) are
prerequisites so that `gup-core` is built against a pruned `main`, not modelled
on code that is about to be deleted — and so that `naga_oil`, which currently
enters `Cargo.lock` only transitively through the parked `gup-bevy`
(Orchestrator review point 5), becomes a direct, explicitly pinned dependency of
`gup-core` instead.

**Text rendering decision.** RFC-001 §11 lists `text/{font,layout,msdf,atlas,…}`
→ `gup-text` as a "Survive (move)" item, but extracting a general-purpose
`gup-text` leaf crate with a stable public API is scoped to **S2**, a dedicated
story. Building that extraction now would pull unrelated API-design work into an
already-large spike. This story instead **ports a minimal, internal-only text
path directly into `gup-core`** — just enough to shape and rasterize the title
and tick-label `TextRun`s this story's exit criteria require: font loading via
the same external crates the old path already uses (`fontdb`, `ttf-parser`,
`fontdue` — these are third-party dependencies, not old Gup code, so depending
on them directly does not violate "no reaching into old modules"), a single
bitmap glyph atlas, and straight-line horizontal layout. Widget-shaped concerns
in the old `src/text/` (`hover_reveal.rs`, `tooltip_bg.rs`, `ui_quad.rs`) and
MSDF (vs. plain bitmap) rendering are explicitly **not** ported — they are not
needed for a title and axis tick labels and are deferred to S2's real
extraction. This module is marked in doc comments as temporary and superseded by
`gup-text` at S2; it must not grow a public API surface that S2 would then have
to deprecate.

## User Story

> "As a Gup implementer validating RFC-001, I want a headless vertical slice in
> `gup-core` that renders a scatter chart with a title and tick labels to a PNG
> using typed channels, a GPU column store, and naga_oil-composed shaders, so
> that the RFC's architecture is proven (or falsified) before any further
> RFC-001 story is written."

## Acceptance Criteria

### AC1: `gup-core` exists as an isolated workspace member

- [ ] `crates/gup-core` is a new workspace member in the root `Cargo.toml`, with
      its own `Cargo.toml` pinning direct dependencies (`wgpu = "27.0"`,
      `naga_oil = "0.20"`, `naga = "27.0.3"`, `encase = "0.12"`, plus
      `fontdb`/`ttf-parser`/`fontdue`/`image`/`bytemuck`/`pollster`), matching
      the versions already resolved in `Cargo.lock` (Appendix evidence table,
      RFC-001 §11).
- [ ] `gup-core` has zero `path` or `use` dependencies on the root `gup` crate
      or any of its modules. `cargo tree -p gup-core` does not list `gup`.
- [ ] The root `gup` crate's `src/` tree (`selection.rs`, `mark/`,
      `shader_function/`, `shader_pipeline.rs`, `chart_builder*`, `context.rs`,
      `render.rs`, and all old-path builders) has **zero diff** from `main` at
      the start of this story — verified with:

  ```sh
  git diff main -- src/selection.rs src/mark/ src/shader_function/ \
    src/shader_pipeline.rs src/chart_builder.rs src/chart_builder/ \
    src/context.rs src/render.rs
  ```

  producing no output. `gup-macros` may be touched only to add a
  `crate = "::gup_core"` path-override call site for `gup-core`'s own macro use
  (the override mechanism itself, in `gup-macros/src/crate_path.rs`, already
  exists and needs no change).

### AC2: `Context` supports headless creation from both constructors

- [ ] `gup_core::Context::new_blocking() -> Result<Context>` creates a `Context`
      owning its own `wgpu::Instance`/`Adapter`/`Device`/`Queue` (RFC §2), works
      headlessly (no window), and is `Clone` (`Arc<Inner>`).
- [ ] `gup_core::Context::from_wgpu(device: wgpu::Device, queue: wgpu::Queue) -> Context`
      builds a `Context` with no new adapter requests, reading `Caps` from
      `device.limits()`/`device.features()`.
- [ ] A unit test creates a `Context` with each constructor and asserts
      `context.device()`/`context.queue()` are usable (e.g. a trivial buffer
      write/read round-trip).

### AC3: Column store, Circle module, and glue emitter compose via naga_oil

- [ ] A single-chunk `ColumnStore` (RFC §3) uploads X/Y/FILL columns as
      instance-rate vertex buffers with `VERTEX | COPY_DST` usage, and supports
      `F32Relative` format with a per-chunk f64 origin for the X column.
- [ ] A hand-written `gup::marks::circle` WGSL module (`#define_import_path`)
      and WGSL modules for `gup::scale::linear`, `gup::scale::log`, and
      `gup::color::sequential` are preloaded into a naga_oil `Composer` owned by
      `Context`.
- [ ] A typed glue emitter generates the top-level entry-point module for the
      signature
      `Circle { x: f32rel→linear, y: f32→log, fill: f32→sequential(lut), radius: const }`,
      matching the structure of RFC-001 §6's worked example. No string
      find-and-replace is performed on any authored WGSL module.
- [ ] The generated WGSL is captured to a file or test fixture and attached to
      this story's retrospective and to RFC-001 (see AC8).

### AC4: Typed channels and scales with CPU mirrors

- [ ] `Channel<Circle, Px>` constants exist for `X`, `Y`, `RADIUS`, `FILL`
      (`FILL: Channel<Circle, Color>`), defined as in RFC §4 (hand-written for
      this spike; the `#[derive(Mark)]` generator is S6's job).
- [ ] `Linear`, `Log`, and `Sequential` scale types implement `ShaderFn` +
      `CpuMirror` (RFC §5): each has a WGSL `Params` struct via `encase`, a
      `fn eval(&self, x: f64) -> …` CPU mirror, and `input_format()` returning
      `F32Relative` (Linear) or `F32` (Log).
- [ ] A conformance test dispatches each scale's WGSL in a compute pass over
      1,000 sampled inputs and asserts the GPU result matches `CpuMirror::eval`
      within 0.25 px (position scales) or 1/255 (colour), per RFC §5.

### AC5: Scene, text, and PNG output are user-visible and correct

- [ ] A `Scene` (subset of RFC §7: `Marks`, `Rules` for axis lines/ticks, `Text`
      for `TextRun`s) is built from a resolved scatter layout: plot rect, two
      axis rules, at least 4 tick labels per axis, and one title `TextRun`.
- [ ] `ImageTarget` renders the `Scene` to an offscreen texture and reads it
      back (async) to an `RgbaImage`, written to PNG.
- [ ] **AC (user-visible, required)**: the resulting PNG is verified by eye —
      using the GUP-388 visual regression harness if it has landed by
      implementation time, or by reading the PNG file directly with an image
      viewer/tool otherwise. The image must show: a title string at the top, at
      least 4 numeric tick labels on each axis, circle marks entirely inside the
      plot rect, and no placeholder/blank/garbled text. A written note
      (screenshot path + description of what was checked) goes in the
      retrospective.

### AC6: Compile-fail suite covers all three documented error cases

- [ ] `trybuild` compile-fail tests (run from `gup-core`'s test suite) cover,
      per RFC §4 and §12 risk 11: 1. **Wrong value type**:
      `attr(Circle::RADIUS, some_sequential_scale.encode(..))` fails to compile
      because `Sequential`'s `Out` is `Color`, not `Px`. 2. **Unknown channel**:
      a reference to a channel name that doesn't exist (e.g. a typo'd const)
      fails with "no associated item", not a runtime error. 3. **Wrong mark**: a
      `Channel<OtherMark, _>` (a minimal dummy mark defined solely for this
      test, since only `Circle` exists in this story's scope) does not unify
      with `Selection<_, Circle>::attr`.
- [ ] Each `.stderr` snapshot is checked into the repo and reviewed for message
      quality (RFC §12 risk 11); if `#[diagnostic::on_unimplemented]` requires
      nightly, document that trade-off rather than blocking on it.

### AC7: gup-core is exercised from outside its own crate

- [ ] A test under `crates/gup-core/tests/` (an external-crate-style integration
      test, not a `#[cfg(test)] mod tests` inside `src/`) builds the scatter
      scene end-to-end and asserts the PNG is produced and non-trivial
      (dimensions, non-uniform pixel content) — this is in addition to, not
      instead of, the by-eye check in AC5.
- [ ] A north-star-equivalent snippet compiles as a doctest on a public
      `gup-core` item. Since the high-level `gup::scatter()` builder is T5/S10
      work far outside this story's scope, the doctest uses the low-level API
      available in `gup-core` today (RFC §10(c)-shaped: `Selection<T, Circle>`
      with `.attr(Circle::X, …)`/`.attr(Circle::Y, …)`/`.attr(Circle::FILL, …)`
      against `Linear`/`Log`/`Sequential` scales). Record in the retrospective
      whether RFC §12 risk 4 (borrowed-`&str` closures not inferring) was hit;
      this story's scope only needs `Sequential` (a numeric scale), so risk 4
      may not actually arise here — confirm either way rather than assuming.

### AC8: naga_oil go/no-go gate recorded in RFC-001

- [ ] Generated WGSL (AC3) and wall-clock timings for naga_oil compose +
      `wgpu::Device::create_render_pipeline` for the
      Circle/Linear/Log/Sequential signature are measured (debug and release)
      and appended to RFC-001, compared against the §12 risk 2 budget (≤30 ms
      per pipeline on desktop).
- [ ] RFC-001 is amended with a new dated subsection recording the **go/no-go
      decision on naga_oil** (continue, per §6's recommendation, or fall back to
      the import-only concatenation path described in §6's "Fallback" bullet),
      with the evidence that justifies it.

## Technical Tasks

- [ ] Add `crates/gup-core` to the workspace; set up its `Cargo.toml` with
      pinned versions matching `Cargo.lock`'s existing
      `naga`/`naga_oil`/`encase` resolution (currently reached only via the
      now-parked `gup-bevy`; make it a direct dependency per Orchestrator review
      point 5).
- [ ] Implement `Context` (`new_blocking`, `from_wgpu`, `Caps` from device
      limits/features, `Mutex<Composer>` for naga_oil, `Mutex<PipelineCache>`).
- [ ] Implement a single-chunk `ColumnStore` with `F32Relative`/`F32` column
      formats and per-chunk f64 origin bookkeeping for the X column.
- [ ] Write WGSL library modules: `gup::marks::circle`, `gup::scale::linear`,
      `gup::scale::log`, `gup::color::sequential` (with a LUT texture), and
      `gup::view` (px→clip), each with `#define_import_path`.
- [ ] Implement the typed glue emitter producing the entry-point module for the
      one required `(Circle, {x,y,fill})` encoding signature, matching RFC §6's
      worked example structure.
- [ ] Define `Channel<Circle, V>` consts (`X`, `Y`, `RADIUS`, `FILL`) and the
      `IntoEncoding`/`ShaderFn`/`CpuMirror` traits needed to type-check `attr`
      calls (hand-written; no derive macro yet).
- [ ] Implement `Linear`, `Log`, `Sequential` scale types: `Params` via
      `encase`, WGSL body, and f64 `CpuMirror::eval`.
- [ ] Port a minimal internal text path into `gup_core::text` (font loading via
      `fontdb`/`ttf-parser`, glyph rasterization via `fontdue`, one bitmap
      atlas, straight-line horizontal layout) — explicitly scoped to title +
      tick-label rendering, marked as temporary/internal-only pending S2's
      `gup-text` extraction.
- [ ] Implement `Scene` (subset: `Marks`, `Rules`, `Text`), a minimal resolve
      step producing axis rules + tick `TextRun`s + a title `TextRun` from a
      hard-coded or simple domain/range, and `ImageTarget` → PNG via async
      readback and the `image` crate.
- [ ] Write the `trybuild` compile-fail suite (3 cases) with checked-in
      `.stderr` snapshots.
- [ ] Write the external-crate integration test (`crates/gup-core/tests/`) and
      the north-star-equivalent doctest.
- [ ] Measure and record WGSL generation + pipeline-compile timings; append the
      naga_oil go/no-go subsection to RFC-001.
- [ ] Verify the old `gup` crate's `src/` tree has no diff from `main` (AC1).

## Dependencies

### Prerequisite Stories

- GUP-389: Delete Unwired Dead Subsystems 📋 — prunes transpiler, Mixable,
  MarkRenderer and the broken `plot_api` path before `gup-core` is designed, so
  it doesn't model itself on code scheduled for deletion.
- GUP-390: Quarantine Culling/LOD and Integration Crates 📋 — excludes
  `gup-bevy` from the default workspace, which currently supplies `naga_oil`/
  `naga`/`encase` to `Cargo.lock` only transitively; this story makes `gup-core`
  depend on them directly instead.
- GUP-388: Visual Regression Harness ✅ (soft dependency) — preferred for AC5's
  by-eye PNG verification; if not yet landed, AC5 is satisfied by directly
  reading the generated PNG and recording the finding.

### Enables Stories

- GUP-396: gup-core Vertical Slice, Window and Performance — needs this story's
  `Context`, `ColumnStore`, Circle glue, scales, and `Scene`/PNG path before
  adding `WindowTarget` and the zoom benchmark.
- RFC-001 S1–S13 (not yet written as stories) — all depend on this story's
  naga_oil go/no-go decision and the proven column-store/channel/scale
  architecture.

## Testing Strategy

- **Unit tests**: `Context` construction via both paths; `ColumnStore` chunk
  upload and relative-origin math; each scale's `CpuMirror::eval` against known
  values.
- **Conformance tests**: GPU-vs-CPU scale dispatch within tolerance (AC4).
- **Integration tests**: external-crate-style test in `crates/gup-core/tests/`
  producing and asserting on the PNG (AC7).
- **Compile-fail tests**: `trybuild` suite with checked-in `.stderr` snapshots
  (AC6).
- **Visual validation**: the PNG is read by eye and described in the
  retrospective (AC5) — this is the hard requirement the strategic review found
  consistently skipped on the old path.
- **Performance**: WGSL generation + pipeline-compile wall time recorded for the
  go/no-go gate (AC8); no fps/throughput benchmark in this story (that's
  GUP-396).

## Success Metrics

- [ ] The golden/by-eye-verified PNG shows a title, tick labels on both axes,
      and circle marks inside the plot rect.
- [ ] All three `trybuild` compile-fail cases produce the expected diagnostics.
- [ ] The north-star-equivalent doctest compiles and runs.
- [ ] Pipeline-compile timing is recorded and compared against the ≤30 ms
      budget; the naga_oil go/no-go decision is written into RFC-001.
- [ ] `git diff main -- <old-path files>` is empty.

## Risk Assessment

- **High**: naga*oil 0.20 may not integrate cleanly with wgpu 27/naga 27.0.3
  once it is a direct dependency instead of reaching the lockfile transitively
  through `gup-bevy`. \_Mitigation*: this is exactly what AC8's go/no-go gate is
  for — if it doesn't work, the fallback (import-only WGSL concatenation, RFC
  §6) is already documented and this story's exit criteria explicitly allow a
  "no-go" outcome, which still unblocks later stories by ruling out an approach.
- **Medium**: this story still touches six new subsystems (context, columns,
  shader composition, channels, scales, scene/text/PNG) in one vertical slice.
  _Mitigation_: build only the minimal sliver of each needed to prove the
  architecture for one mark and three scales — resist generalizing to the full
  systems that S1–S9 build later.
- **Medium**: the temporary internal text port may diverge from the eventual
  `gup-text` (S2) design, requiring rework. _Mitigation_: keep it small, give it
  no public API beyond what `Scene` resolution needs internally, and mark it
  clearly as temporary in doc comments.
- **Low**: `#[diagnostic::on_unimplemented]` (RFC §12 risk 11) may require
  nightly Rust, which could conflict with the project's toolchain policy.
  _Mitigation_: AC6 explicitly allows documenting this trade-off rather than
  blocking the story on it; message-quality polish can land as a follow-up.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples` (no new examples are
      expected from this story, but the check must stay green)
- [ ] Rendered PNG verified by eye and described in the retrospective
- [ ] RFC-001 updated with the naga_oil go/no-go subsection and timing evidence
- [ ] No diff against `main` in the old `gup` path (`src/selection.rs`,
      `src/mark/`, `src/shader_function/`, `src/shader_pipeline.rs`,
      `src/chart_builder*`, `src/context.rs`, `src/render.rs`)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
