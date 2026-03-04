# GUP-282: Example Gallery

## Story Overview

**Initiative**: Documentation **Status**: 🚧 In Progress **Created**: 2025-01-09

## Context

GUP-103 delivered a comprehensive examples suite — more than 60 runnable
examples spanning chart types, interactions, animations, text rendering, and
performance. These examples are indexed in `examples/INDEX.md` with text
descriptions, but there is currently no visual entry point. A prospective user
must read prose, clone the repository, and run each example individually to see
what it looks like. That friction is a meaningful barrier to adoption.

A visual gallery closes this gap: each example is run headlessly, its output
captured as a PNG thumbnail (enabled by GUP-268's PNG Export API), and the
thumbnails are assembled into a navigable HTML gallery page. New users can scan
the gallery visually, spot a chart that matches their use case, and jump
directly to its source — dramatically shortening time-to-first-chart. The
gallery also functions as a continuous regression check: if an example renders
differently after a code change, the diff appears immediately in CI-generated
thumbnails.

GUP-280 (API Reference Generation) and GUP-281 (Tutorial and Guide Suite) are
being developed in parallel and will link into the same GitHub Pages deployment
that hosts this gallery, forming a coherent documentation site. This story
scopes the gallery page and its CI-driven thumbnail pipeline; the broader site
integration is handled by those companion stories.

## User Story

> "As a new user, I want to browse rendered screenshots of every example in a
> visual gallery so that I can quickly find a chart that matches my use case and
> jump to its source code."

> "As a contributor, I want the CI pipeline to regenerate gallery thumbnails on
> every merge to main so that the gallery always reflects the current state of
> the codebase and visual regressions are immediately visible."

## Acceptance Criteria

### AC1: Thumbnail generation script

- [ ] A script (e.g. `scripts/generate_gallery.sh` or a Cargo binary under
      `tools/`) accepts an optional list of example names; when none are
      supplied it processes all examples listed in `examples/INDEX.md`.
- [ ] The script invokes each eligible example in headless/offscreen mode and
      calls the PNG Export API (GUP-268) to save a 640×480 thumbnail to
      `docs/gallery/thumbs/<example_name>.png`.
- [ ] Examples that require an interactive window or do not produce a stable
      first frame (e.g. purely console examples such as `01_hello_chart`) are
      explicitly skipped; the skip list is maintained in a config file.
- [ ] The script exits non-zero if any non-skipped example fails to produce a
      thumbnail.

### AC2: Gallery HTML page

- [ ] `docs/gallery/index.html` is generated (or checked in as a static page)
      containing every thumbnail with its title, one-line description, and a
      link to the example's source file on GitHub.
- [ ] Examples are grouped into the following categories, matching the sections
      in `examples/INDEX.md`: **Chart Types**, **Axis & Grid**, **Text
      Rendering**, **Interaction & Selection**, **Animation**, **Patterns &
      Blending**, **Performance**, **Custom Marks**, **Showcase**.
- [ ] The page is navigable without JavaScript (pure HTML + CSS); JavaScript
      enhancements (e.g. live search/filter) are optional.
- [ ] Thumbnails are lazy-loaded (`loading="lazy"`) so the page remains
      performant with 60+ images.

### AC3: CI job

- [ ] A GitHub Actions workflow (`.github/workflows/gallery.yml` or a new job in
      an existing workflow) runs on every push to `main`.
- [ ] The job builds Gup in release mode, runs the thumbnail generation script,
      generates the gallery page, and deploys the result to the `gh-pages`
      branch (or equivalent Pages source).
- [ ] The job caches the `target/` directory and previously-generated thumbnails
      so unchanged examples do not re-render on every run.
- [ ] If thumbnail generation fails for any example, the CI job fails and the
      deployment is skipped.

### AC4: Documentation cross-linking

- [ ] `examples/INDEX.md` links to the live gallery page in its header.
- [ ] `docs/README.md` mentions the gallery under a new "Visual Gallery" entry
      in Quick Navigation.
- [ ] Each gallery item links to the example's `.rs` source file; links are
      validated (no 404s) in CI.

## Technical Tasks

- [ ] Survey all examples in `examples/INDEX.md` and classify each as
      "renderable" (produces a GPU frame) or "skip" (console-only,
      interactive-only, or tool); record the skip list in
      `scripts/gallery_config.toml` (or equivalent).
- [ ] Implement headless render mode for examples: add a
      `--headless-screenshot <path>` flag (or environment variable) that makes
      an example render one frame offscreen and export it via the GUP-268 PNG
      API, then exit cleanly.
- [ ] Write `scripts/generate_gallery.sh` (or a Rust binary) that iterates the
      example list, invokes each with the headless flag, and writes thumbnails
      to `docs/gallery/thumbs/`.
- [ ] Write a gallery page generator (shell/Python/Rust) that reads
      `examples/INDEX.md` and the thumbnails directory and emits
      `docs/gallery/index.html`.
- [ ] Design a minimal CSS stylesheet (`docs/gallery/gallery.css`) with a
      responsive grid layout, category headings, and a caption per thumbnail.
- [ ] Add `.github/workflows/gallery.yml` with: checkout, Rust toolchain setup,
      dependency cache, build, thumbnail generation, gallery page generation,
      and `peaceiris/actions-gh-pages` (or equivalent) deployment step.
- [ ] Add thumbnail caching keyed on the example's source file hash so only
      modified examples re-render.
- [ ] Update `examples/INDEX.md` header with a link to the live gallery URL.
- [ ] Update `docs/README.md` with a Visual Gallery entry.
- [ ] Validate all source-file links in the gallery in CI
      (`scripts/check_gallery_links.sh` or similar).

## Dependencies

### Prerequisite Stories

- GUP-103: Comprehensive Chart Examples Suite ✅ — delivers the full set of
  examples and `examples/INDEX.md` that the gallery is built from.
- GUP-268: PNG Export 📋 — provides the `Chart::export_png` API used to capture
  headless thumbnails.

### Enables Stories

- GUP-280: API Reference Generation 📋 — the gallery is co-deployed to GitHub
  Pages alongside the API reference; a stable Pages deployment here confirms the
  deployment pipeline for GUP-280 to reuse.
- GUP-281: Tutorial and Guide Suite 📋 — tutorials will link to gallery entries
  as "see it in action" references; gallery must exist first.

## Testing Strategy

- **Unit tests**: Parse logic for `examples/INDEX.md` (category extraction,
  example enumeration) should have unit tests if implemented in Rust or Python.
- **Integration tests**: Run the thumbnail generation script against a small
  subset (e.g. `02_scatter_window`, `04_bar_chart`, `business_dashboard`) in CI
  on pull requests; full-suite generation runs only on merge to `main`.
- **Visual validation**: Manually review the generated `docs/gallery/index.html`
  in a browser after the first full run; check that thumbnails are correctly
  labelled and grouped.
- **Link validation**: A CI step fetches the deployed gallery and checks all
  `<a href>` source links return HTTP 200.
- **Performance**: Full thumbnail regeneration of all examples should complete
  within 10 minutes on a standard GitHub-hosted runner (Linux, 2-core); cache
  hits should bring incremental runs under 2 minutes.

## Success Metrics

- [ ] All renderable examples (target: ≥ 40 of the ~60+ examples) produce a
      thumbnail without error.
- [ ] `docs/gallery/index.html` loads in a browser and displays all thumbnails
      grouped by category.
- [ ] CI gallery job passes on the first merge to `main` after implementation.
- [ ] Incremental CI run (no examples changed) completes in under 2 minutes
      thanks to thumbnail caching.
- [ ] Zero broken source-code links in the deployed gallery.

## Risk Assessment

- **Medium**: Headless GPU rendering in CI — GitHub-hosted runners do not have a
  discrete GPU; wgpu must fall back to its software rasteriser (WGPU's
  `dx12`/`vulkan`/`metal` or `wgpu-hal` CPU backend). Some examples may produce
  blank or incorrect output under software rendering. _Mitigation_: Gate on
  `WGPU_ADAPTER_NAME=llvmpipe` (or equivalent) in CI and add an explicit
  software-renderer smoke test locally before enabling the full CI job. Mark
  examples known to require hardware features as "CI-skip" in the config.

- **Medium**: Examples not designed for headless use — many examples open a
  winit `EventLoop` and block indefinitely; they need a minimal code change to
  support the `--headless-screenshot` exit path. _Mitigation_: Introduce a
  lightweight `GupTestHarness::run_headless` helper (or an environment variable
  convention) so examples can opt in to headless mode with a one-line change,
  keeping the main event-loop path unchanged.

- **Low**: Gallery page size — 60+ PNG thumbnails at 640×480 could total 10–30
  MB. Lazy loading mitigates browser performance, but the `gh-pages` branch
  grows over time. _Mitigation_: Compress thumbnails with `oxipng` or similar in
  the generation script; target ≤ 100 KB per thumbnail. Consider generating WebP
  alongside PNG and using `<picture>` elements.

- **Low**: GUP-268 not yet complete — this story depends on the PNG Export API.
  If GUP-268 slips, thumbnail generation cannot proceed. _Mitigation_: The
  gallery page structure, CSS, generator script, and CI workflow can all be
  built and reviewed before GUP-268 lands; only the final thumbnail-capture step
  is blocked. Use placeholder thumbnails (a gray rectangle with the example
  name) during development.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Gallery CI job passes on `main` and gallery is accessible via GitHub Pages
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
