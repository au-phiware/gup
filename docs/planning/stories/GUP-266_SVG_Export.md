# GUP-266: SVG Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-01-31

## Context

SVG (Scalable Vector Graphics) is the most requested export format for data
visualization tools. Unlike raster formats (PNG, JPEG), SVG output is
resolution-independent, editable in vector editors (Inkscape, Illustrator,
Figma), and directly embeddable in HTML documents — making it indispensable for
publishing and sharing visualizations.

Gup currently renders entirely on the GPU. This means there is no CPU-side
vector description of what is drawn; the GPU consumes instance data and executes
shaders directly. SVG export therefore cannot simply "read back" the framebuffer
— it requires marks to describe their geometry as SVG elements. The `Mark` trait
(GUP-009) already provides a strong boundary for this: each mark type knows its
own shape and can express it as SVG without depending on GPU state.

Text presents an additional challenge: GUP-099 renders text via Signed Distance
Field (SDF) glyphs on the GPU. For SVG export, text must instead be emitted as
`<text>` elements with explicit font-family, font-size, and anchor attributes so
the output remains editable and searchable rather than being a path
approximation of rendered glyphs.

This story delivers a `SvgRenderer` that traverses a chart's mark tree, collects
SVG element descriptions from each mark, applies the correct coordinate
transform from GPU clip-space to SVG viewport coordinates, and writes a
well-formed SVG document to a file or in-memory string.

## User Story

> "As a visualization developer, I want to call
> `chart.export_svg("output.svg")?` so that I can produce a
> resolution-independent, editable SVG file from any Gup chart without
> additional tooling."

> "As a data journalist or analyst, I want SVG output that embeds cleanly in
> HTML and opens correctly in vector editors so that I can publish and refine
> visualizations outside of Gup."

## Acceptance Criteria

### AC1: Mark Trait SVG Extension

- [ ] The `Mark` trait gains an optional method
      `fn svg_element(&self) -> Option<SvgElement>` with a default
      implementation returning `None`, preserving backward compatibility with
      existing marks.
- [ ] `SvgElement` is a lightweight enum/struct covering the element types
      needed by built-in marks: `<circle>`, `<rect>`, `<line>`, `<path>`,
      `<text>`, and `<g>` (group).
- [ ] All built-in marks (point, bar, line, area, rule) implement
      `svg_element()` and return a correctly described `SvgElement`.

### AC2: Coordinate Transform

- [ ] GPU clip-space coordinates (`[-1, 1]` on both axes, Y-up) are correctly
      mapped to SVG viewport coordinates (`[0, width]` / `[0, height]`, Y-down)
      for a caller-specified output size.
- [ ] The transform is applied uniformly by `SvgRenderer` so individual marks do
      not need to perform their own coordinate conversion.
- [ ] A unit test verifies that clip-space corners (`(-1,-1)`, `(1,1)`,
      `(-1,1)`, `(1,-1)`) map to the correct SVG pixel coordinates for a 800×600
      viewport.

### AC3: Text Export as `<text>` Elements

- [ ] Text marks (labels, axis tick labels, axis titles) are exported as SVG
      `<text>` elements, not as path approximations of SDF-rendered glyphs.
- [ ] Exported `<text>` elements carry `font-family`, `font-size`,
      `text-anchor`, `dominant-baseline`, `x`, and `y` attributes that reproduce
      the visual position and alignment of the GPU-rendered text as closely as
      possible.
- [ ] The SVG text is human-readable and selectable when opened in a browser or
      vector editor.

### AC4: Axes and Grid Lines

- [ ] Axis lines are exported as `<line>` elements with correct stroke colour
      and width.
- [ ] Axis tick marks are exported as individual `<line>` elements.
- [ ] Grid lines (when enabled) are exported as `<line>` or `<path>` elements
      with the correct stroke style (colour, width, dash pattern).

### AC5: `SvgRenderer` and Public API

- [ ] A `SvgRenderer` struct exists in the `gup` crate that accepts a chart
      reference and an `SvgExportOptions` struct (width, height, background
      colour, and optional CSS string).
- [ ] `SvgRenderer::render(&chart) -> Result<String, GupError>` returns a
      well-formed UTF-8 SVG document string.
- [ ] `Chart::export_svg(path, options) -> Result<(), GupError>` is a
      convenience method that calls `SvgRenderer::render` and writes the result
      to a file.
- [ ] The generated SVG passes basic well-formedness validation (correct XML
      prologue, all elements closed, valid attribute syntax).

### AC6: Example and Documentation

- [ ] An `examples/svg_export.rs` example creates a chart and writes it to
      `output.svg`, demonstrating the full end-to-end flow.
- [ ] Public types and methods have doc comments explaining parameters and
      coordinate conventions.
- [ ] The example compiles and runs without GPU validation errors (GPU rendering
      is not required for the SVG path; the example may construct chart data and
      call the export path only).

## Technical Tasks

- [ ] Define `SvgElement` enum in a new `src/export/svg/element.rs` module
      covering: `Circle`, `Rect`, `Line`, `Path`, `Text`, `Group`.
