---
implementer: sonnet
implementer-reason:
  widget API, texture format, input mapping, context sharing and the workspace
  change are fixed in Decisions; the input mapping exists in dogfood t4
---

# GUP-483: RFC-001 Step S13a: `gup-egui` on `gup-core`, Back in the Workspace

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-19

## Context

`gup-egui` was parked by an owner decision (strategic review 2026-10, Decision
3; GUP-390): excluded from the workspace and CI "until T2 lands, then re-wire
onto the single render path". T2 has landed: one `Context` (S1, with
`Context::from_wgpu`), `Scene`/`Renderer`/`RenderTarget` with `TextureTarget`
and `Prepared::draw` (S3), and an object-safe `Chart` with input (S7e, S8a).
RFC-001 S13 reads "Re-wire `gup-egui` and `gup-bevy` through `from_wgpu` and
`Prepared::draw`; back into the workspace and CI".

Today `gup-egui` (820 lines) wraps the old path: `DynChart` over
`ComposedChart`, `GupEguiContext` over `RenderContext`, and a bridge to the old
`InteractionEvent`. Meanwhile the dogfood crate wrote its own embedding,
`ChartTexture` (`dogfood/src/lib.rs`), which renders on a separate gup device,
reads the pixels back and uploads them to egui every time it is dirty, plus the
pointer mapping in `t4_linked.rs`. GUP-465 noted "`ChartTexture` is dogfood user
code" pending S13.

This story rewrites `gup-egui` on `gup-core`: egui's own device through
`Context::from_wgpu`, a `TextureTarget` that egui samples directly (no
readback), pointer input to `Chart::handle`, egui's repaint as the chart's
waker. Dogfood tasks 4 and 5 switch to it. `DynChart` is deleted, which is one
of RFC-001 S11's named deletions.

## Read First

- RFC-001 "S1 findings" proposal "S13 (hosts)" (line 1446) and "S3 findings"
  "S13 (hosts)" (line 1645).
- `crates/gup-core/src/target.rs` lines 465–560 (`TextureTarget`: the sRGB
  texture with a non-sRGB view rule), `crates/gup-core/src/render.rs`
  `Renderer::render`/`prepare` (lines 1184–1300).
- `crates/gup-core/src/context.rs` `from_wgpu` (lines 340–390) and `ContextId`.
- `crates/gup-core/src/chart.rs` (`Chart`, `set_waker` from GUP-480,
  `as_any_mut`).
- `dogfood/src/lib.rs` lines 40–120 (`ChartTexture`, `save_screenshot`) and
  `dogfood/src/bin/t4_linked.rs` lines 160–230 (the egui → `Input` mapping).
- The current `gup-egui/src/*.rs`, `gup-egui/examples/egui_chart.rs`,
  `gup-egui/tests/widget_tests.rs` (to delete), `docs/EGUI_INTEGRATION.md`.
- Root `Cargo.toml` `[workspace]` (lines 1–12), `scripts/clippy_wasm32.sh` (the
  `excluded` map), README "Parked integration crates" (lines 280–313).

## User Story

> "As an egui app developer, I want to put a Gup chart in a panel with one
> widget call, rendered on egui's own GPU device and interactive, so that I
> don't copy pixels through the CPU or write my own input glue."

## Acceptance Criteria

### AC1: The crate on gup-core

- [ ] `gup-egui/Cargo.toml` depends on `gup-core` (path) and no longer on `gup`.
      `DynChart`, `GupEguiContext`, `translate_response`, `map_to_chart_coords`
      and the old tests are deleted.
- [ ] The public API is exactly Decisions' `GupWidget`, `context` and
      `to_input`, each with rustdoc; `lib.rs`'s crate docs show the quick-start
      (a `no_run` doctest that compiles).

### AC2: Rendering is the chart's own image

- [ ] A test in `gup-egui/tests/` renders a `gup_core::scatter` through the
      widget's render path (Decisions: `render_into`) into its texture at dpr 1,
      reads it back, and finds it byte-identical to `ChartExt::render` at the
      same size; at dpr 2 it equals an `ImageTarget` with `dpr: 2.0`.
