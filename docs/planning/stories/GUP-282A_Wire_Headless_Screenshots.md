# GUP-282A: Wire Headless Screenshots Into Examples

## Story Overview

**Initiative**: Documentation **Status**: 🚧 In Progress **Created**: 2025-07-26

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

- [ ] All 62 examples classified as `skip = false` in
      `scripts/gallery_config.toml` check
      `gup::export::gallery::screenshot_request()` at an appropriate point in
      their execution.
- [ ] When `GUP_SCREENSHOT_PATH` is set, each example renders one frame
      offscreen, writes a PNG to the specified path, and exits with code 0.
- [ ] When the variable is not set, examples behave exactly as before (no
      regression).
- [ ] `scripts/generate_gallery.sh` produces a non-placeholder thumbnail for
      every non-skipped example.
- [ ] The generated `docs/gallery/index.html` displays all thumbnails correctly.

## Technical Tasks

- [ ] For each renderable example, add a check near the top of `main()`:
      `rust     if let Some(req) = gup::export::gallery::screenshot_request() {         // build chart / context...         chart.export_png(&req.path, req.width, req.height)?;         return Ok(());     }     `
- [ ] For examples that use `ComposedChart` (6 examples): the integration is
      straightforward — call `export_png` on the chart.
- [ ] For console-only examples that were misclassified: move them to
      `skip = true` in the config.
- [ ] Run `scripts/generate_gallery.sh` end-to-end and verify all thumbnails.
- [ ] Regenerate `docs/gallery/index.html` and visually verify in a browser.

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

- [ ] All non-skipped examples produce real thumbnails
- [ ] Gallery HTML shows thumbnails instead of placeholders
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Story status updated to ✅ Complete
