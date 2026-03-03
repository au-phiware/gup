# GUP-281: Tutorial and Guide Suite

## Story Overview

**Initiative**: Documentation **Status**: 📋 Planned **Created**: 2025-07-14

## Context

A library's adoption depends heavily on its learning materials. Gup currently
has a rich body of technical documentation — the Mark System guide
(`docs/mark-system/`), the Custom Mark Guide (`docs/CUSTOM_MARK_GUIDE.md`), the
Grid System reference (`docs/GRID_SYSTEM.md`), and the Observable Plot Migration
Guide produced by GUP-086 — but none of these are designed to take a developer
from zero Gup experience to productive use. They assume the reader already
understands the domain and are reference-first rather than task-first.

GUP-103 delivered a comprehensive examples suite that demonstrates API patterns
in code, and GUP-018 delivered the high-level chart builder API that most
developers will reach for first. Together, those two stories establish the raw
material this story organises into a structured learning progression. The
proposed GUP-280 (API Reference Generation) will generate cross-linked API docs;
tutorials should be written to reference those docs at the points where a
developer naturally wants to dive deeper, reinforcing the two artefacts'
complementary roles.

There is currently no `docs/tutorials/` directory. This story creates that
directory and populates it with six tutorials ordered by increasing complexity:
getting started with a chart builder, binding custom data types through the
`Selection<T, M>` API, writing GPU shader transforms with `#[shader_fn]`, adding
interactions, connecting live data through `StreamingDataSource`, and
implementing a new mark type from scratch. Each tutorial is a standalone
Markdown document that narrates a concrete coding task, includes working code
snippets, and (where visual output is meaningful) includes rendered output
screenshots captured from the examples suite.

## User Story

> "As a developer new to Gup, I want step-by-step tutorials that guide me
> through the most important use-cases so that I can become productive with both
> the high-level chart builder API and the low-level `Selection` API without
> having to reverse-engineer the examples or read source code."

> "As an experienced Gup user, I want self-contained guides for advanced topics
> — custom shader functions, custom mark types, and streaming data — so that I
> can tackle those tasks confidently without missing important constraints."

## Acceptance Criteria

### AC1: Tutorial Directory Structure

- [ ] `docs/tutorials/` directory is created and committed.
- [ ] `docs/tutorials/README.md` exists and lists all six tutorials with a
      one-sentence description of each and links to the full tutorial document.
- [ ] Each tutorial is a standalone Markdown file named with a numeric prefix
      and a short slug: `01_getting_started.md`, `02_data_binding.md`,
      `03_custom_shader_functions.md`, `04_interactions.md`,
      `05_streaming_data.md`, `06_custom_marks.md`.
- [ ] `docs/README.md` is updated to include a "Tutorials" section that links to
      `docs/tutorials/README.md`.

### AC2: Tutorial 1 — Getting Started

- [ ] Covers installing/adding Gup as a dependency and setting up a minimal
      `Cargo.toml`.
- [ ] Walks through creating a scatter chart with `gup::plot()` using the chart
      builder API (GUP-018), rendering it, and running the program.
- [ ] Reader can produce a working chart by following the tutorial verbatim —
      all code snippets are copy-paste correct against the current API.
- [ ] References the API docs entry-point (or GUP-280 placeholder) for further
      reading on `PlotBuilder`.

### AC3: Tutorial 2 — Data Binding

- [ ] Demonstrates defining a custom Rust struct and binding it to a
      `Selection<T, M>` (GUP-002) using `set_data()` and accessor functions.
- [ ] Covers at least one example of updating the selection's data and
      re-rendering (join / update pattern).
- [ ] All code snippets compile against the current `Selection` API.
- [ ] Explains `PhantomData<M>` and why the mark type parameter exists —
      connecting the conceptual model to the concrete type signature.

### AC4: Tutorial 3 — Custom Shader Functions

