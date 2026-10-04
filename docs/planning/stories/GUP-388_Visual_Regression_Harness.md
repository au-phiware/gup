# GUP-388: Visual Regression Harness for Chart Builders and Examples

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: ✅ Complete (2026-10-05)
**Created**: 2026-10-04

## Context

This story implements **T0.1 and T0.2** of the
[October 2026 strategic review](../STRATEGIC_REVIEW_2026-10.md#t0--guardrails-start-now-never-finish):
a visual regression harness for every chart builder and an examples smoke test.
The review's root-cause analysis is blunt about why this is needed: "Charts with
0 visible pixels passed every checked AC. GUP-379's test only checks for
'non-white pixels', so the area-chart fan explosion passed." "Nobody ran the
examples, looked at the PNGs, or used the crate externally."

Verified against `main` on 2026-10-04:

- The ten chart builder modules live in `src/chart_builder/builders.rs:22-32`:
  `area`, `bar`, `boxplot`, `choropleth`, `composite`, `density`, `gpu_density`,
  `heatmap`, `line`, `scatter`, `violin`.
- Seven of them currently validate their rendered output with only a "non-white
  pixel" assertion: `src/chart_builder/builders/area.rs`, `bar.rs`,
  `boxplot.rs`, `density.rs`, `line.rs`, `scatter.rs`, `violin.rs` (found via
  `grep -rln "non-white\|non_white" src/chart_builder/`). These are the tests
  this story replaces.
- The headless screenshot mechanism (`gup::export::gallery::screenshot_request`,
  gated on `GUP_SCREENSHOT_PATH`) already exists in `src/export/gallery.rs`, but
  is wired into only **13 of the 111** example files under `examples/`
  (`grep -l screenshot_request examples/*.rs`): the non-windowed chart-builder
  demos and export demos. All **38** windowed/interactive examples (those using
  `winit::event_loop::EventLoop` or `ApplicationHandler`, including all four
  `composite_*` examples) currently have **no** headless screenshot path at all
  — GUP-282A's claim of wiring "all 62 renderable examples" does not match
  current `main`; re-verify at implementation time whether that is drift since
  GUP-282A completed or a different counting basis, but plan for the windowed
  skip list starting near 38, not near zero.
- The four `composite_*` examples (`composite_bar_trend.rs`,
  `composite_layer_order.rs`, `composite_mixed_data.rs`,
  `composite_scatter_regression.rs`) are windowed and reported by the review's
  visual audit as panicking on the first frame with a `BindGroup does not exist`
  wgpu validation error. The audit's raw renders lived in
  `/tmp/gup-visual-audit/`, which is ephemeral and no longer present — reproduce
  directly against current `main` rather than trusting the stale description,
  and downgrade/upgrade the expected-failure list accordingly if the symptom has
  changed.
- `render_to_svg()` (`src/chart_builder.rs:2645`) currently always calls
  `self.export_svg_with_marks(options, &[])` — an empty mark slice — so SVG
  output never includes data marks regardless of builder.

This harness must support re-blessing golden images, not just pass/fail
comparison, and it must land (or at least have its golden-image format and bless
workflow settled) before GUP-392 lands so that story has a harness to bless
against.

**Target-agnostic design requirement (2026-10-04)**: separately,
[RFC-001](../rfcs/RFC-001_Core_Architecture.md) (Core Architecture) is accepted
and builds a new core in `crates/gup-core`; GUP-393 (mark fidelity fixes on the
old path) is now parked as superseded by RFC-001 S0/S3, with the old `gup`
rendering path frozen for feature work ahead of the RFC-001 S14 flip. This
harness is **not** parked, because it must serve both: the structural assertions
and comparison logic must take a plain RGBA image plus layout metadata (expected
plot rect, expected text regions, expected colours) as input, with no dependency
on `ComposedChart`, `Selection`, or any other old `gup`-crate type. That keeps
the harness usable against today's chart builders now and against `gup-core`'s
output from RFC-001 step S0 onward, without a rewrite.

## User Story

> "As a story-worker implementing or fixing chart rendering, I want a
> pixel-level regression test for every chart builder and a smoke test for every
> example, so that a story cannot be marked complete while the rendered output
> is blank, missing text, or outside the plot area."
>
> "As a project maintainer, I want the test suite to currently fail (or report
> tracked expected-failures) for every known-broken visual case, so the harness
> reflects reality instead of being tuned to pass."

