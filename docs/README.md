# Gup Documentation

This directory contains the documentation for the Gup project; the
[index](./INDEX.md) lists every file.

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

How work is done (stories, gates, dogfood), what RFC-001 built, and the tracks
N0–N6 that come next.

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

## Integrations

### [🖼️ egui Integration](./EGUI_INTEGRATION.md)

`gup-egui`: put a Gup chart in an egui panel, rendered on egui's own GPU device
and sampled directly by egui.

### [🐦 Bevy Integration](./BEVY_INTEGRATION.md)

`gup-bevy`: embed a Gup chart in a Bevy app as a sprite texture, updated as your
data changes.

## Performance

### [📈 Performance Guide](./PERFORMANCE_GUIDE.md)

A pointer to the performance budgets in
[`crates/gup-core/PERF_BUDGETS.md`](../crates/gup-core/PERF_BUDGETS.md), and the
mask tasks that measure them.

## Tutorials

> The tutorials below are out of date: they were written for the API before the
> RFC-001 flip and do not compile against `gup` today. They are rewritten as
> tested Markdown in track N1 of the strategic review. Where they disagree with
> [Technical Approach](./TECHNICAL_APPROACH.md), it wins.

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

## Archive

### [🗄️ Archived documents](./archive/README.md)

Accessibility and research documents from before the RFC-001 core: history only,
not a description of the current code.

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