- [ ] Add `fn svg_element(&self) -> Option<SvgElement>` to the `Mark` trait in
      `src/mark/mod.rs` with a default `None` implementation.
- [ ] Implement `svg_element()` for each built-in mark type (`PointMark`,
      `BarMark`, `LineMark`, `AreaMark`, `RuleMark`).
- [ ] Implement `svg_element()` for text/label mark types, producing `<text>`
      elements with font attributes sourced from the mark's style fields.
- [ ] Implement `svg_element()` for axis and grid line mark types.
- [ ] Create `src/export/svg/renderer.rs` with `SvgRenderer` and the coordinate
      transform logic (clip-space → SVG viewport).
- [ ] Create `SvgExportOptions` struct with fields: `width: u32`, `height: u32`,
      `background: Option<Color>`, `extra_css: Option<String>`.
- [ ] Implement `SvgRenderer::render(&chart) -> Result<String, GupError>` that
      traverses the chart mark tree, calls `svg_element()` on each mark, applies
      the coordinate transform, and serialises to an SVG string.
- [ ] Add `Chart::export_svg(path: impl AsRef<Path>, options: SvgExportOptions)`
      convenience method.
- [ ] Write coordinate transform unit test (AC2 criterion).
- [ ] Write round-trip tests for each built-in mark type: construct mark → call
      `svg_element()` → verify SVG attributes match input data.
- [ ] Write an integration test that exports a simple bar chart and checks the
      SVG string for expected element counts and key attributes.
- [ ] Create `examples/svg_export.rs`.
- [ ] Add doc comments to all public types and methods.

## Dependencies

### Prerequisite Stories

- GUP-009: Core Mark Trait ✅ — provides the `Mark` trait that gains the new
  `svg_element()` method
- GUP-018: Observable Plot Chart Builders ✅ — the chart builder API that
  `Chart::export_svg()` is added to
- GUP-099: GPU Text Rendering Pipeline ✅ — defines how text is positioned and
  styled; SVG export must faithfully reproduce those positions as `<text>`
  elements

### Enables Stories

- GUP-267: PDF Export — SVG output can serve as an intermediate representation
  for PDF generation (SVG-to-PDF conversion is well-supported via libraries such
  as `svg2pdf`)
- GUP-269: HTML Export — HTML export can embed the SVG string produced by
  `SvgRenderer::render()` directly inside an `<html>` document

## Testing Strategy

- **Unit tests**: Coordinate transform correctness (clip-space corners to SVG
  corners for multiple viewport sizes); each built-in mark's `svg_element()`
  output (attribute values match constructor inputs).
- **Integration tests**: Export a minimal chart (e.g., a bar chart with two
  bars, axes, and a title) and assert: the SVG string is well-formed XML; the
  expected number of `<rect>` elements is present; `<text>` elements carry the
  correct font-size attribute; axis `<line>` elements exist.
- **Visual validation**: Run `examples/svg_export.rs`, open `output.svg` in a
  browser and in a vector editor to confirm visual fidelity relative to the GPU
  render.
- **Compatibility check**: Validate the generated SVG against the W3C SVG 1.1
  schema (or use a crate like `xmlcheck`) as part of the integration test suite.

## Success Metrics

- [ ] All built-in mark types implement `svg_element()` and produce non-`None`
      output.
- [ ] The integration test exports a multi-element chart and the resulting SVG
      file opens correctly in Firefox, Chrome, and Inkscape (or equivalent).
- [ ] Coordinate transform unit test passes for at least three distinct viewport
      sizes.
- [ ] `examples/svg_export.rs` compiles and produces a valid `output.svg` file.
- [ ] No regressions in existing GPU rendering tests
      (`cargo test -- --test-threads=1` passes in full).

## Risk Assessment

- **Medium**: Coordinate fidelity — GPU clip-space uses a Y-up convention while
  SVG uses Y-down; off-by-one errors in the transform or incorrect handling of
  the chart margins/padding may cause misaligned exports. _Mitigation_: Write
  explicit unit tests for the transform function with known inputs before wiring
  it into the renderer. Visually cross-check the SVG export against a screenshot
  of the GPU render.

- **Medium**: Text positioning — SDF text rendering places glyphs using GPU
  metrics that may not map exactly to CSS/SVG font metrics for the same nominal
  font-size. The SVG `<text>` positions may be slightly off from the GPU render.
  _Mitigation_: Accept "visually equivalent" rather than pixel-perfect text
  placement as the success criterion. Document any known offset in the API doc
  comments.

- **Low**: Mark trait backward compatibility — adding a new method with a
  default `None` implementation is a non-breaking change, but any third-party
  custom mark implementations will silently produce no SVG output until they
  implement the method. _Mitigation_: Document the new method prominently in the
  `Mark` trait rustdoc and in the Custom Mark Guide.

- **Low**: SVG serialisation — Rust's standard library has no built-in XML
  serialiser. A lightweight dependency (e.g., `quick-xml`) will be needed.
  _Mitigation_: Evaluate `quick-xml` (already used elsewhere in the Rust
  ecosystem for wgpu tooling) first; fallback to manual string construction for
  a small, controlled element set if a dependency is undesirable.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
