# Gup Documentation

This directory contains comprehensive documentation for the Gup project.

## Core Documentation

### [📋 Mission and Goals](./MISSION_AND_GOALS.md)

Project vision, objectives, and success metrics. Start here to understand what
Gup aims to achieve and why.

### [🔧 Technical Approach](./TECHNICAL_APPROACH.md)

How Gup works today: shader functions with CPU mirrors, the GPU column store,
typed marks, scales, `Plot` and `Scene`, render targets, interaction, hosts and
the gates. Every snippet is a doctest.

### [📊 Market Analysis](./MARKET_ANALYSIS.md)

Competitive landscape analysis and strategic positioning. Understand how Gup
compares to existing solutions and its market opportunity.

### [🚀 Implementation Strategy](./IMPLEMENTATION_STRATEGY.md)

How work is done (stories, gates, dogfood), what RFC-001 built, and what is next
(T5 builders, T6 gallery and docs-as-tests, T7 crate split and LOD). No
milestones or dates.

## Research Documents

The following research documents provide foundational analysis that informed
Gup's design:

### [📚 D3.js Research](./01_D3_RESEARCH.md)

Comprehensive analysis of D3.js architecture, au-phiware plugins, and the
principles that made D3 successful.

### [⚡ WebGPU vs DOM Analysis](./02_WEBGPU_VS_DOM_ANALYSIS.md)

Technical comparison of DOM/SVG-based visualization vs GPU-accelerated
approaches, highlighting fundamental paradigm differences.

### [🦀 Rust Visualization Landscape](./03_RUST_VISUALIZATION_LANDSCAPE.md)

Analysis of existing Rust visualization libraries, their strengths/limitations,
and opportunities for Gup.

## Archive

The [`archive/`](./archive/) directory contains earlier versions of documents
that have been superseded by the core documentation above. These provide
historical context and show the evolution of Gup's design.

## Tutorials

> The tutorials and feature guides below were written for the crate that `gup`
> was before the RFC-001 flip (GUP-488). Where they disagree with
> [Technical Approach](./TECHNICAL_APPROACH.md), it wins. The tutorials are
> rewritten as tested docs in the strategic review's T6 track.

### [📖 Tutorial Series](./tutorials/README.md)

Six step-by-step tutorials that take you from your first chart to advanced
topics. Start with [Getting Started](./tutorials/01_getting_started.md) if you
are new to Gup.

1. [Getting Started](./tutorials/01_getting_started.md) — create your first
   scatter chart
2. [Data Binding](./tutorials/02_data_binding.md) — bind custom structs with
   `Selection<T, M>`
3. [Custom Shader Functions](./tutorials/03_custom_shader_functions.md) — GPU
   transforms with `#[wgsl_function]`
4. [Interactions](./tutorials/04_interactions.md) — hover, click, brush,
   zoom/pan
5. [Streaming Data](./tutorials/05_streaming_data.md) — live data with
   `Appender`
6. [Custom Marks](./tutorials/06_custom_marks.md) — implement new mark types

## Feature Guides

### [📐 Grid System](./GRID_SYSTEM.md)

Comprehensive documentation for the GPU-accelerated grid line rendering system.
Covers quick start, API reference, configuration guide with theme presets,
tutorials, performance guide, and troubleshooting.

### [📊 Mark System](./mark-system/README.md)

Comprehensive documentation for the mark system: architecture overview,
component relationships, API reference for the Mark trait, and a performance
optimization guide.

### [🔲 Custom Mark Guide](./CUSTOM_MARK_GUIDE.md)

Guide for building custom mark types and extending the mark system.

### [🔍 Existing Solutions Analysis](./existing-solutions-analysis.md)

Comprehensive analysis of existing Rust-to-GPU compilation solutions: rust-gpu
ecosystem, naga/WebGPU assessment, academic and industry research review,
alternative approaches (eDSLs, runtime compilation, hybrid), weighted comparison
matrix, and a research-era recommendation that was not adopted.

### [🔤 Text Rendering Architecture](./text-rendering-architecture.md)

Technical details of the SDF-based GPU text rendering pipeline.

### [⚡ WASM Performance Benchmarks](./WASM_PERFORMANCE.md)

Cross-platform performance benchmarking infrastructure: WASM vs native
comparison tooling, browser compatibility matrix, and platform-specific
performance characteristics.

### [📈 Performance Guide](./PERFORMANCE_GUIDE.md)

Performance targets, profiling infrastructure, benchmarking workflow,
optimization tips, and cross-platform considerations.

## Accessibility

### [♿ WCAG 2.1 AA Conformance Statement](./accessibility/WCAG_2_1_AA_CONFORMANCE.md)

Gup's WCAG 2.1 AA conformance declaration, including technologies relied upon,
non-applicable criteria, and known limitations.

### [📋 WCAG 2.1 AA Audit](./accessibility/WCAG_2_1_AA_AUDIT.md)

Criterion-by-criterion evaluation of all 50 WCAG 2.1 Level A and AA success
criteria with Pass/Fail/N-A verdicts and evidence.

### [🔧 CI Accessibility Checks](./accessibility/CI_ACCESSIBILITY.md)

How automated accessibility validation works in CI: test commands, failure
interpretation, and severity levels.

### [🧪 Accessibility Testing Guide](./accessibility/TESTING_GUIDE.md)

Consumer-facing guide for validating WCAG compliance in applications built on
Gup: axe-core scanning, screen reader testing, keyboard navigation, and colour
contrast checking.

## Quick Navigation

**New to Gup?** Start with [Mission and Goals](./MISSION_AND_GOALS.md) to
understand the project vision.

**Want to see examples?** Browse the
[🖼️ Visual Gallery](https://au-phiware.github.io/gup/) for rendered screenshots
of every example, or view the [Examples Index](../examples/INDEX.md) for
runnable code.

**Want technical details?** Read [Technical Approach](./TECHNICAL_APPROACH.md)
for how Gup works today.

**Interested in market positioning?** See
[Market Analysis](./MARKET_ANALYSIS.md) for competitive landscape.

**Planning to contribute?** Check
[Implementation Strategy](./IMPLEMENTATION_STRATEGY.md) for how work is done and
what is next.

---

**These documents represent the current, definitive specification for Gup. All
earlier versions have been archived for historical reference.**