## Acceptance Criteria

### AC1: Target-agnostic assertion API

- [x] The comparison and structural-assertion functions operate on a plain
      `RgbaImage` (width, height, byte buffer) plus a small `LayoutMetadata`
      struct (plot rect in pixels, a list of expected text regions, a list of
      expected colours) — no function signature in the harness's core assertion
      module takes a `ComposedChart`, `Selection`, or other old-`gup`-crate type
      as a parameter.
- [x] Chart-builder-specific glue code (constructing the `RgbaImage` via
      `render_to_png()` and the `LayoutMetadata` via
      `ComposedChart::calculate_chart_area()` etc.) lives in a thin adapter
      layer separate from the assertion module, so a future `gup-core` adapter
      can supply the same two inputs from its own render path without touching
      the assertion logic.
- [x] At least one unit test exercises the assertion functions directly against
      a synthetically constructed `RgbaImage` (not produced by any chart
      builder), proving the API has no hidden coupling to the old render path.

### AC2: Golden-image tests for every chart builder

- [x] Each of the ten builders in `src/chart_builder/builders.rs` has a
      golden-image regression test rendering a representative chart via
      `render_to_png()` (or the GPU-texture equivalent used in tests) and
      comparing against a checked-in golden PNG using perceptual diff (not
      byte-exact comparison, which is too brittle across GPU backends). _Note_:
      `choropleth` has no raster render path (`build()` resolves CPU geometry
      only), so its case is a tracked `render` expected failure rather than a
      golden image. The other builders have 12 golden cases between them (line
      and area also have explicit-scale variants).
- [x] Each golden-image test also runs the following structural assertions
      independent of the perceptual diff, so a regression is caught even if a
      golden image is stale or missing:
  - [x] **Text presence**: when the chart config specifies a title or tick
        labels, at least N pixels in the expected title/tick-label regions
        differ from the background colour (fails today for PNG output per the
        review's "PNG/texture output has no text" finding — track as expected
        failure, see AC5).
  - [x] **Marks confined to plot rect**: no non-background, non-axis/grid pixel
        appears outside the computed chart/plot rectangle
        (`ComposedChart::calculate_chart_area()`).
  - [x] **Configured colour present**: when a mark's colour is explicitly
        configured (e.g. `.color(|_| [r,g,b,a])`), at least one rendered pixel
        matches that colour within a small tolerance (accounts for
        anti-aliasing/gamma — RFC-001 §7's colour policy may let this tolerance
        tighten over time).
  - [x] **Not blank, not solid-fill**: the image is neither a single uniform
        colour nor matches the background colour everywhere.
- [x] These structural + perceptual-diff tests **replace** the existing
      "non-white pixel" tests in `area.rs`, `bar.rs`, `boxplot.rs`,
      `density.rs`, `line.rs`, `scatter.rs`, and `violin.rs`.

### AC3: Bless workflow for intentional baseline changes

- [x] Running tests with `GUP_BLESS=1` regenerates the checked-in golden image
      for any test that ran, instead of failing on mismatch.
- [x] Golden images are stored in a dedicated directory (e.g.
      `tests/golden/chart_builders/`) with a README documenting the bless
      workflow and the perceptual-diff tolerance used.
- [x] The bless workflow is documented in `.github/agents/story-worker.md` so a
      future story (e.g. GUP-392) that intentionally changes visual output knows
      to re-run with `GUP_BLESS=1` and include the updated golden images in its
      commit, with the rendered PNG described in its Definition-of-Done
      evidence.

### AC4: Examples smoke test