- [ ] Unit tests for `to_input`: move, primary press inside, release (inside and
      outside), wheel in each `MouseWheelUnit`, pointer gone → the Decisions
      mapping, positions relative to the widget rect.

### AC3: The example

- [ ] `gup-egui/examples/egui_chart.rs` (at most 80 lines): an eframe app with a
      `gup_core::scatter` (tooltips) beside a live `gup_core::line` fed by an
      `Appender` from a thread. With `GUP_SCREENSHOT_PATH` set it saves an egui
      screenshot after 30 frames and exits (the `save_screenshot` pattern).
- [ ] Run under `xvfb-run` and read the screenshot: both charts with titles,
      tick labels and marks, the line grown past its first rows.
- [ ] Colour parity: at three mark-centre pixels of the scatter (positions from
      its `Layout`), the screenshot is within ΔE 2 (the `gup-visual-regression`
      ΔE helper) of `ChartExt::render` of the same chart at the same size.

### AC4: Dogfood uses it

- [ ] `t4_linked` and `t5_live` use `gup_egui::GupWidget`; `ChartTexture` and
      t4's hand-written pointer mapping are deleted; `dogfood/Cargo.toml` adds
      `gup-egui` (path).
- [ ] `xvfb-run -a ./dogfood/run_all.sh`: t4 and t5_live pass every check
      unchanged (no threshold or region changed); no FAIL, no XPASS.

### AC5: Workspace, CI and docs

- [ ] `gup-egui` is in `[workspace] members` (not `default-members`) and out of
      `exclude`; `mask all-check` and `cargo test --workspace` now cover it.
      Record the extra cold `mask all-check` time and build-directory growth.
- [ ] `scripts/clippy_wasm32.sh` lists `gup-egui` in `excluded` with the reason
      in Decisions.
- [ ] `gup-egui/README.md` and `docs/EGUI_INTEGRATION.md` describe the new API
      (the parked note is gone); the root README's parked table drops
      `gup-egui`.

## Decisions

- **API**:

  ```rust
  /// A Gup chart in egui: renders `chart` into a texture on egui's device
  /// and forwards pointer input to it. Keep one per chart.
  pub struct GupWidget { /* render_state, Renderer, texture state, pointer state */ }
  impl GupWidget {
      pub fn new(render_state: &egui_wgpu::RenderState) -> Self;
      /// Draw `chart` in a `size` (points) rect and forward input to it.
      pub fn show(&mut self, ui: &mut egui::Ui, size: egui::Vec2,
                  chart: &mut dyn gup_core::Chart) -> egui::Response;
  }
  /// The `gup_core::Context` over egui's device: made once per render
  /// state with `Context::from_wgpu` and kept in its renderer's
  /// `callback_resources`.
  pub fn context(render_state: &egui_wgpu::RenderState) -> gup_core::Context;
  /// The `Input` for an egui event, in coordinates relative to `rect`.
  pub fn to_input(event: &egui::Event, rect: egui::Rect) -> Option<gup_core::Input>;
  ```

  The chart stays the caller's (`&mut dyn Chart`), as `ChartTexture` did, so
  apps mutate and rebuild it freely; no downcasting. `egui_wgpu` comes from
  `eframe::egui_wgpu` (no new dependency).

- **Texture**: `Rgba8UnormSrgb` with `view_formats: [Rgba8Unorm]`, usage
  `RENDER_ATTACHMENT | TEXTURE_BINDING`, size `round(size × pixels_per_point)`;
  Gup draws through `TextureTarget` (its non-sRGB view), egui samples the sRGB
  view, registered with `Renderer::register_native_texture` (linear filter) and
  updated with `update_egui_texture_from_wgpu_texture` on resize. Reason: Gup
  writes sRGB-encoded values; an sRGB view decodes them for egui's shader, which
  re-encodes for its framebuffer.
- **Draw path**: `TextureTarget` + `Renderer::render` (Gup's own pass and
  submit), factored as
  `pub(crate) fn render_into(cx, chart, size_pts, ppp, target) -> Result<()>`. A
  paint callback through `Prepared::draw` is not used: `Prepared::draw` sets
  scissor rects in target pixels, and an egui callback's pass is the whole
  frame, so clips would land in the wrong place. Recorded as a follow-up in the
  retrospective, not built.
