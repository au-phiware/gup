# GUP-267: PDF Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-01-31

## Context

PDF remains the dominant format for distributing charts in academic papers,
business reports, and archival documents. While Gup's GPU pipeline produces
pixel-perfect on-screen renders, there is currently no supported path for
exporting charts to a portable, print-ready format.

GUP-266 introduces an SVG export pipeline that extracts vector paths, geometry,
and text runs from the scene graph into an intermediate representation without
requiring a display. This story builds directly on that intermediate: rather
than serialising the vector data as SVG markup, a `PdfRenderer` converts it into
a PDF document using a pure-Rust PDF library. Sharing the extraction step means
that mark-to-vector conversion code is written once and reused across both
output formats.

GUP-018 established the high-level chart builder API (`gup::plot()`) that
end-users interact with. PDF export is exposed through the same fluent
interface, giving chart authors a consistent path from authoring to publication.

Multi-page support is a first-class requirement: workflows that need to compile
several charts into a single report document must be able to add charts one per
page without orchestrating multiple single-page PDFs externally.

## User Story

> "As a data analyst, I want to export one or more Gup charts to a PDF file so
> that I can include them in printed reports and academic papers with full
> vector quality."

> "As a Rust application developer, I want a programmatic multi-page PDF export
> API so that I can generate report documents containing several charts without
> shell-level PDF tooling."

## Acceptance Criteria

### AC1: PdfRenderer Converts SVG Intermediate to PDF

- [ ] A `PdfRenderer` type is available in the public API (gated behind a `pdf`
      Cargo feature flag, off by default)
- [ ] `PdfRenderer` accepts the vector intermediate produced by the SVG export
      pipeline (GUP-266) — no additional GPU commands are issued during export
- [ ] The generated PDF contains vector paths that match the source chart
      geometry (verified by opening in a PDF viewer or automated comparison)
- [ ] Text elements are rendered as proper PDF text objects, not outlines,
      unless the requested font cannot be embedded

### AC2: Configurable Page Size

- [ ] `PdfOptions` exposes named constructors `PdfOptions::a4()`,
      `PdfOptions::letter()`, and `PdfOptions::custom(width_mm, height_mm)`
- [ ] The chart is scaled to fill the page while preserving aspect ratio, with
      configurable margin (default 10 mm on each side)
- [ ] Portrait and landscape orientation can be selected via
      `PdfOptions::orientation(Orientation::Landscape)`

### AC3: Embedded Fonts

- [ ] Fonts used by text marks are embedded in the PDF as subsets so that the
      document renders correctly on systems that do not have those fonts
      installed
- [ ] When a font file cannot be located or embedded, export falls back to a
      standard PDF base-14 font and emits a non-fatal warning (not a hard error)

### AC4: Multi-Page PDF

- [ ] A `PdfDocument` builder allows multiple charts to be appended, each on its
      own page, before writing the final file
- [ ] The API compiles without ergonomic friction:
      `rust     let mut doc = PdfDocument::new(PdfOptions::a4());     doc.add_chart(&chart_a)?;     doc.add_chart(&chart_b)?;     doc.write("report.pdf")?;     `
- [ ] Page count in the written file matches the number of `add_chart` calls

### AC5: Single-Chart Convenience Method

- [ ] The chart builder exposes a `export_pdf` convenience method so that the
      common single-chart case requires minimal boilerplate:
      `rust     chart.export_pdf("report.pdf", PdfOptions::a4())?;     `
- [ ] The method returns a `Result<(), GupError>` and propagates I/O and
      serialisation errors cleanly

### AC6: Runnable Example

- [ ] A compilable example at `examples/pdf_export.rs` demonstrates single-chart
      and multi-page export using a real dataset
- [ ] The example is listed in `Cargo.toml` and passes `cargo check --examples`

## Technical Tasks

- [ ] Add optional `pdf` feature to `Cargo.toml`; select a pure-Rust PDF library
      (e.g. `printpdf`, `lopdf`, or `pdf-writer`) as a feature-gated dependency
- [ ] Define `PdfOptions` struct with page-size presets, orientation, and margin
      fields
- [ ] Implement `PdfRenderer` that consumes the SVG vector intermediate (from
      GUP-266) and writes PDF content streams, path operators, and text objects
- [ ] Implement font subsetting / embedding using the chosen library's font API;
      add fallback logic with a warning for missing fonts
- [ ] Implement `PdfDocument` builder supporting `add_chart`, `add_page` (raw),
      and `write` / `write_to_writer` methods
- [ ] Add `export_pdf` convenience method to the chart builder type introduced
      in GUP-018
- [ ] Write unit tests for `PdfOptions` preset dimensions and scaling logic
- [ ] Write integration test: render a minimal chart, export to PDF, assert file
      is valid PDF (magic bytes, page count via a lightweight parser)
- [ ] Write `examples/pdf_export.rs` covering single-chart and multi-page paths
- [ ] Update `docs/` with a short PDF export guide (or a section in the SVG
      export guide)

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot Chart Builders ✅ — provides the high-level chart
  builder API that `export_pdf` is attached to
- GUP-266: SVG Export 📋 — provides the vector intermediate (path extraction,
  text runs, scene-graph traversal) that `PdfRenderer` consumes; no duplication
  of mark-to-vector code

### Enables Stories

- (None identified at time of writing — PDF export is an end-user deliverable
  with no known downstream stories)

## Testing Strategy

- **Unit tests**: `PdfOptions` dimension calculations; scaling and margin maths;
  orientation transforms; font-fallback warning path
- **Integration tests**: render a simple scatter chart headlessly, export to
  bytes, verify PDF header (`%PDF-`) and that the reported page count is correct
  using a minimal parser
- **Visual validation**: open the output of `examples/pdf_export.rs` in a PDF
  viewer (Evince, Preview, Adobe Reader) and confirm axes, marks, and labels are
  present and legible
- **Feature-flag hygiene**: `cargo check` without the `pdf` feature must compile
  cleanly; no PDF types leak into the default public API

## Success Metrics

- [ ] `cargo test --features pdf -- --test-threads=1` passes with zero failures
- [ ] `cargo check --examples --features pdf` passes
- [ ] `cargo check` (without `pdf` feature) passes — no feature-flag leakage
- [ ] The PDF written by `examples/pdf_export.rs` opens correctly in at least
      one standard PDF viewer
- [ ] Font is embedded in the output file (verifiable with `pdfinfo` or
      equivalent)

## Risk Assessment

- **Medium**: The chosen PDF library may have limited or immature
  font-subsetting support. `printpdf` supports TTF embedding but its subsetting
  API is low-level; `pdf-writer` is lower-level still. _Mitigation_: Prototype
  font embedding early in the task list. If subsetting proves infeasible with
  the first choice, fall back to full font embedding (larger file size) and
  document the limitation.

- **Medium**: The SVG intermediate format (GUP-266) is not yet defined at the
  time this story is written. If the intermediate changes during GUP-266
  implementation, the `PdfRenderer` adapter will need to be updated.
  _Mitigation_: Implement GUP-266 first and treat its intermediate type as the
  stable interface contract before starting PDF rendering work.

- **Low**: Multi-page documents with many charts may produce large in-memory PDF
  objects before the file is written. _Mitigation_: Prefer streaming /
  incremental write APIs offered by the PDF library where available; document
  memory characteristics in the API docs.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test --features pdf -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples --features pdf`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