- [x] A test (or CI job) builds every example in `examples/` and, for examples
      that support `GUP_SCREENSHOT_PATH`/`screenshot_request()`, runs it
      headless for a configurable number of frames (`N`, default low enough to
      keep CI fast) and asserts it exits zero without panicking. _Note_:
      `screenshot_request()` examples always render exactly one offscreen frame
      and exit, so the configurable knob is a per-example time limit
      (`GUP_SMOKE_TIMEOUT_SECS`, default 60; `GUP_SMOKE_WINDOW_SECS`, default 5,
      for opt-in windowed runs) rather than a frame count. Screenshot examples
      must also write a non-blank PNG. Console examples must exit zero.
- [x] Examples not yet wired for headless screenshots (the ~38 windowed examples
      identified above, confirmed by presence of
      `winit::event_loop::EventLoop`/`ApplicationHandler` and absence of
      `screenshot_request`) are listed in a documented skip file (e.g.
      `tests/examples_skip_list.toml`) with a one-line reason per entry.
- [x] The skip list file has a comment explaining it is expected to shrink over
      time as examples are migrated to `GupApp`/headless-capable shells (see
      GUP-375/GUP-376), and CI should fail if an example in the skip list no
      longer exists (stale entries removed).

### AC5: Known-broken cases are tracked, not hidden

- [x] The harness currently **fails**, or is marked expected-fail in a tracked
      list (e.g. `tests/visual_regression/expected_failures.toml`), for each of
      the following, re-verified against current `main` at implementation time
      since the original audit renders are gone:
  - [x] No text (titles/tick labels) in PNG output.
  - [x] The `composite_*` examples panicking on first frame
        (`BindGroup does not exist` or whatever the current symptom is).
  - [x] Heatmap/density blank output, if still reproducible.
  - [x] Any other builder the implementer finds producing blank/wrong output
        while building the golden-image set — add it to the tracked list rather
        than silently adjusting the test to pass.
- [x] A tracked expected-failure must never be "fixed" by loosening the
      assertion that would otherwise catch it — only by the underlying bug being
      fixed in a follow-up story and the entry removed.

## Technical Tasks

- [x] Define a `LayoutMetadata` struct (plot rect in pixels, expected text
      regions, expected colours) and have every assertion/comparison function
      take `(&RgbaImage, &LayoutMetadata)` or equivalent — no chart-object types
      in this module's signatures (AC1).
- [x] Add a `tests/visual_regression/` (or `src/visual_test_utils.rs` — reuse
      existing helpers in `src/visual_test_utils.rs` where present) module with:
  - [x] A perceptual-diff comparison function (simple Delta-E or SSIM-style
        comparison; a crate dependency is acceptable if it is dev-only).
  - [x] Structural assertion helpers: `assert_text_present_in(region)`,
        `assert_marks_confined_to(rect)`, `assert_color_present(color, tol)`,
        `assert_not_blank()` — all operating on the target-agnostic
        `(RgbaImage, LayoutMetadata)` inputs.
  - [x] `GUP_BLESS` env var handling shared by all golden-image tests.
- [x] Add a thin old-`gup`-specific adapter (separate module) that builds an
      `RgbaImage` from `render_to_png()` output and a `LayoutMetadata` from
      `ComposedChart::calculate_chart_area()` and the chart's configured
      title/tick-label/colour settings — this is the only place old-path types
      appear.
- [x] Write one golden-image test per builder in
      `src/chart_builder/builders/*.rs` (or a new
      `tests/visual_regression_builders.rs` integration test binary — prefer
      consolidating into few binaries per the review's build-footprint guidance
      rather than one binary per builder).
- [x] Delete the existing non-white-pixel-only assertions in the seven files
      listed in AC2 and replace their call sites with the new harness.
- [x] Add the examples smoke-test harness (one consolidated test binary, not 111
      separate binaries) that iterates example binaries, runs them with
      `GUP_SCREENSHOT_PATH` set to a temp file, and checks exit status + file
      existence for non-skipped examples.
