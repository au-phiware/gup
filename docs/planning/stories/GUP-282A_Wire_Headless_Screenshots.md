# GUP-282A: Wire Headless Screenshots Into Examples

## Story Overview

**Initiative**: Documentation **Status**: ✅ Complete **Created**: 2025-07-26
**Completed**: 2025-07-27

## Context

GUP-282 delivered the gallery infrastructure — config, scripts, CSS, CI
workflow, and HTML generator — but the thumbnail generation pipeline depends on
each example detecting the `GUP_SCREENSHOT_PATH` environment variable and
exporting a single frame via the PNG Export API (GUP-268). Currently no examples
check this variable, so the gallery shows placeholder cards for all 62
renderable examples.

This story wires the headless screenshot support into every renderable example
so that `scripts/generate_gallery.sh` can produce actual PNG thumbnails.

## User Story

> "As a contributor running the gallery generation script, I want every
> renderable example to produce a real PNG thumbnail so that the gallery shows
> actual chart screenshots instead of placeholder boxes."

## Acceptance Criteria

- [x] All examples classified as `skip = false` in `scripts/gallery_config.toml`
      check `gup::export::gallery::screenshot_request()` at an appropriate point
      in their execution. (18 renderable examples after audit; 44 console-only,
      windowed-only, WASM-only, or feature-gated examples reclassified as skip.)
- [x] When `GUP_SCREENSHOT_PATH` is set, each example renders one frame
      offscreen, writes a PNG to the specified path, and exits with code 0.
- [x] When the variable is not set, examples behave exactly as before (no
      regression).
- [x] `scripts/generate_gallery.sh` produces a non-placeholder thumbnail for
      every non-skipped example.
- [x] The generated `docs/gallery/index.html` displays all thumbnails correctly.

## Technical Tasks

- [x] For each renderable example, add a check near the top of `main()`:
      `rust     if let Some(req) = gup::export::gallery::screenshot_request() {         // build chart / context...         chart.export_png(&req.path, req.width, req.height)?;         return Ok(());     }     `
- [x] For examples that use `ComposedChart` (19 examples): the integration is
      straightforward — call `export_png` on the chart.
- [x] For console-only examples that were misclassified: move them to
      `skip = true` in the config.
- [x] Run `scripts/generate_gallery.sh` end-to-end and verify all thumbnails.
- [x] Regenerate `docs/gallery/index.html` and visually verify.

## Dependencies

### Prerequisite Stories

- GUP-282: Example Gallery ✅ — provides the gallery infrastructure, config, and
  `screenshot_request()` helper.
- GUP-268: PNG Export ✅ — provides `export_png` / `render_to_png`.

## Testing Strategy

- Run `scripts/generate_gallery.sh` and verify it exits 0 with all thumbnails
  produced.
- Verify each thumbnail is a valid PNG > 0 bytes.
- Visual spot-check of 10+ thumbnails in the gallery HTML.

## Risk Assessment

- **Medium**: Some examples may not produce meaningful visual output in headless
  mode (e.g. animation examples that need multiple frames). Mark these as
  CI-skip if the first frame is blank.
- **Low**: Time cost — modifying 62 examples is mechanical but time-consuming.

## Definition of Done

- [x] All non-skipped examples produce real thumbnails
- [x] Gallery HTML shows thumbnails instead of placeholders
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] Story status updated to ✅ Complete

## Implementation Summary

### What Was Implemented

1. **Gallery config audit**: Reclassified 44 examples from `skip=false` to
   `skip=true` with documented skip reasons. Categories:
   - Console-only (no GPU rendering): 38 examples
   - Windowed-only (requires EventLoop): 3 examples
   - WASM-only: 1 example
   - Feature-gated (`pdf` feature): 1 example
   - Pre-existing skip entries: 37 examples (unchanged)

2. **Screenshot support**: Wired `gup::export::gallery::screenshot_request()`
   into all 18 renderable examples. Each checks the env var after GPU context
   init, builds one representative `ComposedChart`, exports PNG, and returns
   early.

3. **Gallery script fix**: Fixed a pre-existing awk parser bug in
   `scripts/generate_gallery.sh` where the `[[examples]]` section detection rule
   reset state before the emit rule could fire.

4. **Gallery regeneration**: Regenerated `docs/gallery/index.html` and verified
   all 18 thumbnails are valid PNGs.

### Key Files Changed

