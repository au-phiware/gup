# GUP-269: HTML Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-08-12

## Context

Sharing a Gup chart today requires either running a native binary or setting up
a web server with the correct WASM build, WebGPU bindings, and asset pipeline.
Neither option is accessible to a stakeholder who simply wants to open a chart
in a browser. A self-contained HTML export closes this gap: a single `.html`
file that anyone can double-click and view — no server, no build toolchain, no
framework.

GUP-266 (SVG Export) provides a static SVG rendering of the chart that can be
embedded as a graceful fallback for browsers where WebGPU is unavailable or
blocked. GUP-268 (PNG Export) provides a raster thumbnail suitable for Open
Graph `<meta>` tags so that the page previews correctly when shared via
messaging apps and social media. GUP-172 (WebAssembly Performance Benchmarks)
established the WASM build pipeline and confirmed that a Gup chart can be
compiled and run inside a browser, giving this story a proven technical
foundation to build on.

The primary deliverable is an `HtmlExporter` type (mirroring the conventions
established by the SVG and PNG exporters) that packages the WASM bundle, chart
definition data, SVG fallback, and PNG thumbnail into a single HTML file. Two
embedding strategies are supported: inlining the WASM as a Base64 data URI
(maximally portable, no network required) and referencing a CDN or local path
(smaller HTML file, suitable for deployment contexts).

## User Story

> "As a visualization developer, I want to export a Gup chart as a standalone
> HTML file so that I can share an interactive chart with colleagues or
> stakeholders who have only a web browser."

> "As an end user receiving a shared chart, I want to open an HTML file in my
> browser and see the interactive chart — or a clear static fallback if WebGPU
> is unavailable — so that I can explore the data without installing anything."

## Acceptance Criteria

### AC1: `HtmlExporter` API

- [ ] A public `HtmlExporter` struct exists in the `gup` crate (or a
      `gup-export` sub-crate consistent with project conventions).
- [ ] `chart.export_html("chart.html")?` compiles and writes a valid HTML file.
- [ ] The exporter accepts a builder-style configuration for: inline vs. CDN
      WASM strategy, page title, optional author metadata.
- [ ] The public API is documented with at least one `# Example` doctest that
      passes `cargo test`.

### AC2: Interactive WASM rendering

- [ ] The exported HTML correctly loads and initialises the Gup WASM bundle.
- [ ] The chart renders interactively in a WebGPU-capable browser (Chromium ≥
      113 / Chrome stable) opened from the local filesystem (i.e. `file://` URL,
      no server required) when using the inline strategy.
- [ ] The WASM bundle is either Base64-inlined into the HTML (inline strategy)
      or referenced via a configurable URL (CDN strategy), controlled by
      `WasmStrategy::Inline` and `WasmStrategy::Url(String)` enum variants.

### AC3: Chart data embedding

- [ ] The chart definition and data are serialised as JSON and embedded in a
      `<script type="application/json">` block within the HTML.
- [ ] Round-tripping the embedded JSON back through the Gup deserialiser
      produces an equivalent chart (tested via a unit test).

### AC4: Static SVG fallback

- [ ] When WebGPU is unavailable or the WASM module fails to initialise, the
      page displays the embedded SVG export (from GUP-266) instead of a blank
      canvas or error.
- [ ] The SVG fallback is shown via a `<noscript>` block and a JavaScript
      runtime check for `navigator.gpu`, so it is visible both when JS is
      disabled and when WebGPU is absent.

### AC5: Open Graph thumbnail

- [ ] The exported HTML contains `<meta property="og:image">` and
      `<meta name="twitter:image">` tags whose content is the PNG thumbnail
      produced by GUP-268, Base64-inlined as a data URI.
- [ ] `<meta property="og:title">` and `<meta property="og:description">` are
      populated from the chart title and optional description fields.

### AC6: Example

- [ ] An example `examples/html_export.rs` exists that creates a simple chart,
      calls `export_html`, and writes the result to a temp path.
- [ ] `cargo check --examples` passes.
- [ ] The example is listed in `Cargo.toml` under `[[example]]`.

## Technical Tasks

- [ ] Audit the existing SVG exporter (GUP-266) and PNG exporter (GUP-268) APIs
      to confirm the trait/struct conventions to follow.
- [ ] Define `WasmStrategy` enum (`Inline`, `Url(String)`) in the exporter
      module.
- [ ] Implement `HtmlExporter` builder with fields: `wasm_strategy`,
      `page_title: Option<String>`, `description: Option<String>`,
      `author: Option<String>`.
- [ ] Add an `export_html` convenience method on the chart type that delegates
      to `HtmlExporter`.