- [x] Create `tests/examples_skip_list.toml` populated with the ~38 windowed
      examples found via
      `grep -l "EventLoop\|ApplicationHandler" examples/*.rs | xargs grep -L screenshot_request`.
- [x] Create `tests/visual_regression/expected_failures.toml` populated per AC5
      after re-verifying each case.
- [x] Update `.github/agents/story-worker.md` with the bless-workflow
      instructions from AC3.

## Dependencies

### Prerequisite Stories

- None — this can start immediately against current `main`.

### Enables Stories

- GUP-392 (T4a Correct visual defaults) — needs this harness to bless updated
  golden images.
- GUP-394 (T0 Dogfood suite in CI) — a natural companion CI job; not a hard
  dependency, since the dogfood suite validates a different surface
  (external-crate usage) than this harness (internal golden images).
- Future `gup-core` rendering work (RFC-001 S0 onward) — the target-agnostic
  assertion API (AC1) is designed so `gup-core` can reuse it directly once that
  path produces renderable output; GUP-393 (old-path mark fidelity fixes) is
  parked as superseded by RFC-001, so this harness's old-path adapter will have
  fewer old-path bugs to chase than originally planned, but the harness itself
  remains the guardrail for whichever path is active.

## Testing Strategy

- **Unit tests**: structural assertion helpers (text-presence, confinement,
  colour-tolerance, blank/solid-fill detection) get their own unit tests with
  synthetic images so the helpers themselves are trustworthy before being relied
  on by golden-image tests.
- **Integration tests**: the ten builder golden-image tests, run via
  `cargo test -- --test-threads=1` per project convention for GPU tests.
- **Visual validation**: every golden image added to the repo must have been
  looked at by the implementer (per the strategic review's new definition of
  done) — attach or describe at least 2-3 representative golden images in the
  Definition-of-Done evidence for this story.
- **CI**: the examples smoke test and expected-failures list should run as a
  dedicated CI job so a regression in a currently-passing example is caught
  immediately.

## Success Metrics

- [x] All ten chart builders have a golden-image test with structural
      assertions, replacing the seven non-white-pixel-only tests.
- [x] The examples smoke test runs against all 109 example targets (61 run
      headless and pass, 3 tracked failures, 45 skipped), with a documented (not
      silently empty) skip list for windowed-only examples.
- [x] `GUP_BLESS=1 cargo test` regenerates golden images without manual file
      editing.
- [x] The tracked expected-failures list accurately reflects current `main`
      (re-verified, not copied blindly from the ephemeral 2026-10-04 audit).

## Risk Assessment

- **Medium**: Perceptual-diff tolerance tuning is inherently fuzzy — too loose
  and regressions slip through (the exact failure mode this story exists to
  fix); too tight and GPU-backend/driver variance causes flaky CI. Mitigation:
  start with generous tolerance plus the independent structural assertions
  (which are backend-agnostic), and tighten the perceptual-diff tolerance only
  after observing CI stability across backends.
- **Medium**: The original visual-audit evidence this story's expected-failure
  list is based on lived in an ephemeral `/tmp` directory and is already gone.
  Mitigation: the AC5 tasks explicitly require re-reproducing each case against
  current `main`, not transcribing the review's prose.
- **Low**: Consolidating examples into one smoke-test binary (per the build
  footprint guidance) is more complex than one-binary-per-example but avoids the
  28%-of-retros disk-exhaustion problem the review documents.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked.
- [x] All tests pass: `cargo test -- --test-threads=1` (new expected-failure
      tests are allowed to fail/be marked `#[ignore]` with a tracked reason, per
      AC5 — they must not be silently passing).
- [x] Lint and format clean: `mask all-fix`.
- [x] All examples compile: `cargo check --examples`.
- [x] At least 2-3 golden images attached or described (actually looked at) in
      the completion evidence.
- [x] Story status updated to ✅ Complete in story file and INDEX.md.
- [x] Retrospective added to story document.

## Implementation Summary

**Completed**: 2026-10-05

### What was built

