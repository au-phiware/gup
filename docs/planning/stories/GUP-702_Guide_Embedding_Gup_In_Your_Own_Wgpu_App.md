---
implementer: sonnet
implementer-reason:
  every name and loop is already in the code and its rustdoc; the page's home,
  shape and checks are decided below, and one claim (the sRGB recipe) gets a
  test
touches:
  [
    docs/EMBEDDING.md,
    docs/INDEX.md,
    docs/README.md,
    README.md,
    src/lib.rs,
    crates/gup-core/tests/targets.rs,
    CHANGELOG.md,
  ]
---

# GUP-702: Guide: Embedding Gup In Your Own Wgpu App

## Story Overview

**Initiative**: Strategic Review 2026-11 (first outside user, before 0.1) X
**Created**: 2026-11-09

## Context

The first outside user (a Game of Life app with its own wgpu 27 renderer) drew
gup charts into a texture it then composited itself. It reached that by reading
`gup-egui/src/render.rs`, the rustdoc on `TextureTarget` and `Prepared::draw`,
and trial. Nothing in `docs/` says how: `docs/EGUI_INTEGRATION.md` and
`docs/BEVY_INTEGRATION.md` teach those two crates, and the tour's chapter 13
(`docs/tour/13_everywhere.md`) lists `TextureTarget` as a row in a table.

The pieces exist and are documented one by one: `Context::from_wgpu`
(`context.rs` line 346), `TextureTarget` (`target.rs` line 480, whose doc names
the sRGB rule), `Renderer::render`, `Renderer::prepare` with `Prepared::draw`
and `Prepared::encode` (`render.rs` lines 1459 to 2010), `Chart::handle`,
`Plot::view_axes`. This story is the page that puts them in order.

Checked against `main` (2026-11-09), the report's recipe is right and two
details need care:

- Two texture recipes are both correct. `gup-egui` (`widget.rs` line 258) makes
  a plain `Rgba8Unorm` texture and has egui sample it raw (egui's shader works
  on gamma bytes). A host whose shader samples through hardware sRGB decode and
  writes to an sRGB surface wants `Rgba8UnormSrgb` with
  `view_formats: &[Rgba8Unorm]`, so `TextureTarget` (which draws through the
  non-sRGB twin, `target.rs` line 520) still writes gamma bytes and the host's
  own sampling and surface encode return them unchanged. The guide gives both
  and says which host wants which.
- `Prepared::encode` is not a general "record your pass" call. It records the
  offscreen pre-pass of a translucent `Line` layer (GUP-439); without it
  `Prepared::draw` still draws that layer, directly, and it blends twice where
  its segments overlap (`render.rs` lines 1464 to 1470 and 1915 to 1935). The
  guide says that and nothing more about it.
- `Plot::view_axes` exists only on `Plot` (`plot.rs` line 1400), not on `Chart`
  or the builders; a builder reaches it through `plot_mut()`, and a
  `Box<dyn Chart>` cannot, so a host sets it before boxing.

## Read First

- `gup-egui/src/render.rs` (all 25 lines) and `gup-egui/src/widget.rs` lines 240
  to 300 (`resize`: the texture, the target, the dpr) and its input forwarding
  in `gup-egui/src/input.rs` lines 1 to 60.
- `crates/gup-core/src/target.rs` lines 51 to 95 (`TargetOptions`) and 465 to
  560 (`TextureTarget` and its doc).
- `crates/gup-core/src/render.rs`: `Renderer` and `prepare` (1371 to 1480),
  `Prepared::encode` and `draw` (1895 to 2040), `TargetDesc` (28 to 50).
- `crates/gup-core/tests/targets.rs` lines 19 to 130 (`read_texture`,
  `host_texture`, `draw_in_host_pass`): a working host pass.
- `crates/gup-core/src/context.rs` lines 342 to 348 (`from_wgpu`) and lines 296
  to 304 (the `DeviceDescriptor` gup itself builds).