- `scripts/gallery_config.toml` — 44 examples reclassified as skip
- `scripts/generate_gallery.sh` — awk parser bug fix
- `docs/gallery/index.html` — regenerated with 18 renderable examples
- 18 example files — screenshot support added:
  - `examples/basic/03_line_chart.rs`
  - `examples/basic/04_bar_chart.rs`
  - `examples/showcase/business_dashboard.rs`
  - `examples/boxplot_builder_demo.rs`
  - `examples/multi_category_boxplot.rs`
  - `examples/observable_plot_showcase.rs`
  - `examples/bar_chart.rs`
  - `examples/line_chart_demo.rs`
  - `examples/area_chart_demo.rs`
  - `examples/heatmap_chart.rs`
  - `examples/violin_plot_demo.rs`
  - `examples/density_scatter_overlay.rs`
  - `examples/export_png.rs`
  - `examples/svg_export.rs`
  - `examples/html_export.rs`
  - `examples/pdf_export.rs`
  - `examples/intermediate/styled_scatter.rs`
  - `examples/intermediate/multi_series_line.rs`
  - `examples/intermediate/categorical_bar.rs`

### Test Results

- All cargo tests pass: 391+ passed, 0 failed
- All 18 gallery thumbnails generated successfully (7.2–7.5 KB each)
- Normal example execution unaffected (no regression)

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Example Classification Is More Nuanced Than Expected

- **Challenge**: The story assumed 62 "renderable" examples, but many were
  console-only demos that print statistics, validate APIs, or generate non-PNG
  output (DOT, JSON, SVG/HTML files). Only 18 of 62 actually use `ComposedChart`
  and can produce PNG screenshots.
- **Solution**: Audited all 62 examples by checking for `ComposedChart`,
  `build_with_data`, `winit`, and GPU rendering patterns. Reclassified 44
  examples as `skip=true` with documented reasons.
- **Pattern**: When planning gallery/screenshot stories, first audit the example
  landscape to determine how many are actually renderable. The example count in
  the config is not a reliable indicator.

#### Builder API Returns ComposedChart, Not Selection

- **Challenge**: Many examples assign `build_with_data()` results to variables
  named `selection`, making it appear they return `Selection`. In fact, all
  builder types (`line()`, `bar()`, `scatter()`, `area()`, `heatmap()`,
  `boxplot()`, `violin()`, `density_plot()`) return `ComposedChart` which
  supports `export_png`.
- **Solution**: Verified builder output types via `type Output` declarations in
  the builder trait implementations.
- **Pattern**: Check the `type Output` on `ChartBuilder` implementations to
  determine what capabilities are available on builder results.

#### Pre-existing Awk Parser Bug in Gallery Script

- **Challenge**: `generate_gallery.sh` had a bug where the `[[examples]]`
  section-start awk rule reset state BEFORE the emit rule could fire, so only
  the last entry in the config was ever parsed.
- **Solution**: Moved the emit logic into the section-start rule (emit previous
  entry, then reset), matching the pattern already used correctly in
  `generate_gallery_html.sh`.
- **Pattern**: When awk has multiple rules that match the same line, execution
  order matters. Combine dependent logic into a single rule block rather than
  splitting across rules.

### Architectural Decisions

#### Reclassify Rather Than Force-Fit Screenshots

- **Decision**: Reclassified 44 examples as `skip=true` rather than trying to
  add artificial screenshot support to console-only examples.
- **Reasoning**: Producing a meaningful thumbnail from an example that only
  prints statistics would require creating an entirely new chart that doesn't
  reflect what the example actually does. This would be misleading.
- **Trade-off**: Gallery shows fewer thumbnails (18 vs 62) but each thumbnail
  accurately represents the example's visual output.
- **Future**: As more examples gain `ComposedChart` support (e.g., windowed
  examples migrated to `GupApp`), they can be reclassified back to `skip=false`.

#### Screenshot After Context Init, Not At Top of Main

- **Decision**: Insert screenshot check immediately after `RenderContext::new()`
  and data creation, not at the very top of `main()`.
- **Reasoning**: The screenshot needs a GPU context and data to build the chart.
  Checking too early (before context init) would require restructuring the
  example to separate context init from the rest of the logic.
- **Trade-off**: Slight code duplication (chart builder config repeated in
  screenshot block and normal flow) but maintains readability.

### Development Workflow Insights

- The mechanical nature of this story (19 similar edits) benefited from
  batch-analysing the example landscape first, then applying a consistent
  pattern. The pre-analysis phase (categorising 62 examples) took longer than
  the actual edits.
- `mask all-fix` caught minor formatting differences in the screenshot blocks
  (rustfmt preferences for closure formatting).
- Running `generate_gallery.sh` end-to-end was the most valuable validation — it
  caught the pdf_export feature-gate issue that individual checks missed.

### Follow-up Stories

1. **GUP-318: Migrate Examples to GupApp** — When windowed examples are migrated
   to the headless-capable `GupApp` framework, they can be reclassified as
   `skip=false` and wired for gallery screenshots. This would increase the
   renderable count significantly.
