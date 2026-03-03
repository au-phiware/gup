# GUP-285: Legend Rendering System

## Story Overview

**Initiative**: Chart Builders **Status**: 💡 New **Created**: 2025-07-27

## Context

GUP-245 (Bar Chart Builder) introduced grouped and stacked bar charts that store
per-series labels and palette colours. However, no visual legend is currently
rendered alongside the chart. A dedicated legend rendering system is needed so
that readers can identify which colour corresponds to which series.

## User Story

> "As a chart consumer, I want an automatically generated legend that maps
> series colours to their names, so that I can interpret grouped and stacked
> charts without external documentation."

## Acceptance Criteria

- [ ] `LegendRenderer` reads series labels and palette colours from
      `ComposedChart` metadata
- [ ] Legend entries are positioned automatically (right, bottom, or floating)
- [ ] Legend renders using GPU-instanced rectangles and text glyphs
- [ ] Works with bar, line, and scatter charts that have multi-series encoding

## Dependencies

### Prerequisite Stories

- GUP-245: Bar Chart Builder ✅ — provides series label storage
- GUP-018: Observable Plot Chart Builders ✅ — provides ComposedChart
  infrastructure

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