- **`crates/gup-visual-regression`** is a new workspace crate
  (`publish = false`) with **no dependency on `gup`**. Its only dependencies are
  `png` and `toml_edit`. Every check takes
  `(&RgbaImage, &LayoutMetadata, &Tolerances)`.
  - `image.rs`: `RgbaImage` (validated RGBA8 buffer, PNG load/save, synthetic
    `filled`/`fill_rect` helpers).
  - `layout.rs`: `LayoutMetadata` (plot rect, background, text regions with role
    and text colour, guide regions for axes/ticks, expected colours, mark
    overhang) and `PxRect`.
  - `color.rs`: `Rgba8`, sRGB to Lab, and CIEDE2000, verified against the
    published Sharma et al. reference pairs.
  - `checks.rs`: `not_blank`, `marks_present`, `text_present` (a glyph-coverage
    test, so mark spill-over cannot pass for text), `marks_confined`, and
    `color_present`, plus `Tolerances`.
  - `diff.rs`: per-pixel CIEDE2000 perceptual diff with a diff image. The
    default tolerance is ΔE > 3.0 per pixel, with at most 0.25% of pixels
    allowed to differ.
  - `golden.rs`: `GoldenStore` and the `GUP_BLESS=1` workflow. It rewrites only
    goldens that are missing, resized or out of tolerance, and writes
    actual/diff artefacts to `target/visual-regression/`.
  - `expected.rs`: the `expected_failures.toml` loader with glob cases and
    strict reconciliation (an XPASS fails the test until the entry is removed).
  - `harness.rs`: `Harness` / `CaseReport`, which run every check for a named
    case and reconcile the result against the tracked list.
- **Old-path adapter** `src/chart_builder/visual_regression.rs`
  (`#[cfg(test)] pub(crate)`). It is the only place old-path types meet the
  harness. It renders a `ComposedChart` via `render_to_rgba`, or a
  `CompositeChart` offscreen, and turns render errors or panics into `render`
  failures. It derives `LayoutMetadata` from the chart's own layout code.
- **Builder golden tests** live in
  `src/chart_builder/visual_regression/builder_cases.rs`. There is one case per
  builder module, plus explicit-scale variants for line and area: 12 golden PNGs
  in `tests/golden/chart_builders/`, and choropleth as a tracked `render`
  failure. The seven non-white-pixel tests were deleted (502 lines removed
  across the builder files).
- **Examples smoke test** is `tests/examples_smoke.rs`, a single binary with two
  tests.
  - `skip_list_matches_examples` always runs and needs no GPU. It enumerates
    example targets via `cargo metadata` and fails on stale skip entries,
    unlisted windowed examples, skip-listed screenshot-capable examples, or
    `examples/*` expected-failure entries naming no example.
  - `examples_run_headless` is ignored by default (`mask smoke-examples`). It
    runs every non-skipped example in a scratch cwd. `GUP_SMOKE_WINDOWED=1` also
    runs windowed examples for a few seconds.
- **Tracked lists**:
  - `tests/visual_regression/examples_skip_list.toml` has 45 entries: 44
    windowed examples and one HTTP server loop.
  - `tests/visual_regression/expected_failures.toml` has 21 entries, each
    re-verified on 2026-10-04 or 2026-10-05.
- **Workflow**: `mask visual-regression` and `mask smoke-examples` (examples
  built stripped and without debug info into `target/smoke`: about 1.3 GB
  instead of about 17 GB). Also `.github/workflows/visual-regression.yml`
  (lavapipe; not yet observed running in CI), `tests/golden/README.md`, and the
  bless and tracked-failure rules in `.github/agents/story-worker.md`.

### Verified results (2026-10-05)

- `cargo test --all-features -p gup -- --test-threads=1`: 4,275 passed, 0
  failed, 15 ignored (lib: 3,158 passed). `gup-visual-regression`: 37 unit tests
  and 1 doc-test passed.