- `crates/gup-core/src/input.rs` lines 40 to 135 (`Input` constructors) and 225
  to 235 (`Response`); `chart.rs` lines 51 to 100 (`handle`, `needs_frame`,
  `set_waker`).
- `docs/EGUI_INTEGRATION.md` (how a sibling guide is built and compiled) and
  `src/lib.rs` lines 296 to 345 (the `cfg(doctest)` modules).

## User Story

> "As a developer with my own wgpu renderer, I want one page that shows how to
> draw a gup chart into my own texture or pass, resize it and forward input, so
> that I do not learn it from another crate's source."

## Acceptance Criteria

### AC1: The page exists and is compiled

- [ ] `docs/EMBEDDING.md` exists, is included as doctests from `src/lib.rs` in a
      new `#[cfg(doctest)] mod embedding_docs` beside `accessibility_docs`, and
      every code block compiles and runs under `cargo test --doc` (blocks that
      need a window or a surface use a texture instead; none is `ignore`).
- [ ] It is linked from `README.md` (Quick Start's end), `docs/README.md`,
      `docs/INDEX.md` (under Integrations) and chapter 13's `TextureTarget` row
      (`docs/tour/13_everywhere.md`, a link in the "You call" column; keep the
      tour's page tests passing).

### AC2: What the page covers, in this order

Each is a heading with a runnable block, and every name in it is checked against
the code by compiling:

- [ ] **Your device**: create a wgpu 27 device in the host's way and wrap it
      with `Context::from_wgpu(device, queue)`. State that the major version of
      wgpu must match (`gup::wgpu` re-exports it), that `from_wgpu` requests
      nothing from the adapter, and that `Caps` is read from the device (so a
      device made with default limits gets default-limit chunk sizes). Show the
      device creation with the `experimental_features` field set to
      `wgpu::ExperimentalFeatures::disabled()`, and say that wgpu 27's
      `DeviceDescriptor` has that field (a struct literal without
      `..Default::default()` fails to compile without it).
- [ ] **A texture and a target**: `Rgba8Unorm` first (the egui recipe), then
      `Rgba8UnormSrgb` with `view_formats: &[Rgba8Unorm]` (the sRGB-surface
      recipe), each with `RENDER_ATTACHMENT | TEXTURE_BINDING`, single-sampled,
      wrapped by `TextureTarget::new` with
      `TargetOptions::default().with_dpr(dpr)`. One sentence on why: gup writes
      gamma-encoded bytes and blends in that space; the target draws through the
      non-sRGB view.
- [ ] **The frame loop**: resolve at the logical size, then render the scene
      into the target (the two lines below), with one `Renderer` kept across
      frames.

  ```rust
  let resolved = chart.resolve(&cx, w_logical, h_logical)?;
  renderer.render(&cx, &resolved.scene, &mut target)?;
  ```

  State that a clean chart resolves to the same scene at no GPU cost, that
  `render` runs gup's own pass and submits once, and that the host samples the
  texture after, in its own pass.

- [ ] **Logical and physical size**: the chart is sized in logical pixels; the
      texture is `round(logical * dpr)` physical pixels; `with_dpr` is the same
      `dpr`; input points are logical pixels from the chart's top-left.
- [ ] **Resize**: there is no `TextureTarget::resize`; on a size or dpr change,
      make a new texture and a new `TextureTarget` (as `gup-egui`'s `resize`
      does), keep the `Renderer` and the chart, and re-register the new view
      with the host's own bind group.
- [ ] **Input**: forward the host's events as `Input::pointer_moved`,
      `pointer_down`, `pointer_up`, `wheel` and `key_with` (all
      `Point::new(x, y)` in logical pixels, through the prelude), call
      `chart.handle(input)`, and re-render when `Response::redraw` is true or
      `chart.needs_frame()` is. Say `Response` is `#[must_use]`. Say
      `Chart::set_waker` is how an appender wakes the host. Show
      `chart.plot_mut().view_axes(Axes::X)` on a line chart and note it is on
      `Plot` only.
- [ ] **Your own pass (advanced)**: the calls in the block below, with a
      `TargetDesc` for the host's attachment (format non-sRGB, physical size,
      dpr, samples), `prepared.clear_color()` as the clear.

  ```rust
  let prepared = renderer.prepare(&cx, &resolved.scene, &desc)?;
  prepared.encode(&mut encoder); // optional, before the host's pass
  prepared.draw(&mut pass); // inside the host's pass
  ```

  Say: `prepare` rejects an `*Srgb` format; the pass's colour view must be
  non-sRGB (make an sRGB surface's view with the non-sRGB format listed in
  `view_formats`); gup never submits in this path; `encode` is only the
  translucent-line pre-pass and skipping it blends such lines twice where
  segments overlap.

- [ ] **Rebuilding a live chart**: one paragraph that points at
      `Chart::view_is_home` (GUP-699) so a host does not discard the user's
      zoom, once that has landed; if GUP-699 has not landed when this story is
      implemented, omit the paragraph and say so in the summary.

### AC3: The sRGB claim is tested

- [ ] A new test in `crates/gup-core/tests/targets.rs` draws one chart through a
      `TextureTarget` on an `Rgba8UnormSrgb` texture with
      `view_formats: &[Rgba8Unorm]`, reads the texture's bytes back raw, and
      asserts they equal `ChartExt::render` of the same chart exactly (zero
      differing pixels, as `image_texture_and_draw_in_pass_agree` asserts for
      the unorm texture). Cite the test by name in the page.
- [ ] The test also asserts a texture with the sRGB format and no `view_formats`
      entry is rejected by `TextureTarget::new` with an error naming
      `view_formats`, if it is; if wgpu panics or the error does not name it,
      stop (Escalation Triggers).

### AC4: Names are verified

- [ ] Every item the page names is checked by compiling the block that uses it;
      the page's prose does not name an item that no block uses, except
      `Chart::set_waker` and `Chart::needs_frame`, which are checked by this
      command, whose output is quoted in the summary:

  ```bash
  grep -n "fn set_waker\|fn needs_frame" crates/gup-core/src/chart.rs
  ```

- [ ] The page states no version or feature it has not read from the repository:
      wgpu 27 (`Cargo.lock`), Rust 1.93 (`Cargo.toml` `rust-version`).
- [ ] CHANGELOG `[0.1.0]` Added: the embedding guide.

## Decisions

- **Home: `docs/EMBEDDING.md`**, a sibling of `EGUI_INTEGRATION.md` and
  `BEVY_INTEGRATION.md`, included as doctests from `src/lib.rs`. Not a tour
  chapter 14: tour pages are a snippet file plus a golden image each, kept in
  sync by `tests/tour.rs`, and this page draws nothing to show. Not a tutorial
  8: the tutorials teach building charts from rows; this teaches a host's
  rendering loop, and it is a how-to (the integration guides are the precedent).
- **The page runs on a device it creates itself** (`gup::wgpu` instance and
  adapter, blocking with `pollster` if the root crate has it, else
  `Context::new_blocking()` and its device cloned into `from_wgpu`; use
  whichever the other doctests use, found by grepping `pollster` in the two
  `Cargo.toml` files). It never needs a window.
- **One sRGB sentence, two recipes**, as in Context above. The page does not
  tell the host which blend space to use in its own shaders.
- **No new API.** If the page is awkward without one (a
  `TextureTarget::resize`), say so in the retrospective as a follow-up; do not
  add it here.
- **Placement of the input list**: only constructors that exist in `input.rs`;
  no invented ones.

## Escalation Triggers

- `Context::from_wgpu` over a device created with wgpu's default limits fails a
  render of the reference scatter (the page would then have to name the limits
  gup needs; report which).
- The sRGB recipe does not reproduce `ImageTarget`'s pixels exactly.
- `TextureTarget::new` accepts an sRGB texture without `view_formats` and draws
  wrong colours silently.
- The doctests need a GPU that CI's doc job does not have.
- Chapter 13's table edit breaks `tests/tour.rs`.

## Out of Scope

- A `TextureTarget::resize`, or any API change.
- Surfaces and swapchains: `WindowTarget` is for gup-owned windows; the page
  covers hosts that composite a texture or draw into their own pass.
- Bevy and egui specifics (their guides own those).
- The first-run requirements (GUP-704).

## Technical Tasks

- [ ] Write `docs/EMBEDDING.md` per AC2, building each block from the sources in
      Read First.
- [ ] `embedding_docs` in `src/lib.rs`; links in README, docs index, docs README
      and chapter 13.
- [ ] The sRGB test in `tests/targets.rs`.
- [ ] CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-686: The Prelude Holds What Users Reach For ✅ — `Input` and friends in
  the prelude.
- GUP-700: One `Point` 📋 (soft) — the page's input code writes `Point::new(..)`
  from the prelude; without it, import `gup::geom::Point`.
- GUP-701: Row Window Type Name 📋 (soft) — only if the page shows a windowed
  line.
- GUP-699: Live Charts Keep Data Off The Axis 📋 (soft) — for the last
  paragraph.

### Enables Stories

- None.

## Testing Strategy

- **Unit tests**: the sRGB texture test.
- **Integration tests**: `cargo test --doc` runs every block of the page.
- **Visual validation**: none; the page draws no image. The test in AC3 is the
  pixel check.
- **Performance**: not applicable.

## Success Metrics

- [ ] A developer with a wgpu 27 renderer can follow the page, top to bottom, to
      a chart in their own texture, using only what the page and the prelude
      name.

## Risk Assessment

- **Medium**: the page can drift from the code. _Mitigation_: every block is a
  doctest.
- **Medium**: the sRGB recipe has not been tested end to end in this repository.
  _Mitigation_: AC3 and its trigger.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, the page draws nothing; AC3
      compares pixels exactly
- [ ] `mask wasm-browser`: not needed, docs and a test only
- [ ] WASM size: unchanged: no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-11-11)

