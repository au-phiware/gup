# GUP-267: PDF Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
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

- [x] A `PdfRenderer` type is available in the public API (gated behind a `pdf`
      Cargo feature flag, off by default)
- [x] `PdfRenderer` accepts the vector intermediate produced by the SVG export
      pipeline (GUP-266) — no additional GPU commands are issued during export
- [x] The generated PDF contains vector paths that match the source chart
      geometry (verified by opening in a PDF viewer or automated comparison)
- [x] Text elements are rendered as proper PDF text objects, not outlines,
      unless the requested font cannot be embedded

### AC2: Configurable Page Size

- [x] `PdfOptions` exposes named constructors `PdfOptions::a4()`,
      `PdfOptions::letter()`, and `PdfOptions::custom(width_mm, height_mm)`
- [x] The chart is scaled to fill the page while preserving aspect ratio, with
      configurable margin (default 10 mm on each side)
- [x] Portrait and landscape orientation can be selected via
      `PdfOptions::orientation(Orientation::Landscape)`

### AC3: Embedded Fonts

- [x] Fonts used by text marks are embedded in the PDF as subsets so that the
      document renders correctly on systems that do not have those fonts
      installed
- [x] When a font file cannot be located or embedded, export falls back to a
      standard PDF base-14 font and emits a non-fatal warning (not a hard error)

### AC4: Multi-Page PDF

- [x] A `PdfDocument` builder allows multiple charts to be appended, each on its
      own page, before writing the final file
- [x] The API compiles without ergonomic friction:
      `rust     let mut doc = PdfDocument::new(PdfOptions::a4());     doc.add_chart(&chart_a)?;     doc.add_chart(&chart_b)?;     doc.write("report.pdf")?;     `
- [x] Page count in the written file matches the number of `add_chart` calls

### AC5: Single-Chart Convenience Method

- [x] The chart builder exposes a `export_pdf` convenience method so that the
      common single-chart case requires minimal boilerplate:
      `rust     chart.export_pdf("report.pdf", PdfOptions::a4())?;     `
- [x] The method returns a `Result<(), GupError>` and propagates I/O and
      serialisation errors cleanly

### AC6: Runnable Example

- [x] A compilable example at `examples/pdf_export.rs` demonstrates single-chart
      and multi-page export using a real dataset
- [x] The example is listed in `Cargo.toml` and passes `cargo check --examples`

## Technical Tasks

- [x] Add optional `pdf` feature to `Cargo.toml`; select a pure-Rust PDF library
      (e.g. `printpdf`, `lopdf`, or `pdf-writer`) as a feature-gated dependency
- [x] Define `PdfOptions` struct with page-size presets, orientation, and margin
      fields
- [x] Implement `PdfRenderer` that consumes the SVG vector intermediate (from
      GUP-266) and writes PDF content streams, path operators, and text objects
- [x] Implement font subsetting / embedding using the chosen library's font API;
      add fallback logic with a warning for missing fonts
- [x] Implement `PdfDocument` builder supporting `add_chart`, `add_page` (raw),
      and `write` / `write_to_writer` methods
- [x] Add `export_pdf` convenience method to the chart builder type introduced
      in GUP-018
- [x] Write unit tests for `PdfOptions` preset dimensions and scaling logic
- [x] Write integration test: render a minimal chart, export to PDF, assert file
      is valid PDF (magic bytes, page count via a lightweight parser)
