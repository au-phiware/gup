# Gup Documentation

This directory contains comprehensive documentation for the Gup project.

## Core Documentation

### [📋 Mission and Goals](./MISSION_AND_GOALS.md)

Project vision, objectives, and success metrics. Start here to understand what
Gup aims to achieve and why.

### [🔧 Technical Approach](./TECHNICAL_APPROACH.md)

Deep dive into Gup's unified shader function architecture - the core innovation
that enables GPU-accelerated composable visualizations.

### [📊 Market Analysis](./MARKET_ANALYSIS.md)

Competitive landscape analysis and strategic positioning. Understand how Gup
compares to existing solutions and its market opportunity.

### [🚀 Implementation Strategy](./IMPLEMENTATION_STRATEGY.md)

Development roadmap with detailed phases, timelines, and success criteria. The
definitive guide to how Gup will be built.

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

## Feature Guides

### [📐 Grid System](./GRID_SYSTEM.md)

Comprehensive documentation for the GPU-accelerated grid line rendering system.
Covers quick start, API reference, configuration guide with theme presets,
tutorials, performance guide, and troubleshooting.

### [📊 Mark System](./mark-system/README.md)

Comprehensive documentation for the mark system: architecture overview,
component relationships, API reference for Mark trait, MarkRegistry, and
MarkRenderer, and a performance optimization guide.

### [🔲 Custom Mark Guide](./CUSTOM_MARK_GUIDE.md)

Guide for building custom mark types and extending the mark system.

### [🔀 Transpilation Validation Report](./transpilation-validation-report.md)

Analysis of the Rust-to-WGSL transpilation system: approach comparison,
technical validation results, performance benchmarks, developer experience
assessment, and implementation recommendation.

### [🔍 Existing Solutions Analysis](./existing-solutions-analysis.md)

Comprehensive analysis of existing Rust-to-GPU compilation solutions: rust-gpu
ecosystem, naga/WebGPU assessment, academic and industry research review,
alternative approaches (eDSLs, runtime compilation, hybrid), weighted comparison
matrix, and strategic recommendation for Gup's transpilation approach.

### [🔤 Text Rendering Architecture](./text-rendering-architecture.md)

Technical details of the SDF-based GPU text rendering pipeline.

## Quick Navigation

**New to Gup?** Start with [Mission and Goals](./MISSION_AND_GOALS.md) to
understand the project vision.

**Want technical details?** Read [Technical Approach](./TECHNICAL_APPROACH.md)
for the unified shader architecture.

**Interested in market positioning?** See
[Market Analysis](./MARKET_ANALYSIS.md) for competitive landscape.

**Planning to contribute?** Check
[Implementation Strategy](./IMPLEMENTATION_STRATEGY.md) for development phases.

---

**These documents represent the current, definitive specification for Gup. All
earlier versions have been archived for historical reference.**