- [ ] Explains when to use `#[shader_fn]` versus the chart builder API.
- [ ] Walks through annotating a Rust function with `#[shader_fn]`, registering
      it as a vertex attribute via `Selection::attr_shader`, and validating the
      generated WGSL with a `cargo test` invocation.
- [ ] Covers at least one uniform parameter example (parameterised shader
      function).
- [ ] Notes WGSL type constraints and the supported Rust type subset, pointing
      to `docs/TECHNICAL_APPROACH.md` for the transpiler reference.

### AC5: Tutorial 4 — Interactions

- [ ] Covers the hover, click, and brush interaction patterns using the
      `InteractionEvent` system (GUP-031).
- [ ] Includes a worked example that adds a tooltip on hover to an existing
      scatter chart.
- [ ] Documents the zoom/pan interaction (GUP-277) as a sub-section with a
      minimal code snippet wiring `ZoomBehavior` to a chart.
- [ ] All interaction API snippets are correct against the current codebase.

### AC6: Tutorial 5 — Streaming Data

- [ ] Explains the `StreamingDataSource<T>` trait and when to use
      `StreamingScatterPlot` versus the `DataStream` builder API (GUP-244).
- [ ] Walks through implementing a minimal `StreamingDataSource` that generates
      synthetic data and wires it to a `StreamingScatterPlot`.
- [ ] Notes backpressure and eviction semantics, pointing to GUP-244 story or
      API reference for deeper detail.
- [ ] Code example compiles (even if GUP-244 builder is not yet complete, the
      `StreamingDataSource` trait path must work against the current codebase).

### AC7: Tutorial 6 — Custom Marks

- [ ] Explains the role of the `Mark` trait and the `#[derive(Mark)]` macro.
- [ ] Walks through implementing a new mark type — struct definition, `Mark`
      derive, WGSL vertex/fragment shader, and registration with `MarkRegistry`.
- [ ] References `docs/CUSTOM_MARK_GUIDE.md` for the architectural overview and
      the mark-system docs for the `MarkRenderer` API.
- [ ] The worked example code is self-contained, compiles, and produces
      verifiable output (e.g., passes a `cargo test` invocation or
      `cargo check`).

### AC8: Screenshot Assets

- [ ] `docs/tutorials/assets/` directory contains at least one rendered output
      screenshot per tutorial (where visual output is meaningful — Tutorials 1,
      4, 5, 6).
- [ ] Screenshots are `.png` files named `tutorialNN_<slug>.png`.
- [ ] Each tutorial document embeds the corresponding screenshot using a
      relative Markdown image link.

## Technical Tasks

- [ ] Create `docs/tutorials/` directory with `.gitkeep` or initial `README.md`.
- [ ] Write `docs/tutorials/README.md` — overview, prerequisites (Rust stable,
      wgpu-compatible GPU), and links to all six tutorials.
- [ ] Write `docs/tutorials/01_getting_started.md` — chart builder walkthrough.
- [ ] Write `docs/tutorials/02_data_binding.md` — `Selection<T, M>` and accessor
      patterns.
- [ ] Write `docs/tutorials/03_custom_shader_functions.md` — `#[shader_fn]`
      transpiler usage.
- [ ] Write `docs/tutorials/04_interactions.md` — hover, click, brush, zoom/pan.
- [ ] Write `docs/tutorials/05_streaming_data.md` — `StreamingDataSource` and
      `StreamingScatterPlot`.
- [ ] Write `docs/tutorials/06_custom_marks.md` — `Mark` trait, derive macro,
      shader, registry.
- [ ] Capture screenshot assets for Tutorials 1, 4, 5, and 6 by running the
      relevant examples from the GUP-103 suite; save to
      `docs/tutorials/assets/`.
- [ ] Verify all code snippets compile: run `cargo check --examples` and attempt
      to build any snippet that is not lifted verbatim from an existing example.
