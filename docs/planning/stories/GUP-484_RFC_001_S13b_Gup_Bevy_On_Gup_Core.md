---
implementer: sonnet
implementer-reason:
  keeps the crate's existing render-then-copy architecture and swaps its
  internals for gup-core; the CI placement is a measured decision rule
---

# GUP-484: RFC-001 Step S13b: `gup-bevy` on `gup-core`, Back in the Workspace

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-23)
**Created**: 2026-10-19

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

- [x] Before changing CI, add `gup-bevy` as a workspace member and measure on
      the dev machine: cold `cargo clippy -p gup-bevy --all-targets` time and
      build-directory growth (`du -s` before and after). Record both.
- [x] Apply the decision rule in Decisions and record which branch was taken.

### AC2: The crate on gup-core

- [x] `gup-bevy/Cargo.toml` depends on `gup-core` (path), not `gup`. `DynChart`,
      `GupRenderContext` and the old `ChartTextureTarget` are deleted.
- [x] The public API is Decisions' `GupPlugin`, `GupChart` and `GupContext`,
      with rustdoc and a `no_run` quick-start doctest.
- [x] A test renders a `gup_core::scatter` through the crate's render function
      (`render_into`, as GUP-483) into its texture, reads it back, and finds it
      byte-identical to `ChartExt::render` at dpr 1.

### AC3: The example

- [x] `gup-bevy/examples/bevy_scatter.rs` (at most 80 lines): a sprite showing a
      `gup_core::scatter` titled "Bevy scatter", whose rows an `Appender`
      extends every frame. With `GUP_SCREENSHOT_PATH` set it takes a Bevy
      screenshot after 30 frames, saves it and exits.
- [x] Run under `xvfb-run` and read the screenshot: the chart's title, tick
      labels and points on the sprite.