- [x] Write `examples/pdf_export.rs` covering single-chart and multi-page paths
- [x] Update `docs/` with a short PDF export guide (or a section in the SVG
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

- [x] `cargo test --features pdf -- --test-threads=1` passes with zero failures
- [x] `cargo check --examples --features pdf` passes
- [x] `cargo check` (without `pdf` feature) passes — no feature-flag leakage
- [x] The PDF written by `examples/pdf_export.rs` opens correctly in at least
      one standard PDF viewer
- [x] Font is embedded in the output file (verifiable with `pdfinfo` or
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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test --features pdf -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples --features pdf`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

### What was implemented

- **`pdf` Cargo feature** gating all PDF-related types and dependencies
- **`printpdf` v0.9** as the PDF generation library
- **`src/export/pdf/options.rs`**: `PdfOptions` with A4, US Letter, and custom
  page sizes; `Orientation` enum for portrait/landscape; margin and fit-scale
  calculations
- **`src/export/pdf/renderer.rs`**:
  - `PdfRenderer` converting `SvgElement` trees to PDF operations (Rect,
    Circle via Bézier arcs, Line, Text, Path, Group with graphics state
    save/restore)
  - `PdfDocument` multi-page builder with `add_page_from_elements()`,
    `add_chart()`, `write()`, `write_to_writer()`, and `to_bytes()`
  - CSS colour parsing (`rgb()`, `rgba()`, `#hex`, named colours)
  - SVG path data parsing (M, L, Z commands)
  - System font embedding via `fontdb` with fallback to built-in Helvetica
- **`src/export/pdf/mod.rs`**: Module root re-exporting public types
- **`src/chart_builder.rs`**: `export_pdf()` and `export_pdf_with_marks()`
  convenience methods on `ComposedChart`, feature-gated behind `pdf`
- **`src/export/mod.rs`** and **`src/prelude.rs`**: Updated to re-export PDF
  types when feature is enabled (with `PdfOrientation` alias to avoid name
  collision with `bar::Orientation`)
- **`examples/pdf_export.rs`**: Demonstrates single-chart export, 3-page
  multi-page export, and landscape letter-size export
- **`tests/pdf_export_integration.rs`**: 14 integration tests covering PDF
  magic bytes, non-trivial size, multi-page, write/read-back, error paths,
  landscape, custom sizes, groups, paths, dashed lines, writer output, empty
  docs, preset options, and feature-flag existence

### Key files changed

| File | Change |
|------|--------|
| `Cargo.toml` | Added `printpdf` optional dep, `pdf` feature, `pdf_export` example entry |
| `src/export/pdf/mod.rs` | New module root |
| `src/export/pdf/options.rs` | `PdfOptions`, `Orientation`, 11 unit tests |
| `src/export/pdf/renderer.rs` | `PdfRenderer`, `PdfDocument`, colour/path parsers, font embedding, 17 unit tests |
| `src/export/mod.rs` | Added `pdf` submodule + re-exports |
| `src/prelude.rs` | Added feature-gated PDF re-exports |
| `src/chart_builder.rs` | Added `export_pdf` and `export_pdf_with_marks` |
| `examples/pdf_export.rs` | New example |
| `tests/pdf_export_integration.rs` | 14 integration tests |

### Test counts

- 28 unit tests in `src/export/pdf/` (11 options + 17 renderer)
- 14 integration tests in `tests/pdf_export_integration.rs`
- **42 total PDF-specific tests**

## Retrospective

**Completed**: 2026-03-04

### Key Technical Learnings

#### printpdf API Design

- **Challenge**: `printpdf` v0.9 has a completely different API from older
  versions documented in most online tutorials.  The new API uses `Op` enums
  pushed to a `Vec<Op>` rather than layer-based method calls.
- **Solution**: Read the library source code directly
  (`/home/corin/.cargo/registry/src/`) rather than relying on outdated docs.
  The `PdfPage::new(width, height, ops)` constructor and `Op::DrawRectangle`,
  `Op::DrawPolygon`, `Op::DrawLine`, `Op::ShowText` etc. are the key building
  blocks.
- **Pattern**: When a crate's published docs are sparse or outdated, reading the
  source of its types and examples in the registry is faster than searching
  online.

#### Coordinate System Mapping

- **Challenge**: Three coordinate systems are in play — GPU clip-space (Y-up,
  [-1,1]), SVG viewport (Y-down, origin top-left), and PDF page (Y-up, origin
  bottom-left, in points).  Getting the Y-flip right required care.
- **Solution**: The SVG intermediate already handles clip→SVG transforms.  For
  SVG→PDF, the transform is: `pdf_y = page_height - svg_y` (after scaling and
  offset).  Rectangles need special attention because SVG specifies top-left
  corner but PDF Rect specifies bottom-left.
- **Pattern**: Always write a coordinate-transform helper function and unit-test
  corner cases rather than inlining the math in multiple places.

#### Font Embedding via fontdb

- **Challenge**: `printpdf`'s `ParsedFont::from_bytes` requires raw TTF/OTF
  bytes and a face index.  The font must first be located on the system.
- **Solution**: Used `fontdb::Database::load_system_fonts()` followed by
  `db.query()` to find a matching sans-serif font, then
  `db.with_face_data(id, |data, index| ...)` to get the bytes for
  `ParsedFont::from_bytes()`.  The `with_face_data` callback approach avoids
  needing to know the `Source` variant.
- **Pattern**: `fontdb`'s `with_face_data` is the most portable way to access
  font bytes — it works for file-backed, memory-mapped, and binary sources
  without pattern-matching on the `Source` enum.

#### Circle Approximation with Bézier Curves

- **Challenge**: PDF has no native circle primitive.  Circles must be
  approximated as cubic Bézier polygons.
- **Solution**: Used the standard four-arc approximation with magic constant
  `k ≈ 0.552285` (= 4/3 × (√2 − 1)).  Each arc uses one on-curve point and
  two control points, totaling 13 points for a closed circle.
- **Pattern**: This is a well-known PDF/PostScript pattern.  The constant is
  precise enough that the visual difference from a true circle is imperceptible
  at any reasonable zoom level.

### Architectural Decisions

#### printpdf over pdf-writer

- **Decision**: Chose `printpdf` v0.9 over `pdf-writer` v0.14
- **Reasoning**: `printpdf` provides higher-level types (`PdfPage`, `Op`,
  `PdfDocument`, `ParsedFont`) that map well to our SVG element abstraction.
  `pdf-writer` would require manually constructing PDF objects and content
  streams, which is more work for the same result.
- **Trade-off**: `printpdf` brings `lopdf` and potentially `azul-layout` as
  transitive dependencies, adding to compile time and binary size.  `pdf-writer`
  is a much lighter dependency.
- **Future**: If binary size becomes an issue, the renderer's `Op`-based output
  could be adapted to `pdf-writer` without changing the public API.

#### Orientation Alias in Prelude

- **Decision**: Re-export `pdf::Orientation` as `PdfOrientation` in the prelude
  to avoid a name collision with `bar::Orientation`.
- **Reasoning**: Both types have the same name but different semantics (page
  orientation vs bar chart orientation).  Aliasing is cleaner than removing one
  from the prelude.
- **Trade-off**: Users who import both must use the alias or qualified paths.
- **Future**: If more orientation-like types appear, a unified
  `gup::Orientation` enum with more variants might be warranted.

#### Font Embedding at Document Level

- **Decision**: Font embedding happens once in `PdfDocument::new()`, not per
  page.
- **Reasoning**: Font resources are shared across all pages in a PDF.  Loading
  fonts once avoids duplicate work and ensures consistent font IDs.
- **Trade-off**: The `PdfRenderer` used standalone (without `PdfDocument`) uses
  built-in fonts only.  Users needing embedded fonts should use `PdfDocument`.
- **Future**: Could add a `PdfRenderer::embed_font()` method for standalone use.

### Development Workflow Insights

- **Disk space**: The `/tmp` partition filled up during the initial build because
  the cargo target directory was on tmpfs.  Cleaning `/tmp/gup-target` and
  rebuilding freed ~60 GB.  Setting `CARGO_TARGET_DIR=/tmp/gup-target` and using
  a symlink kept builds working.

- **Feature-flag testing**: Testing both `cargo check` and
  `cargo check --features pdf` is essential for every commit to prevent
  feature-flag leakage.  The `required-features` field in `Cargo.toml` for
  examples prevents them from failing in the default build.

- **printpdf type mismatch**: The `ParsedFont::from_bytes` signature differs
  between printpdf's default and `text_layout` feature configurations — it takes
  `u32` without `text_layout` but `usize` with it.  `fontdb::with_face_data`
  gives `u32`, so a cast is needed when text_layout is enabled.

### Follow-up Stories

No new follow-up stories identified during implementation.  The PDF export is
self-contained and the existing SVG intermediate from GUP-266 proved to be a
clean integration point.