- [ ] Update `docs/README.md` to add a "Tutorials" section linking to
      `docs/tutorials/README.md`.
- [ ] Cross-link: add a "See also" note in `docs/CUSTOM_MARK_GUIDE.md` pointing
      to Tutorial 6, and in `docs/GRID_SYSTEM.md` pointing to Tutorial 1.

## Dependencies

### Prerequisite Stories

- GUP-002: Core Selection Type ✅ — `Selection<T, M>` API documented in Tutorial
  2; accessor and `attr_shader` patterns used throughout.
- GUP-018: Observable Plot Chart Builders ✅ — Tutorial 1 is built entirely
  around the chart builder API.
- GUP-103: Comprehensive Chart Examples Suite ✅ — Tutorial code snippets are
  derived from or consistent with this example suite; screenshots sourced from
  running these examples.
- GUP-280: API Reference Generation 📋 — Tutorials reference the generated API
  docs for deeper reading; "See API docs for `X`" links should point to the
  output of GUP-280. Tutorials can be written with placeholder links before
  GUP-280 completes, but final review should confirm all links resolve.

### Enables Stories

- GUP-282: Example Gallery — The gallery is a visual index of runnable examples;
  the tutorials establish the learning narrative that the gallery links back to,
  so the two artefacts are designed together even if implemented separately.

## Testing Strategy

- **Prose review**: Each tutorial is reviewed end-to-end by walking through the
  described steps in a clean checkout to verify instructions are complete and
  correct.
- **Snippet compilation**: Every non-trivial code block is either (a) lifted
  verbatim from a tested example file, or (b) verified with `cargo check` in an
  isolated snippet file under `tests/doc_snippets/` or equivalent.
- **Link checking**: All cross-references to other docs files and API anchors
  are checked with a `grep`-based or `mdbook-linkcheck`-style pass.
- **Screenshot freshness**: Screenshots are generated from the GUP-103 example
  binaries at the time of writing; a note in `docs/tutorials/README.md`
  documents which example binary produces each screenshot so they can be
  regenerated after visual changes.

## Success Metrics

- [ ] A developer with no prior Gup experience can follow Tutorial 1 and run a
      working chart without consulting any other document.
- [ ] All six tutorial documents exist in `docs/tutorials/` and are linked from
      `docs/tutorials/README.md` and `docs/README.md`.
- [ ] Every code snippet in every tutorial passes `cargo check` (or is
      explicitly marked as pseudocode with a prose note explaining the
      deviation).
- [ ] At least four screenshot assets exist in `docs/tutorials/assets/`.

## Risk Assessment

- **Medium**: Code snippets may drift from the API as other stories land
  concurrently. _Mitigation_: Where possible, lift snippets verbatim from the
  GUP-103 examples suite rather than writing new standalone code; add a CI lint
  step (or a note in CONTRIBUTING.md) reminding contributors to update tutorials
  when public APIs change.
- **Low**: GUP-280 (API Reference) is not yet complete, so tutorial links to API
  docs will be placeholder URLs. _Mitigation_: Use descriptive text anchors
  (`"See the PlotBuilder API reference"`) that can be turned into hyperlinks
  once GUP-280 delivers rendered output; mark such placeholders with a
  `TODO(GUP-280):` comment in the Markdown source.
- **Low**: Screenshot capture requires a GPU-capable environment. _Mitigation_:
  Screenshots can be captured in the development environment and committed as
  static assets; they do not need to be regenerated in CI. Document the
  reproduction command in `docs/tutorials/README.md`.
- **Low**: Tutorial 5 (Streaming Data) depends on `StreamingDataSource` API that
  is present in the codebase but whose ergonomic builder layer is planned in
  GUP-244 (not yet complete). _Mitigation_: Tutorial 5 should be written against
  the existing `StreamingDataSource` trait and `StreamingScatterPlot` struct,
  which are already stable. Add a forward-reference note for the builder API.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