**Question**: AC3 says to stop if `TextureTarget::new` rejects an sRGB texture
without `view_formats` with an error that does not name `view_formats`. It
rejects it (no panic, no wrong colours), but the message does not name it. May I
add a check to `TextureTarget::new` (a library change, about 10 lines)?

**Evidence**: the sRGB recipe itself is proven: an `Rgba8UnormSrgb` texture with
`view_formats: &[Rgba8Unorm]` read back raw equals `ImageTarget`'s image with 0
differing pixels (test
`srgb_texture_with_unorm_view_format_matches_image_target`, passing). The
refusal case returns:

```text
GPU error in texture target (Rgba8Unorm view of the host's texture): Validation Error
  In Texture::create_view, label = 'gup texture target'
    Unable to view texture Rgba8UnormSrgb as Rgba8Unorm
```

**Options**:

1. Add a pre-check in `TextureTarget::new`: if the texture's format is sRGB and
   `view_formats` (via `texture.format()` and the descriptor's list, which wgpu
   27 does not expose on `Texture`, so the check would have to attempt the view
   and map the error) lacks the twin, return
   `Error::config("texture target", "... list Rgba8Unorm in view_formats")`.
   Cheapest form: on the `create_view` failure, when
   `texture.format().is_srgb()`, rewrite the message to name `view_formats`.
   Better UX for the first outside user.
2. Change AC3 to assert the `Err` and that it names the format pair
   (`Unable to view texture Rgba8UnormSrgb as Rgba8Unorm`), and let the guide
   quote that. No library change.

**Recommendation**: option 1 (the rewrite on failure), because the guide's
reader hits exactly this error. The gated test
`srgb_texture_without_view_formats_is_refused_by_name` is committed under
`#[cfg(any())]` and becomes live with option 1. Everything else (the guide,
links, CHANGELOG) is unstarted and independent of the answer.