- [x] Colour parity: three mark-centre pixels (from the chart's `Layout`, mapped
      to the sprite's screen position) are within ΔE 2 of `ChartExt::render` of
      the same chart.

### AC4: Workspace, CI and docs

- [x] `gup-bevy` is a workspace member (not default) and out of `exclude`, and
      is linted and tested in CI per the AC1 branch.
- [x] `scripts/clippy_wasm32.sh` lists it in `excluded` ("a native host
      integration").
- [x] `gup-bevy/README.md` and `docs/BEVY_INTEGRATION.md` describe the new API;
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

- [x] AC1 spike and branch.
- [x] Delete old modules; `GupContext`, `GupChart`, systems on gup-core;
      `render_into` and its test.
- [x] Example, screenshot, parity check.
- [x] Workspace and CI per the branch; READMEs and docs.

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

- [x] `gup-bevy` builds and is tested on every push.
- [x] No CPU readback in the Bevy path.

## Risk Assessment

- **Medium**: Bevy's compile time and disk (AC1 decides). The shared build
  directory has 11–30 GB free; record the growth.
- **Medium**: pipelined rendering: frame N's copy runs while the main world
  renders frame N+1 into the same texture. _Mitigation_: copy commands and Gup's
  submit are ordered on one queue; a frame may show the newer chart, never a
  torn one. Record what the example shows.
- **Low**: trybuild `.stderr` churn: none expected (no gup-core edits).

## Definition of Done

- [x] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`,
      `cargo test -p gup-bevy -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples -p gup-bevy`
- [x] `mask wasm-browser` passes (CI and workspace changes must leave the
      browser job green)
- [x] `mask wasm-size` run and the figures reported in the Implementation
      Summary (expected unchanged; the orchestrator re-records)
- [x] Rendered output verified by eye: the example screenshot
- [x] `mask old-path-loc` recorded (expected unchanged)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Resolved (2026-10-23)

Escalation Trigger 1 fired: `ScatterChart`'s pending-op closures were `Send` but
not `Sync` (`builder/scatter.rs`), so it could not be a Bevy component.
**Decision (orchestrator)**: option 1, as a class fix. `Send + Sync` added to
scatter's `Op` and its `op` bound; line and bar already had it. A
`builders_are_send_and_sync` test in `builder/mod.rs` asserts `Plot`,
`ScatterChart`, `LineChart` and `BarChart` are `Send + Sync`. gup-core's tests,
doctests and compile-fail suites pass unchanged.

## Implementation Summary

`gup-bevy` is rewritten on `gup-core` (`src/{lib,chart,plugin,copy,render}.rs`,
about 330 lines; `DynChart`, `GupRenderContext` and `ChartTextureTarget`
deleted) and is a workspace member (not default). `ScatterChart` is now
`Send + Sync` (see Resolved). Tests: `render_parity` (2, byte-identical to
`ChartExt::render` at dpr 1 and to `ImageTarget` at dpr 2), `screenshot_parity`
(1), 2 doctests, and gup-core's `builders_are_send_and_sync`.

**AC1 measurements** (private empty build directory, dev machine):

| Measure                                           | Value                    |
| ------------------------------------------------- | ------------------------ |
| cold `clippy -p gup-bevy --all-targets`           | 98 s (over the 90 s cap) |
| build-directory size after it                     | 0.42 GB (under 1.5 GB)   |
| same after `clippy -p gup-core` first             | bevy part 83 s, +0.50 GB |
| warm re-lint after touching `gup-bevy/src/lib.rs` | 1 s                      |

**Branch taken: CI-only.** 98 s exceeds 90 s, so `--exclude gup-bevy` is on
every `--workspace` cargo call in `maskfile.md` (`check`, `lint`, `lint-check`,
`all-fix`'s clippy, `ci tests`) and in `tests.yml`, which gains an
`integrations` job (clippy and `xvfb-run` tests). `scripts/test_pre_commit.sh`
fixtures list `gup-bevy` among the dependents. `clippy_wasm32.sh` lists it as
excluded.

**Rendered output**: the example under `xvfb-run` shows the "Bevy scatter"
title, tick labels, 20 static points and the appended sine curve on a sprite
(read from `/tmp/bevy.png`). Colour parity: three mark centres (points 3,
11, 17) measured ΔE 0, 0, 0 against `ChartExt::render` (limit 2). Needed Bevy
features: `2d_bevy_render` (without it the window is black: no 2D pipeline).

**Gates**: `mask all-check` passes; `mask old-path-loc` TOTAL 18504 (unchanged);
`mask wasm-size` (via perf budgets) scatter 334149, over-wgpu 301789 gz bytes,
all pass; dogfood suite 10 pass; one `wgpu` in `Cargo.lock`.

## Retrospective

**Completed**: 2026-10-23

### Key Technical Learnings

#### Bevy's feature set decides whether anything draws

- **Challenge**: the first screenshot was black with no log output.
- **Solution**: `bevy_log` showed nothing was wrong at run time; the real cause
  was missing `bevy_core_pipeline` and `bevy_sprite_render`. `2d_bevy_render`
  brings both.
- **Pattern**: enable `bevy_log` in example crates; a silent black frame means a
  missing render feature.

#### Hosts need `Send + Sync` charts

- **Challenge**: `ScatterChart`'s pending-op closures were `Send` only.
- **Solution**: `+ Sync` on the bound and the alias; asserted by a test.
- **Pattern**: assert thread-safety of public types in a test.

### Architectural Decisions

#### CI-only placement for gup-bevy

- **Decision**: the literal rule (cold clippy of `-p gup-bevy`) gave 98 s.
- **Reasoning**: follow the rule as written; the margin is small, and the disk
  cost (0.42 GB) would have allowed the local gate.
- **Trade-off**: bevy breakage shows up in CI, not in `mask all-check`.
- **Future**: if the toolchain gets faster, remove the excludes.

### Development Workflow Insights

- Example mark choice: a point at y = 0 lies on the axis line; pick points clear
  of guides.
- Adding a member changes the `test_pre_commit.sh` fixtures (alphabetical).

### Follow-up Stories

None.