- [ ] Write the HTML template (either a `const` string with format placeholders
      or a minimal template engine) covering: `<!DOCTYPE html>`, viewport meta,
      OG meta tags, inline CSS for canvas sizing, WASM bootstrap `<script>`, SVG
      fallback `<noscript>` block, embedded JSON data block.
- [ ] Implement the inline WASM strategy: read the compiled `.wasm` artifact,
      Base64-encode it, emit as a `WebAssembly.instantiate(base64decode(...))`
      JS snippet.
- [ ] Implement the CDN/URL strategy: emit a standard `fetch(url)` WASM load
      snippet.
- [ ] Call `SvgExporter` internally to produce the fallback SVG bytes.
- [ ] Call `PngExporter` internally to produce the thumbnail PNG bytes, then
      Base64-encode for the OG `<meta>` tags.
- [ ] Serialise chart definition to JSON; add a `Serialize` impl or derive if
      not already present.
- [ ] Write unit test: parse the embedded JSON from the rendered HTML, confirm
      round-trip equivalence.
- [ ] Write integration test: render to a `NamedTempFile`, assert the file is
      valid UTF-8 HTML containing expected substrings (`og:image`, `<noscript>`,
      `application/json`).
- [ ] Create `examples/html_export.rs`.
- [ ] Add `[[example]]` entry to `Cargo.toml` if required by project
      conventions.
- [ ] Run `mask all-fix` and resolve any lint/format warnings.

## Dependencies

### Prerequisite Stories

- GUP-266: SVG Export 📋 — provides the static SVG fallback embedded in the HTML
  and the exporter conventions this story follows.
- GUP-268: PNG Export 📋 — provides the raster thumbnail embedded as the OG
  image.
- GUP-172: WebAssembly Performance Benchmarks ✅ — established the WASM build
  pipeline and confirmed browser-side execution of Gup charts.

### Enables Stories

_(None identified at this time. Future stories for chart-sharing services or
notebook integrations would naturally build on this.)_

## Testing Strategy

- **Unit tests**: Verify JSON serialisation round-trip of the chart definition;
  verify Base64 encoding of a known byte sequence produces the expected data URI
  prefix.
- **Integration tests**: Write an HTML file to a temp path; assert it is
  well-formed (contains `<!DOCTYPE html>`), includes OG meta tags, includes a
  `<noscript>` block with SVG content, and includes an `application/json` script
  block.
- **Visual validation**: Open the exported `chart.html` from
  `examples/html_export.rs` in a local Chromium build; confirm the chart renders
  interactively. (Manual step; not automated in CI for this story.)
- **Compilation**: `cargo check --examples` must pass; `cargo test` must pass
  for all new unit and integration tests.

## Success Metrics

- [ ] `chart.export_html("chart.html")?` produces a file that opens and renders
      in a local Chromium browser without a web server.
- [ ] The HTML file is self-contained: no external network requests are required
      when using `WasmStrategy::Inline`.
- [ ] All new tests pass under `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: `mask all-fix` exits 0.
- [ ] The SVG fallback is visible when `navigator.gpu` is absent (verified by
      toggling WebGPU off in DevTools or using a non-WebGPU browser).

## Risk Assessment

- **Medium**: The WASM binary can be large (several MB). Base64 encoding adds
  ~33 % overhead, which may produce an HTML file that is impractical for email
  or low-bandwidth sharing. _Mitigation_: Document the size trade-off in the API
  docs; make `WasmStrategy` a first-class choice so users can opt for the
  CDN/URL strategy and distribute the `.wasm` file separately.

- **Medium**: Chart definition serialisation may not yet exist (Gup's internal
  types may not implement `serde::Serialize`). Adding derives could conflict
  with existing `#[cfg(feature = "serde")]` gating or non-serialisable GPU
  handles. _Mitigation_: Use a separate `ChartSnapshot` DTO that captures only
  the data/configuration needed for re-rendering, keeping GPU resource handles
  out of the serialisation path.

- **Low**: The `file://` protocol imposes same-origin restrictions in some
  browsers that can block WASM loading from a data URI. This is known to work in
  Chrome but may require the `--allow-file-access-from-files` flag in some
  environments. _Mitigation_: Document the browser requirement; test against
  Chrome stable as the primary target; note the CDN strategy as the workaround
  for restricted environments.

- **Low**: GUP-266 and GUP-268 are both `📋 Planned`. If they are not complete
  when work on this story begins, the SVG fallback and PNG thumbnail steps must
  be deferred or stubbed. _Mitigation_: Design the `HtmlExporter` builder so
  that SVG fallback and PNG thumbnail are optional fields; the story can proceed
  to a mergeable state without them, with those ACs gated on dependency
  completion.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
