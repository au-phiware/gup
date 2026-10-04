# GUP-388: Visual Regression Harness for Chart Builders and Examples

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 📋 Planned **Created**:
2026-10-04

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

- [ ] The comparison and structural-assertion functions operate on a plain
      `RgbaImage` (width, height, byte buffer) plus a small `LayoutMetadata`
      struct (plot rect in pixels, a list of expected text regions, a list of
      expected colours) — no function signature in the harness's core assertion
      module takes a `ComposedChart`, `Selection`, or other old-`gup`-crate type
      as a parameter.
- [ ] Chart-builder-specific glue code (constructing the `RgbaImage` via
      `render_to_png()` and the `LayoutMetadata` via
      `ComposedChart::calculate_chart_area()` etc.) lives in a thin adapter
      layer separate from the assertion module, so a future `gup-core` adapter
      can supply the same two inputs from its own render path without touching
      the assertion logic.
- [ ] At least one unit test exercises the assertion functions directly against
      a synthetically constructed `RgbaImage` (not produced by any chart
      builder), proving the API has no hidden coupling to the old render path.

### AC2: Golden-image tests for every chart builder

- [ ] Each of the ten builders in `src/chart_builder/builders.rs` has a
      golden-image regression test rendering a representative chart via
      `render_to_png()` (or the GPU-texture equivalent used in tests) and
      comparing against a checked-in golden PNG using perceptual diff (not
      byte-exact comparison, which is too brittle across GPU backends).
- [ ] Each golden-image test also runs the following structural assertions
      independent of the perceptual diff, so a regression is caught even if a
      golden image is stale or missing:
  - [ ] **Text presence**: when the chart config specifies a title or tick
        labels, at least N pixels in the expected title/tick-label regions
        differ from the background colour (fails today for PNG output per the
        review's "PNG/texture output has no text" finding — track as expected
        failure, see AC5).
  - [ ] **Marks confined to plot rect**: no non-background, non-axis/grid pixel
        appears outside the computed chart/plot rectangle
        (`ComposedChart::calculate_chart_area()`).
  - [ ] **Configured colour present**: when a mark's colour is explicitly
        configured (e.g. `.color(|_| [r,g,b,a])`), at least one rendered pixel
        matches that colour within a small tolerance (accounts for
        anti-aliasing/gamma — RFC-001 §7's colour policy may let this tolerance
        tighten over time).
  - [ ] **Not blank, not solid-fill**: the image is neither a single uniform
        colour nor matches the background colour everywhere.
- [ ] These structural + perceptual-diff tests **replace** the existing
      "non-white pixel" tests in `area.rs`, `bar.rs`, `boxplot.rs`,
      `density.rs`, `line.rs`, `scatter.rs`, and `violin.rs`.

### AC3: Bless workflow for intentional baseline changes

- [ ] Running tests with `GUP_BLESS=1` regenerates the checked-in golden image
      for any test that ran, instead of failing on mismatch.
- [ ] Golden images are stored in a dedicated directory (e.g.
      `tests/golden/chart_builders/`) with a README documenting the bless
      workflow and the perceptual-diff tolerance used.
- [ ] The bless workflow is documented in `.github/agents/story-worker.md` so a
      future story (e.g. GUP-392) that intentionally changes visual output knows
      to re-run with `GUP_BLESS=1` and include the updated golden images in its
      commit, with the rendered PNG described in its Definition-of-Done
      evidence.

### AC4: Examples smoke test

- [ ] A test (or CI job) builds every example in `examples/` and, for examples
      that support `GUP_SCREENSHOT_PATH`/`screenshot_request()`, runs it
      headless for a configurable number of frames (`N`, default low enough to
      keep CI fast) and asserts it exits zero without panicking.
- [ ] Examples not yet wired for headless screenshots (the ~38 windowed examples
      identified above, confirmed by presence of
      `winit::event_loop::EventLoop`/`ApplicationHandler` and absence of
      `screenshot_request`) are listed in a documented skip file (e.g.
      `tests/examples_skip_list.toml`) with a one-line reason per entry.
