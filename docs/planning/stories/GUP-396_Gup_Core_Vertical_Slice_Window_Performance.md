# GUP-396: gup-core Vertical Slice, Window and Performance

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-05)
**Created**: 2026-10-04

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

- [x] `gup_core::WindowTarget` implements the `RenderTarget` trait from RFC-001
      §7, wrapping a `winit` surface created from an `Arc<Window>` (per this
      project's established `Arc<Window>` surface-lifetime pattern).
- [x] The same `Scene`-building code used by GUP-395's PNG path (not a duplicate
      or re-derived scene) is drawn into both `ImageTarget` and `WindowTarget`
      by the same `Renderer`/`Prepared` draw call, proving the "one render path"
      claim (RFC-001 §1 Goals, §7).

### AC2: A minimal `gup::show()` displays a gup-core chart

- [x] A minimal `gup_core::show(chart)` function (RFC-001 §8's `ChartExt`-style
      entry point, scoped down to what this spike's `Chart`-like scatter scene
      needs — a full `Chart` trait implementation is S7/S8's job) opens a
      window, creates a `Context` and `WindowTarget`, and redraws the GUP-395
      scatter scene on resize and on a wheel-zoom input. _Resize was checked
      live (niri column 1920 → 960 px; the layout re-resolved). No tool on this
      machine can inject wheel events, so the wheel path (`Plot::zoom` about the
      cursor) is covered by unit tests and the benchmark's per-frame domain
      changes rather than a live scroll._
- [x] Running the resulting minimal example headlessly via `GUP_SCREENSHOT_PATH`
      (the project's existing headless-screenshot mechanism) produces a
      window-rendered screenshot. _It runs non-interactively and exits, but it
      still needs a display: the PNG is the window's own surface texture read
      back, not an offscreen render, which is what makes AC3 meaningful._

### AC3: Window and PNG outputs are visually equivalent (user-visible)

- [x] **AC (user-visible, required)**: the `WindowTarget` screenshot (via
      `GUP_SCREENSHOT_PATH`) and the GUP-395 `ImageTarget` PNG are compared
      pixel-by-pixel (same scene, same dimensions, same dpr) and the maximum
      per-pixel colour difference is **ΔE < 2** (CIE76 or CIE2000, document
      which). Use the GUP-388 visual regression harness's comparison utility if
      it has landed by implementation time; otherwise compute ΔE directly (e.g.
      with the `image`/`palette` crates) and record the computed value — "looks
      the same" is not sufficient, a number is required.
- [x] A written note in the retrospective states the measured ΔE and includes
      both images' paths.

### AC4: 100K-point zoom sustains ≥60 fps with 0 column bytes written

- [x] A benchmark (criterion, or a dedicated instrumented binary/test) renders
      100K points (Linear x, Log y, Sequential fill, constant radius — the
      encodings GUP-395 implements) in a `WindowTarget` or an equivalent
      off-screen-but-presenting loop, and simulates a continuous zoom (domain
      changes on every frame) for at least 300 frames.
- [x] Frame time is measured (e.g. via `gpu_timer.rs`'s pattern, survived per
      RFC-001 §11's migration table, or a CPU-side `Instant` wrapper) and the
      median and p95 fps are recorded; the median must be **≥60 fps**.
- [x] A **buffer-write counter** — a thin wrapper or instrumented path around
      `queue.write_buffer` calls targeting column sub-ranges — proves **0 column
      bytes** are written during the zoom loop. Only the `Encodings`/view
      uniform buffers may be written per frame. The counter's value (0) and the
      measurement method are recorded in the retrospective. _Deviation: guide
      instance buffers (axis rules and glyph quads, about 9.4 KB per frame) are
      also rewritten every frame, because the ticks move while zooming. They are
      counted separately (`Upload::Instances`) and are not column data._
- [x] If the ≥60 fps target is not met on the development machine's GPU, record
      the actual number, the GPU used, and whether the shortfall is in CPU
      submission overhead or GPU time (e.g. via the timer), rather than silently
      lowering the bar. _Not needed: the target is met (60.0 fps median under
      vsync, 231–245 fps uncapped). The GPU and CPU split is recorded anyway._

## Technical Tasks

- [x] Implement `WindowTarget` (`RenderTarget` impl: `desc`, `acquire`,
      `present`) over a `winit` surface held via `Arc<Window>`.
- [x] Factor GUP-395's scene-building code so it is shared (not duplicated)
      between the `ImageTarget` and `WindowTarget` call sites.
- [x] Implement a minimal `gup_core::show(..)` that opens a window, builds a
      `Context::new_blocking()` (or reuses one), creates a `WindowTarget`, and
      drives a redraw loop with resize and wheel-zoom input handling that
      mutates the Linear/Log scale domains only (no column re-upload).
- [x] Wire the existing `GUP_SCREENSHOT_PATH` headless-screenshot mechanism into
      the new minimal example/binary used for this story's window tests.
- [x] Write the window/PNG ΔE comparison (reuse GUP-388's harness if landed;
      otherwise a small standalone comparison using the `image` crate plus a
      CIE76/CIE2000 ΔE implementation — check if one is already a transitive
      dependency, e.g. via `palette`, before adding a new one).
- [x] Implement the buffer-write counter: wrap or instrument the `ColumnStore`'s
      `queue.write_buffer` call sites so a test can assert zero column-range
      writes occurred during a domain-only update.
- [x] Write the 100K-point zoom benchmark, generating synthetic data once and
      reusing the GUP-395 column upload path, then looping a simulated zoom
      (domain shrink/grow) for ≥300 frames while recording frame time and the
      buffer-write counter.
- [x] Record fps results, ΔE measurement, and the zero-byte-write proof in the
      story retrospective.
- [x] Verify the old `gup` crate's `src/` tree still has zero diff from `main`
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

- [x] Window screenshot and PNG differ by ΔE < 2, with the measured value
      recorded.
- [x] 100K-point zoom sustains ≥60 fps median over ≥300 frames, with the
      buffer-write counter at 0 column bytes for the entire run.
- [x] `gup_core::show(..)` runs headlessly via `GUP_SCREENSHOT_PATH`.
- [x] `git diff main -- <old-path files>` remains empty.

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Window/PNG ΔE and the 100K-point zoom fps + zero-byte-write results
      verified by eye/measurement and recorded in the retrospective
- [x] No diff against `main` in the old `gup` path (same file list as GUP-395)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

S0 is complete. Both exit criteria this story owns hold on the development
machine, with measured numbers. They are recorded with their method in RFC-001,
"S0b findings (2026-10-05, GUP-396)".

| Area                                                                                                                   | Files                                                  |
| ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Upload counters (`Upload`, `UploadStats`, `WriteCount`); every GPU write goes through `Context`; `with_options`        | `src/context.rs`                                       |
| `PositionScale::set_domain`/`invert`; pixel-space `zoom` on the plot's scale handles; `Plot::zoom`, `Plot::title_text` | `src/scale/{mod,linear,log}.rs`, `src/plot.rs`         |
| Layer GPU state kept between prepares; uniforms written in place                                                       | `src/selection.rs`, `src/render.rs` (`LayerGpu`)       |
| `Renderer::prepare` → `Prepared::draw`/`clear_color`, `Renderer::render`; pooled guide and view buffers                | `src/render.rs`                                        |
| `RenderTarget`, `Frame`, shared RGBA/BGRA `Readback`, `ImageTarget` on the trait                                       | `src/target.rs`                                        |
| `WindowTarget` (feature `window`, default): non-sRGB view, present modes, frame capture                                | `src/window.rs`                                        |
| `gup_core::show` (resize, wheel zoom, Escape, `GUP_SCREENSHOT_PATH`, safe teardown)                                    | `src/show.rs`                                          |
| Glyph atlas uploads dirty rows only                                                                                    | `src/text.rs`                                          |
| Shared reference scatter                                                                                               | `tests/common/scatter.rs`                              |
| Window/PNG parity (ignored: needs a display), headless zoom upload test                                                | `tests/window_parity.rs`, `tests/zoom_uploads.rs`      |
| Examples                                                                                                               | `examples/scatter_window.rs`, `examples/zoom_bench.rs` |

Also changed: `crates/gup-core/Cargo.toml` (optional `winit`, `window` feature,
example entries), `Cargo.lock` (one edge), `maskfile.md` (`mask gup-core-window`
runs the display-only checks), RFC-001 (S0b findings).

Tests (`cargo test -p gup-core -- --test-threads=1`): 44 unit tests plus one
ignored timing measurement; trybuild (four cases); `scatter_png` (golden, still
byte-identical); `zoom_uploads`; `window_parity` (ignored without `--ignored`;
passes with a display); 3 doctests. New unit tests: three scale tests (invert,
`set_domain`, anchor-fixed zoom), two log-scale tests, `Plot::zoom`,
`every_gpu_write_is_counted`; the rescale test now asserts `Arc<LayerGpu>`
reuse, 0 column bytes and three uniform writes.

Commits: `325ac3a` counters/in-place uniforms, `9a41330`
`RenderTarget`/`Renderer`/`WindowTarget`/`show`/parity, `a0560b9` benchmark,
`36c13a1` headless upload test and atlas dirty rows, `e6bba47` RFC findings.

The old path is frozen:
`git diff 43cda62 HEAD -- src/ Cargo.toml examples/ tests/` is empty, and
`mask old-path-loc` is still **28882**.

## Retrospective

**Completed**: 2026-10-05

### What the user sees

- **Window parity.**
  `/tmp/gup-target/visual-regression/gup_core/window_scatter.png` (the window's
  720×450 surface, read back) against `tests/golden/gup_core/scatter.png` and
  against an in-process `ImageTarget` render. **CIEDE2000 ΔE max 0, mean 0, 0 of
  324,000 pixels differ**: the window is byte-identical to the PNG. The control
  (the same pixels encoded a second time, which is what an sRGB view would do)
  gives ΔE max 28.67, with 16,397 pixels ≥ 2. The metric is the GUP-388
  harness's `Rgba8::delta_e` (CIEDE2000).
- **Read by eye.** The window capture shows the same title, 13 + 13 tick labels,
  viridis fill and round circles as the golden. Live windowed screenshots
  confirm re-layout on resize. At 1920×1052 (niri's tile) the x ticks fall every
  2000; after `set-column-width 50%` (960×1052) they fall every 5000, with the
  title still centred and the circles inside the plot rect.
- **Zoom benchmark** (Intel HD 630, Mesa 26.0.0 Vulkan, borderless fullscreen
  1920×1080, release build, 100K points, radius 3 px, 600 measured frames after
  60 warm-up frames, both domains changed every frame):
  - **vsync on** (`Fifo`, frames paced by winit redraw requests): **median 60.0
    fps, p95 59.2–59.3 fps**, 0 frames over 25 ms. The median frame interval is
    16.66 ms, the panel's 60.02 Hz refresh. About 45% of frames are a few tenths
    of a millisecond over 16.7 ms; that is jitter around vblank, not missed
    refreshes.
  - **vsync off** (`Mailbox`, frames back to back, `--uncapped`): **median
    231–245 fps, p95 145–183 fps** over two runs.
  - **Where the time goes:** the GPU. CPU work per frame (resolve with ticks and
    text, prepare, encode, submit, present) is 0.65–0.78 ms median. The GPU
    render pass is 2.7–3.0 ms median and 4.5–5.9 ms p95, from `TIMESTAMP_QUERY`.
    Uncapped, most of each frame is spent in `acquire`, waiting for a swapchain
    image.
  - **Method:** the frame interval is the `Instant` difference between
    consecutive frame starts. fps is 1000 / interval at the median and p95 of
    the interval. Timestamps are resolved once after the run, so they never
    stall it. A radius of 4.5 px gives the same picture (60.0 fps under vsync;
    217 fps uncapped).
  - **Column bytes during the zoom: 0, in 0 writes**, for every run. The counter
    is `Context::upload_stats()` (per-kind atomics), and every GPU write in the
    crate goes through it. Per frame the run writes 88 B of uniforms in 3 writes
    and about 9.4 KB of guide instances, and no textures. The only column write
    in the process is the startup upload (1,200,384 B).
- Raw benchmark output: `/tmp/gup396/bench_*.txt`.

### Key Technical Learnings

#### The window's pixels can be the proof

- **Challenge**: "headless via `GUP_SCREENSHOT_PATH`" in the old crate means an
  offscreen render. Comparing that with an `ImageTarget` PNG would prove nothing
  about the window.
- **Solution**: the surface is configured with `COPY_SRC`, which Mesa offers.
  `WindowTarget::capture_next_frame` copies the presented surface texture in a
  second command buffer of the same `submit`. `show` writes that copy.
- **Pattern**: compare what the user would see. Build the capture into the
  target, not into a separate offscreen path.

#### A zero needs a control

- ΔE 0 could also mean "compared the image with itself". The control (a double
  sRGB encoding of the window image, the bug the non-sRGB view policy prevents)
  scores ΔE 28.7. That shows both that the check is sensitive and that the
  colour policy is what makes the two outputs equal.

#### wgpu's GL backend ties the instance to the Wayland connection

- **Challenge**: the first screenshot run wrote a correct PNG, then segfaulted
  in `eglTerminate` → `wl_proxy_marshal` on exit. `Context::new` enables every
  backend. Creating a surface makes the GL backend bind its EGL display to
  winit's `wl_display`. The renderer's buffers held the last reference to the
  instance, and they were dropped after `run_app` had closed the connection.
- **Solution**: drop the surface, the renderer and the plot's GPU state in
  `ApplicationHandler::exiting`. Drop order inside `WindowState` is surface
  first.
- **Pattern**: any winit host must release every wgpu object before the event
  loop is dropped, not just the surface. S8's `GupApp` must keep this rule.

#### winit paces redraws, not the swapchain

- With `Mailbox`, the frame interval was still 16.66 ms. On Wayland, winit
  delivers `RedrawRequested` on frame callbacks (`pre_present_notify`). An
  uncapped measurement has to drive frames from `about_to_wait` with
  `ControlFlow::Poll`.

#### Counters only prove something if nothing bypasses them

- A source scan (`every_gpu_write_is_counted`) fails if any file but
  `context.rs` calls `queue.write_buffer`, `write_texture`, `create_buffer_init`
  or `create_texture_with_data`, or maps a buffer at creation. The scan exempts
  the test-only compute harness. It is crude, but it turns "the counter is
  complete" from a claim into a check.
- The counters paid off at once: the headless zoom test showed the temporary
  glyph atlas re-uploading all 1 MB whenever a new glyph appeared. It now
  uploads only the dirty rows (2 MB → 18 KB over 300 frames).

### Architectural Decisions

#### `RenderTarget` and `Renderer`/`Prepared` now, not at S3

- **Decision**: AC1 asks for "the same `Renderer`/`Prepared` draw call", so the
  RFC §7 shapes landed here:
  `RenderTarget { desc, acquire, present(cx, frame, CommandBuffer) }`,
  `Renderer::prepare → Prepared`, and `Prepared::draw(&mut RenderPass)`.
  `Prepared::draw` allocates nothing and takes no locks.
- **Reasoning**: a second target would otherwise have duplicated `encode_scene`.
  The trait surfaced one design detail: a readback target submits its copy as a
  second command buffer in the same `submit`.
- **Trade-off**: `TargetDesc` still has no `dpr` (dpr is
  `desc.width / scene.width`). Draw-in-pass for hosts and `TextureTarget` are
  untouched (S3).

#### `Selection` keeps its `LayerGpu`

- **Decision**: when the program, context, column chunk and LUTs are unchanged,
  `prepare` writes the `Encodings` and `Chunk` uniforms into the existing
  buffers and returns the same `Arc<LayerGpu>`.
- **Trade-off**: GPU state is now shared between the scenes of successive
  resolves, so an older `Scene` draws with the newest uniforms. That is the
  right behaviour for a live chart, but it is a semantic worth stating in S3/S7.

#### Zoom is pixel-space with `invert`

- **Decision**: `DynPositionScale::zoom(anchor, factor)` scales the range ends
  about the anchor in pixels and inverts them to a domain. Scales need only
  `invert` and `set_domain`, not their own zoom code.
- **Future**: S9's zoom and pan behaviours, brushing and picking reuse `invert`.

#### Upload counting lives on `Context`

- **Decision**: four kinds (`Column`, `Uniform`, `Instances`, `Texture`),
  relaxed atomics, `UploadStats` with `Sub` for intervals.
- **Reasoning**: the story suggested counting in `ColumnStore::upload`. Counting
  every write proves the stronger claim ("nothing else wrote columns either")
  and exposed the atlas bug.

#### `show` runs once per process and owns the `Plot`

- winit allows one `EventLoop` per process, and `show` uses `with_any_thread` on
  Linux so the parity test can call it from the test harness thread. Hence one
  test per file. Re-entry (`run_app_on_demand`) is left for S8.

### Development Workflow Insights

- I committed at every building increment, as the checkpoint rule asks. The
  `RenderTarget` increment failed the strict gup-core clippy (an unread
  `Frame::surface` field until `WindowTarget` existed), so it went in with the
  window commit rather than on its own. The pre-commit hook passed every time.
- Perl in-place edits with `|` as the delimiter turned `\|` in the _pattern_
  into alternation and spliced code into line 1 of `plot.rs`. Use another
  delimiter, or the Edit tool, when the pattern contains pipes.
- Two release builds (gup-core plus `zoom_bench`, about 1 m 40 s each) were the
  only release work; `/tmp` went from 14 GB to 10 GB free.
- I did not re-run the root `gup` crate's suites or `mask visual-regression`
  (old-path chart builders) because nothing in `gup` changed. gup-core's golden
  goes through the same harness in `scatter_png` and is unchanged.
- gup-core's examples are not in `tests/examples_smoke.rs`'s `PACKAGES`.
  `scatter_window` needs a display even in screenshot mode, so CI could not run
  it, and `zoom_bench` is windowed. Both were run here (`mask gup-core-window`,
  plus a live windowed run).

### Follow-up Stories

No new story files. Everything found here belongs to RFC-001 steps that are not
yet written as stories, or to stories that already exist:

1. **S1**: keep `with_options` and the upload counters. Make
   `every_gpu_write_is_counted` part of the contract, so S4 tail appends count
   as `Upload::Column`. Consider primary-only backends by default (the EGL
   teardown hazard).
2. **S2 (GUP-392)**: `gup-text` should keep the dirty-row/rect atlas upload.
3. **S3**: add `dpr` to `TargetDesc`, `TextureTarget` and draw-in-pass;
   `WindowTarget`'s wasm canvas path is untested.
4. **S5**: re-run `mask gup-core-window` once `Sqrt` exists, with
   `Circle::RADIUS` bound through it (the RFC's full benchmark encoding).
5. **S8**: `GupApp` should start from `gup_core::show` and keep its teardown
   rule. Decide whether `show` must be re-entrant.

## Recommended Next Story

S0 is closed, so the S1–S3 stories can be written and started. **S1 (`Context`
as the single device owner, with old `RenderContext`/`GupContext` shims)** is
the natural next step. It is the root of the dependency chain (S2, S3 and S8 all
need it), and this story has just exercised `Context` against a real window,
`with_options`, the upload counters and the teardown rule. The S1 story should
be written first. GUP-392 (S2, `gup-text` + Inter) is the best parallel
candidate: it is independent of S1's shims and would re-bless the one gup-core
golden.