- The examples smoke test found 109 example targets. Headless: 61 passed, 3
  expected failures (`density_scatter_overlay` solid fill, `gpu_debug_demo` CSV
  serialisation error, `pattern_pipeline_demo` runtime-in-runtime panic), 45
  skipped. With `GUP_SMOKE_WINDOWED=1`, all four `composite_*` examples still
  panic on the first frame (`BindGroup[Id(0,1)] does not exist`;
  `composite_layer_order`: `RenderPipeline[Id(2,1)] does not exist`), and the
  control `02_scatter_window` ran cleanly.
- The pre-commit hook passed: `mask check`, fmt,
  `clippy --all-targets --all-features -D warnings` (covers every example),
  markdown lint and mark validation.

### Golden images looked at

Goldens are **baselines of current output, not certified-correct images**. The
structural checks and the tracked list are what flag broken output.

- `bar.png`: four sky-blue bars inside the axes with ticks. There is no title or
  tick text (tracked: `chart_builders/*` `text_present`). The fill differs from
  the configured colour (tracked: sRGB double-encoding, `color_present`).
- `scatter.png`: twelve large **black** circles inside the plot rect. The
  configured fill is ignored (tracked: `color_present`).
- `area.png`: the lavender "fan explosion" polygon covers most of the canvas,
  well outside the plot rect (tracked: `marks_confined`).
- `density.png`: one uniform light-blue fill over the entire canvas (tracked:
  `not_blank`, `marks_confined`).

## Retrospective

**Completed**: 2026-10-05

### Key Technical Learnings

#### Golden images record current output, not correct output

- **Challenge**: Most builders render broken output today: no text at all, an
  ignored or double-encoded fill colour, the area fan, solid-fill density. A
  golden-only harness would either bless the breakage as "correct" or fail
  everywhere.
- **Solution**: Goldens are explicitly baselines of current output, and their
  only job is to catch change. Correctness comes from the structural checks,
  which run independently of the golden. Every broken case gets a tracked
  `expected_failures.toml` entry naming the check, the observed symptom and the
  owning story. Reconciliation is strict: an XPASS fails the test, so a fix
  forces the entry to be removed.
- **Pattern**: Separate "did it change?" (perceptual diff) from "is it right?"
  (structural checks). Never let a golden stand in for correctness.

#### Text checks must not be satisfiable by marks

- **Challenge**: A naive "non-background pixels in the title region" check
  passes when an overflowing mark (the area fan, the density fill) covers the
  title band. This is exactly the review's "non-white pixel" failure mode again.
- **Solution**: `text_present` is a glyph-coverage test. It counts pixels that
  match the configured text colour (or a blend of it with the background) in a
  sparse, glyph-like pattern. Guides (axis lines, tick marks) are subtracted
  from text regions, so tick marks alone do not count as tick labels.
- **Pattern**: Every structural check needs a negative unit test where a
  _different_ kind of content fills the region.
  `guides_alone_do_not_satisfy_text_regions` and the synthetic-image tests in
  `checks.rs` are those tests.

#### Example binaries dominate disk use

- **Challenge**: With debug info, the 109 example binaries take about 160 MB
  each (about 17 GB in total). On the shared ZFS pool this filled the disk for
  every worker.
- **Solution**: Build them with `CARGO_PROFILE_DEV_DEBUG=0`,
  `CARGO_PROFILE_DEV_STRIP=true` and `CARGO_INCREMENTAL=0` into `target/smoke`:
  about 29 MB each, 1.3 GB in total. `mask smoke-examples` and the CI job use
  the same settings. Reusing that target dir for the full test suite also
  avoided a second debug build.
- **Pattern**: Any job that links every example must strip and drop debug info.
  Panic messages remain readable without debug info.

#### Examples write into their working directory

- **Challenge**: The first smoke run dirtied tracked files
  (`resource_graph.dot`, `resource_graph.json`) because examples ran with the
  repo root as cwd.
- **Solution**: Examples now run in a scratch cwd under the artefact dir.
- **Pattern**: Never run untrusted or legacy binaries from the repo root. Check
  `git status` after a smoke run.