- **When it renders**: on every `show`. egui repaints only on input or request,
  and a clean `resolve` writes nothing.
- **Input**: inside the rect, `PointerMoved` → `PointerMoved`; a primary,
  secondary or middle press inside → `PointerDown` (and the widget remembers the
  press); a release while pressed (inside or not) → `PointerUp`; `MouseWheel`
  while hovered → `Wheel` with `lines = delta.y` for `Line`, `delta.y / 40.0`
  for `Point`, `delta.y × 10.0` for `Page`; `PointerGone` or leaving the rect
  when not pressed → `PointerLeft`. Positions are `pos − rect.min` in points (=
  Gup logical pixels).
- **Repaint**: `response.redraw` or `chart.needs_frame()` →
  `ui.ctx().request_repaint()`. The waker passed to `chart.set_waker` on every
  `show` holds an `egui::Context` clone and calls `request_repaint()`.
- **Workspace**: member, not default member. `clippy_wasm32.sh` `excluded`
  entry: "a native host integration; the browser host is `gup_core::web`". This
  story is authorised to edit the root `Cargo.toml` workspace lists and that
  script for this purpose only.

## Escalation Triggers

- The colour parity check (AC3) fails by more than ΔE 2: the sRGB view
  assumption is wrong for egui 0.33's shader.
- egui-wgpu's `RenderState` device is not usable with `Context::from_wgpu`
  (missing features or limits gup-core requires).
- Adding the member changes `Cargo.lock`'s resolved `wgpu` (there must stay one
  wgpu, RFC-001 "S1 findings").
- Cold `mask all-check` grows by more than 120 s.

## Out of Scope

- `gup-bevy`: GUP-484.
- A paint-callback (in-pass) mode; egui on wasm.
- Mobile and Tauri crates: strategic review T7.

## Technical Tasks

- [ ] Delete the old modules and tests; new `Cargo.toml`.
- [ ] `context`, `to_input`, `GupWidget` (texture, render, input, repaint,
      waker); `render_into`.
- [ ] Tests (AC2); example and screenshot; parity check.
- [ ] Dogfood t4 and t5_live on the widget; suite run.
- [ ] Workspace, `clippy_wasm32.sh`, READMEs, `docs/EGUI_INTEGRATION.md`.

## Dependencies

### Prerequisite Stories

- GUP-480: RFC-001 S12a Appender Handle 📋 — `Chart::set_waker` and the
  example's live line.
- GUP-401, GUP-444, GUP-434: `TextureTarget`, `Chart::handle`, `Chart` ✅.

### Enables Stories

- GUP-488: S14c — no workspace crate depends on the old path's egui integration.

## Testing Strategy

- **Unit tests**: `to_input`.
- **Integration tests**: `render_into` byte parity at dpr 1 and 2.
- **Visual validation**: the example screenshot and the ΔE parity check.
- **End to end**: dogfood t4 and t5_live under `xvfb-run`.

## Success Metrics

- [ ] An egui chart in two calls: `GupWidget::new` and `show`.
- [ ] No CPU readback in the egui path.

## Risk Assessment

- **Medium**: egui's texture registration API differs from what Decisions names
  in 0.33. _Mitigation_: same intent through whatever 0.33 provides for native
  textures; record the call used.
- **Low**: trybuild `.stderr` churn: none expected (no gup-core source edits).
  If gup-core changes are needed, re-bless only line-number diffs.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`,
      `cargo test -p gup-egui -- --test-threads=1`
- [ ] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS)
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples -p gup-egui`
- [ ] `mask wasm-browser` passes (this story touches input handling; it must not
      change gup-core's browser behaviour)
- [ ] `mask wasm-size` run and the figures reported in the Implementation
      Summary (expected unchanged: gup-egui is not in the wasm harness; the
      orchestrator re-records)
- [ ] Rendered output verified by eye: the example screenshot, `t4_linked.png`,
      `t5_live.png`
- [ ] `mask old-path-loc` recorded (expected unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
