---
implementer: sonnet
implementer-reason:
  keeps the crate's existing render-then-copy architecture and swaps its
  internals for gup-core; the CI placement is a measured decision rule
---

# GUP-484: RFC-001 Step S13b: `gup-bevy` on `gup-core`, Back in the Workspace

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-19

## Context

`gup-bevy` is parked like `gup-egui` (strategic review Decision 3, GUP-390)
until the single render path exists, which it now does (see GUP-483's context).
RFC-001 S13 re-wires both crates through `Context::from_wgpu` and gup-core's
targets and puts them back into the workspace and CI.

The crate's architecture is sound and survives: `GupPlugin::finish` takes Bevy's
`RenderDevice`/`RenderQueue`, a `PostUpdate` system renders each `GupChart` into
its own texture on that device, and a render-world system copies the texture
into the `GpuImage` a sprite shows (GPU to GPU, no readback). What changes is
everything Gup-side: `DynChart` over the old `ComposedChart`, `GupRenderContext`
over the old `GupContext`/`RenderContext`, and the old texture target.
`DynChart` is one of RFC-001 S11's named deletions.

Bevy is a large dependency. Whether it can join the local gate or only CI is
measured here (AC1) with a decision rule, rather than guessed.

## Read First

- [GUP-483](GUP-483_RFC_001_S13a_Gup_Egui_On_Gup_Core.md) "Decisions" (texture
  format, `render_into`, context sharing).
- RFC-001 "S1 findings" "S13 (hosts)" (line 1446): never compare devices with
  `==`; compare `ContextId`s.
- `gup-bevy/src/*.rs` (whole, 690 lines), `gup-bevy/examples/bevy_scatter.rs`,
  `gup-bevy/tests/integration.rs`, `docs/BEVY_INTEGRATION.md`.
- `crates/gup-core/src/target.rs` lines 465–560 (`TextureTarget`).
- Root `Cargo.toml` `[workspace]`, `maskfile.md` `lint`, `lint-check` and
  `all-check` (lines 194–310), `.github/workflows/tests.yml`,
  `scripts/clippy_wasm32.sh`.

## User Story

> "As a Bevy developer, I want a Gup chart as a sprite texture in my scene,
> rendered on Bevy's GPU device and updated as my data changes."

## Acceptance Criteria

### AC1: Spike: what bevy costs the gate

- [ ] Before changing CI, add `gup-bevy` as a workspace member and measure on
      the dev machine: cold `cargo clippy -p gup-bevy --all-targets` time and
      build-directory growth (`du -s` before and after). Record both.
- [ ] Apply the decision rule in Decisions and record which branch was taken.

### AC2: The crate on gup-core

- [ ] `gup-bevy/Cargo.toml` depends on `gup-core` (path), not `gup`. `DynChart`,
      `GupRenderContext` and the old `ChartTextureTarget` are deleted.
- [ ] The public API is Decisions' `GupPlugin`, `GupChart` and `GupContext`,
      with rustdoc and a `no_run` quick-start doctest.
- [ ] A test renders a `gup_core::scatter` through the crate's render function
      (`render_into`, as GUP-483) into its texture, reads it back, and finds it
      byte-identical to `ChartExt::render` at dpr 1.

### AC3: The example

- [ ] `gup-bevy/examples/bevy_scatter.rs` (at most 80 lines): a sprite showing a
      `gup_core::scatter` titled "Bevy scatter", whose rows an `Appender`
      extends every frame. With `GUP_SCREENSHOT_PATH` set it takes a Bevy
      screenshot after 30 frames, saves it and exits.
- [ ] Run under `xvfb-run` and read the screenshot: the chart's title, tick
      labels and points on the sprite.
- [ ] Colour parity: three mark-centre pixels (from the chart's `Layout`, mapped
      to the sprite's screen position) are within ΔE 2 of `ChartExt::render` of
      the same chart.

### AC4: Workspace, CI and docs

- [ ] `gup-bevy` is a workspace member (not default) and out of `exclude`, and
      is linted and tested in CI per the AC1 branch.
- [ ] `scripts/clippy_wasm32.sh` lists it in `excluded` ("a native host
      integration").
- [ ] `gup-bevy/README.md` and `docs/BEVY_INTEGRATION.md` describe the new API;
      the root README's parked table drops `gup-bevy`. If both GUP-483 and this
      story are done, the table lists only the T7 crates.

## Decisions

- **API**:

  ```rust
  /// Adds chart rendering on Bevy's device.
  pub struct GupPlugin;
  /// The `gup_core::Context` over Bevy's render device (a main-world
  /// resource, made in `GupPlugin::finish` with `Context::from_wgpu`).
  #[derive(Resource, Clone)]
  pub struct GupContext(pub gup_core::Context);
  /// A chart drawn into `image` at `size` logical pixels and `dpr`.
  #[derive(Component)]
  pub struct GupChart {
      pub chart: Box<dyn gup_core::Chart + Send + Sync>,
      pub size: UVec2,
      pub dpr: f32,
      pub image: Handle<Image>,
  }
  impl GupChart {
      pub fn new(chart: impl gup_core::Chart + Send + Sync, size: UVec2,
                 image: Handle<Image>) -> Self;   // dpr 1.0
  }
  ```

  Users reach the chart through `chart.as_any_mut().downcast_mut::<C>()` (the
  `Chart` trait already has it).

- **Render flow** (kept): a `PostUpdate` system resolves and renders each
  `GupChart` into its own texture (`Rgba8UnormSrgb`,
  `view_formats: [Rgba8Unorm]`,
  `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`) through `TextureTarget` and
  `Renderer::render` every frame (a clean resolve writes nothing); extraction
  clones the `wgpu::Texture` and image handle into the render world; a `Render`
  system in `RenderSystems::Queue` copies it into the image's `GpuImage`
  (`copy_texture_to_texture`). The image is created by the user as
  `Rgba8UnormSrgb` at `size × dpr`. Reason: the copy already works and needs no
  render-graph node.
- **Device**: `GupContext` wraps
  `Context::from_wgpu(render_device.wgpu_device().clone(), queue)`, where
  `queue` is the inner `wgpu::Queue` of `RenderQueue`, as `plugin.rs` does
  today. One `wgpu` in `Cargo.lock` (Bevy 0.18 uses wgpu 27, as gup-core).
- **CI decision rule (AC1)**: if cold clippy takes at most 90 s and the build
  directory grows by at most 1.5 GB, `gup-bevy` is an ordinary member, covered
  by `mask all-check` and `cargo test --workspace` like everything else.
  Otherwise, every `--workspace` cargo invocation in `maskfile.md` (`lint`,
  `lint-check`, `all-check`) and `.github/workflows/tests.yml` gains
  `--exclude gup-bevy`, and `tests.yml` gains an `integrations` job running
  `cargo clippy -p gup-bevy --all-targets -- -D warnings` and
  `xvfb-run -a cargo test -p gup-bevy -- --test-threads=1`. This story is
  authorised to make exactly these edits to the maskfile, `tests.yml`, the root
  `Cargo.toml` workspace lists and `clippy_wasm32.sh`.
- **System libraries**: if the dev shell lacks a library Bevy's `x11` feature
  links, add it to `flake.nix`'s dev shell packages.

## Escalation Triggers

- `Plot` or the builders are not `Send + Sync`, so `GupChart` cannot hold them.
- Colour parity (AC3) fails by more than ΔE 2.
- Adding the member changes the resolved `wgpu` in `Cargo.lock`.
- Bevy needs more than dev-shell packages (for example a different toolchain or
  a CI runner image change).

## Out of Scope

- `gup-egui`: GUP-483.
- Drawing into Bevy's own pass (`Prepared::draw` in a render-graph node), Bevy
  input forwarding to `Chart::handle`: follow-ups if wanted.
- Mobile and Tauri: T7.

## Technical Tasks

- [ ] AC1 spike and branch.
- [ ] Delete old modules; `GupContext`, `GupChart`, systems on gup-core;
      `render_into` and its test.
- [ ] Example, screenshot, parity check.
- [ ] Workspace and CI per the branch; READMEs and docs.

## Dependencies

### Prerequisite Stories

- GUP-480: RFC-001 S12a Appender Handle 📋 — the example's live data.
- GUP-483: RFC-001 S13a gup-egui 📋 (soft) — `render_into` and the texture
  format to mirror.

### Enables Stories

- GUP-488: S14c — no workspace crate depends on the old path.

## Testing Strategy

- **Integration tests**: `render_into` byte parity.
- **Visual validation**: the example screenshot and ΔE parity.
- **CI**: the gate per AC1's branch.

## Success Metrics

- [ ] `gup-bevy` builds and is tested on every push.
- [ ] No CPU readback in the Bevy path.

## Risk Assessment

- **Medium**: Bevy's compile time and disk (AC1 decides). The shared build
  directory has 11–30 GB free; record the growth.
- **Medium**: pipelined rendering: frame N's copy runs while the main world
  renders frame N+1 into the same texture. _Mitigation_: copy commands and Gup's
  submit are ordered on one queue; a frame may show the newer chart, never a
  torn one. Record what the example shows.
- **Low**: trybuild `.stderr` churn: none expected (no gup-core edits).

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`,
      `cargo test -p gup-bevy -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples -p gup-bevy`
- [ ] `mask wasm-browser` passes (CI and workspace changes must leave the
      browser job green)
- [ ] `mask wasm-size` run and the figures reported in the Implementation
      Summary (expected unchanged; the orchestrator re-records)
- [ ] Rendered output verified by eye: the example screenshot
- [ ] `mask old-path-loc` recorded (expected unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