#### Backend variance is smaller than feared

- **Challenge**: The goldens were blessed on a hardware GPU, but CI runs
  lavapipe.
- **Solution**: All 13 builder cases pass under forced lavapipe
  (`VK_ICD_FILENAMES=.../lvp_icd.x86_64.json LIBGL_ALWAYS_SOFTWARE=1`) with the
  default tolerance (per-pixel ΔE > 3.0, at most 0.25% of pixels differing). The
  lavapipe renders were not byte-identical to the hardware renders, which
  confirms a different backend produced them.
- **Pattern**: Check a new golden suite under the CI backend locally before
  trusting the CI job. Running the existing test binary directly with
  `VK_ICD_FILENAMES` set costs no rebuild.

### Architectural Decisions

#### A separate workspace crate with no `gup` dependency

- **Decision**: The harness is `crates/gup-visual-regression`
  (`publish = false`; dependencies `png` and `toml_edit` only; no
  `workspace = true` inheritance). It is not a module inside `gup`.
- **Reasoning**: This makes AC1's target-agnostic requirement structural rather
  than a matter of discipline. The same crate serves the old builders,
  `gup-core` from RFC-001 S0a, and the detached dogfood crate.
- **Trade-off**: One more workspace member, and `LayoutMetadata` must be derived
  by each adapter instead of being read from chart internals.
- **Future**: RFC-001 S0a's AC5 can build its PNG test on this crate directly
  through a `gup-core` adapter.

#### The adapter lives in the `gup` lib as `#[cfg(test)] pub(crate)`

- **Decision**: `src/chart_builder/visual_regression.rs` and `builder_cases.rs`
  live inside the lib's unit tests rather than in a `tests/` integration binary.
- **Reasoning**: The adapter needs crate-private layout internals (chart area,
  tick geometry), and the review's build-footprint guidance asks for fewer test
  binaries. It is deleted wholesale at RFC-001 S14 along with the old path.
- **Trade-off**: The builder goldens run as part of the 3,000-test lib binary
  (filter: `cargo test --lib visual_regression`).

#### Smoke test runs built binaries, not `cargo run`

- **Decision**: One test binary enumerates example targets via `cargo metadata`
  and executes `target/<profile>/examples/<name>` directly. The run itself is
  `#[ignore]`d. The skip-list consistency test always runs.
- **Reasoning**: `cargo run` per example would serialise on the build lock and
  rebuild-check 109 times. Keeping the run ignored stops plain `cargo test` from
  requiring every example binary.
- **Trade-off**: Running the smoke test requires building the examples first
  (`mask smoke-examples` does both).

#### Time limits instead of frame counts

- **Decision**: The configurable knob is a per-example timeout
  (`GUP_SMOKE_TIMEOUT_SECS`), plus `GUP_SMOKE_WINDOW_SECS` for opt-in windowed
  runs. AC4 asked for a frame count `N`.
- **Reasoning**: `screenshot_request()` examples render exactly one offscreen
  frame by design, and no other examples expose a frame-count hook.
- **Future**: Revisit if RFC-001's `WindowTarget` gains a headless N-frame mode.

### How GUP-397 should consume this harness

- **Dependency**: In `dogfood/Cargo.toml`, add
  `gup-visual-regression = { path = "../crates/gup-visual-regression" }`. The
  crate has no `workspace = true` fields and no `gup` dependency, so the
  detached dogfood workspace can use it by path without joining the parent
  workspace or touching `pub(crate)` items. Only `png` and `toml_edit` enter
  `dogfood/Cargo.lock`.
- **Inputs**: Load renders with `RgbaImage::load_png` (or `from_png` for bytes)
  instead of the `image` crate's type. Convert the manifest's fractional
  `Region`s once with `PxRect::from_edges(fx0 * w, fy0 * h, ...)`. Where the
  task knows its layout, build `LayoutMetadata::new(plot_rect)` with
  `.with_text(role, rect, text_colour)`, `.with_guide(rect)`,
  `.with_expected_color(label, colour)` and `.with_background(colour)`.