- [ ] The skip list file has a comment explaining it is expected to shrink over
      time as examples are migrated to `GupApp`/headless-capable shells (see
      GUP-375/GUP-376), and CI should fail if an example in the skip list no
      longer exists (stale entries removed).

### AC5: Known-broken cases are tracked, not hidden

- [ ] The harness currently **fails**, or is marked expected-fail in a tracked
      list (e.g. `tests/visual_regression/expected_failures.toml`), for each of
      the following, re-verified against current `main` at implementation time
      since the original audit renders are gone:
  - [ ] No text (titles/tick labels) in PNG output.
  - [ ] The `composite_*` examples panicking on first frame
        (`BindGroup does not exist` or whatever the current symptom is).
  - [ ] Heatmap/density blank output, if still reproducible.
  - [ ] Any other builder the implementer finds producing blank/wrong output
        while building the golden-image set — add it to the tracked list rather
        than silently adjusting the test to pass.
- [ ] A tracked expected-failure must never be "fixed" by loosening the
      assertion that would otherwise catch it — only by the underlying bug being
      fixed in a follow-up story and the entry removed.

## Technical Tasks

- [ ] Define a `LayoutMetadata` struct (plot rect in pixels, expected text
      regions, expected colours) and have every assertion/comparison function
      take `(&RgbaImage, &LayoutMetadata)` or equivalent — no chart-object types
      in this module's signatures (AC1).
- [ ] Add a `tests/visual_regression/` (or `src/visual_test_utils.rs` — reuse
      existing helpers in `src/visual_test_utils.rs` where present) module with:
  - [ ] A perceptual-diff comparison function (simple Delta-E or SSIM-style
        comparison; a crate dependency is acceptable if it is dev-only).
  - [ ] Structural assertion helpers: `assert_text_present_in(region)`,
        `assert_marks_confined_to(rect)`, `assert_color_present(color, tol)`,
        `assert_not_blank()` — all operating on the target-agnostic
        `(RgbaImage, LayoutMetadata)` inputs.
  - [ ] `GUP_BLESS` env var handling shared by all golden-image tests.
- [ ] Add a thin old-`gup`-specific adapter (separate module) that builds an
      `RgbaImage` from `render_to_png()` output and a `LayoutMetadata` from
      `ComposedChart::calculate_chart_area()` and the chart's configured
      title/tick-label/colour settings — this is the only place old-path types
      appear.
- [ ] Write one golden-image test per builder in
      `src/chart_builder/builders/*.rs` (or a new
      `tests/visual_regression_builders.rs` integration test binary — prefer
      consolidating into few binaries per the review's build-footprint guidance
      rather than one binary per builder).
- [ ] Delete the existing non-white-pixel-only assertions in the seven files
      listed in AC2 and replace their call sites with the new harness.
- [ ] Add the examples smoke-test harness (one consolidated test binary, not 111
      separate binaries) that iterates example binaries, runs them with
      `GUP_SCREENSHOT_PATH` set to a temp file, and checks exit status + file
      existence for non-skipped examples.
- [ ] Create `tests/examples_skip_list.toml` populated with the ~38 windowed
      examples found via
      `grep -l "EventLoop\|ApplicationHandler" examples/*.rs | xargs grep -L screenshot_request`.
- [ ] Create `tests/visual_regression/expected_failures.toml` populated per AC5
      after re-verifying each case.
- [ ] Update `.github/agents/story-worker.md` with the bless-workflow
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

- [ ] All ten chart builders have a golden-image test with structural
      assertions, replacing the seven non-white-pixel-only tests.
- [ ] The examples smoke test runs against all 111 examples, with a documented
      (not silently empty) skip list for windowed-only examples.
- [ ] `GUP_BLESS=1 cargo test` regenerates golden images without manual file
      editing.
- [ ] The tracked expected-failures list accurately reflects current `main`
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

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1` (new expected-failure
      tests are allowed to fail/be marked `#[ignore]` with a tracked reason, per
      AC5 — they must not be silently passing).
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] At least 2-3 golden images attached or described (actually looked at) in
      the completion evidence.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