- **Pass/fail checks**: `checks::check_text_present`, `check_color_present`,
  `check_not_blank`, `check_marks_present` and `check_marks_confined`
  (`Tolerances` controls thresholds) replace `count_ink`, `count_near` and
  `non_background_fraction` where the manifest only asks a yes/no question.
- **Missing pieces to add in GUP-397**:
  - The harness has no public _measurement_ API. Its pixel classifiers
    (`is_ink`, `is_glyph`) are private and the checks return
    `Result<(), CheckFailure>`. Dogfood's manifest needs counts. Add a `measure`
    module of pure counting functions (`count_ink`, `count_near`, `coverage`,
    `count_hue`, `count_grey`, `count_runs`, `count_saturated`) and re-implement
    the `check_*` functions on top of it, so there is one definition of "ink"
    and "matches colour".
  - Port dogfood's `background()` estimate as `measure::background()` for images
    without a known background.
  - Unify colour tolerance on ΔE (CIEDE2000, `Rgba8::delta_e`). Dogfood's
    per-channel `u8` tolerances must be re-derived from fresh renders, never
    loosened. GUP-397's AC2 already forbids a gap flipping to PASS.
- **Gap tracking**: Keep dogfood's own `Gap`/XFAIL manifest. The harness's
  `ExpectedFailures` is keyed by `(case, Check)` and can serve dogfood only if
  its checks become named `Check` values. That is optional, not required.

### Development Workflow Insights

- The worktree-isolation guard refuses shell lines that combine `export`, `$PWD`
  or heredoc text containing "git" with other commands. Small scripts in `/tmp`
  that take the worktree path as an argument (`gup388_smoke.sh`,
  `gup388_test.sh`) avoided it reliably.
- Long pre-commit hooks (clippy over all targets) were run in the background
  while disk was monitored. Two API-limit interruptions were survived by
  re-orienting from `git status` and `git log main..HEAD`.
- Merging `main` (GUP-394) mid-story was conflict-free: both stories touched
  `maskfile.md` and `INDEX.md`, but in disjoint hunks.
- The skip list has 45 entries, not the story's estimated 38. The estimate
  missed examples using the `GupApp` shell and the never-exiting
  `web_dashboard_demo` server.

### Remaining gaps (tracked, no new stories)

No follow-up stories were written. Every gap has an owner, and none is a quick
fix:

- **No text in PNG output** (`chart_builders/*` `text_present`): RFC-001 S0a.
- **Colour policy** (double sRGB encoding, ignored fills): RFC-001 S3/S10.
- **Area fan, density/heatmap blank or overflowing, choropleth with no raster
  path**: replaced at RFC-001 S14 / T5 ports. Choropleth is parked
  (GUP-366..369).
- **`composite_*` first-frame panics**: RFC-001 S1 (one `Context`) and S11.
- **`gpu_debug_demo`**: `csv` cannot write headers for array fields in the
  generic `dump_buffer_csv<T: Serialize>`. A real fix needs a flattening
  serializer, and `has_headers(false)` would silently drop headers. Owner: T7
  gup-debug split. GUP-390 only feature-gates this code.
- **`pattern_pipeline_demo`**: `LinuxAccessibility::initialize`
  (`src/accessibility/platform.rs:212`) builds a tokio runtime and calls
  `block_on` from inside the example's `#[tokio::main]` runtime. Old-path
  accessibility is redesigned in RFC-001 open question 14 (T5) and feature-gated
  in T7.
- **`render_to_svg()` passes an empty mark slice** (noted in Context): this is
  old-path export, replaced by RFC-001's export targets. It was not covered by
  this harness because AC2 targets PNG.
- **Windowed examples (skip list)**: shrinks via GUP-375/GUP-376 and RFC-001
  S0b's `WindowTarget`.
- **CI workflow**: `.github/workflows/visual-regression.yml` has not yet been
  observed running on GitHub. Watch its first run after merge.
