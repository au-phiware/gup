---
implementer: sonnet
implementer-reason:
  documentation rewrite with a fixed outline and sources per section; every
  snippet is compiled as a doctest, so accuracy is checked by a command
---

# GUP-489: RFC-001 Step S14d: Revise the Architecture Docs

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-24)
**Created**: 2026-10-19

## Context

The strategic review's Decision 6 says `IMPLEMENTATION_STRATEGY.md` and
`TECHNICAL_APPROACH.md` are "to be revised once the T3 RFC is accepted, so they
describe the architecture being built", and RFC-001 S14 names the revision as
part of the flip. Both documents still describe the pre-2026-10 design: a
"unified shader function system" with a Rust-to-WGSL transpiler (deleted in
GUP-389), `ShaderFunction` chains spliced as WGSL text (deleted at the flip),
and an initiative plan whose four initiatives are obsolete. The root `README.md`
and `docs/README.md` point readers at them.

After GUP-488, `gup` is `gup-core`. RFC-001's findings sections (S0a to S14)
record what was actually built and why, but they are a design log, not a
description. This story writes the description, short, with every code snippet
compiled.

## Read First

- `docs/TECHNICAL_APPROACH.md` and `docs/IMPLEMENTATION_STRATEGY.md` (whole;
  being replaced).
- `docs/planning/STRATEGIC_REVIEW_2026-10.md` "North star", "Strategy" and
  "Decisions".
- RFC-001 "Summary" (lines 12–42), §3–§9 "Design" subsections, and the findings
  headings
  (`grep -n '^## .*findings' docs/planning/rfcs/RFC-001_Core_Architecture.md`):
  read each findings section's first paragraph and its "Decisions" lines only.
- `crates/gup-core/src/lib.rs` crate docs and `PERF_BUDGETS.md`.
- `README.md` sections "Quick Start", "Architecture Highlights", "Development
  Status", "Examples", "Workspace Crates" and "Parked integration crates";
  `docs/README.md` "Core Documentation".

## User Story

> "As a new contributor, I want two short documents that describe how Gup works
> today and how it is being built, so that I do not learn a design that was
> deleted."

## Acceptance Criteria

### AC1: TECHNICAL_APPROACH.md

- [x] Rewritten to the outline in Decisions, at most 450 lines, describing only
      what exists on `main`.
- [x] Every Rust snippet compiles: the file is included as doctests in
      `src/lib.rs` under `#[cfg(doctest)]` (Decisions);
      `cargo test --doc -p     gup` passes. Snippets needing a window are
      `no_run`.
- [x] Each section links the RFC-001 findings it summarises.

### AC2: IMPLEMENTATION_STRATEGY.md

- [x] Rewritten to the outline in Decisions, at most 250 lines, with no
      milestones, dates or story points.
- [x] Its snippets, if any, are doctests like AC1's.

### AC3: Entry points

- [x] `README.md`: "Quick Start" is the north-star example (the same code as
      `src/lib.rs`'s doctest); "Architecture Highlights" summarises
      TECHNICAL_APPROACH in at most 10 bullets; "Development Status" says
      RFC-001 is implemented and what is next (T5, T6, T7); "Workspace Crates"
      lists the current members; the parked table lists only what is still
      parked.
- [x] `docs/README.md`'s descriptions of the two documents match their new
      content.
- [x] RFC-001's **Status** line adds "Implemented: S0–S14 (GUP-395 to GUP-489)".
- [x] This finds nothing:

  ```bash
  git grep -n "transpil\|ShaderFunction\b\|ComposedChart\|RenderContext\|GupContext" \
    -- README.md docs/README.md docs/TECHNICAL_APPROACH.md docs/IMPLEMENTATION_STRATEGY.md
  ```

## Decisions

- **TECHNICAL_APPROACH outline** (one `##` each):
  1. The idea: data encodings are shader functions with CPU mirrors (`ShaderFn`,
     `then`, `#[wgsl_function]`), composed at build time (`gup-wgsl`).
  2. Data: the GPU column store (chunks, relative origins and hi/lo, validity
     bits, dictionaries, `Retain`, `append`, `Window`).
  3. Marks and typed channels (`#[derive(Mark)]`, compile-time errors).
  4. Scales (the family, ticks and legends from mirrors, `Offset`, bands).
  5. `Plot`, `Layout`, `Scene` and targets (image, window, texture, canvas,
     SVG); the colour policy (sRGB-space blending) and antialiasing (analytic,
     MSAA on demand).
  6. Interaction (input, CPU and GPU picks, tooltips, zoom, pan, brush,
     culling).
  7. Hosts (`show`, `CanvasApp`, `gup-egui`, `gup-bevy`).
  8. Budgets (performance and WASM size, `PERF_BUDGETS.md`).
- **IMPLEMENTATION_STRATEGY outline**: how work is done (stories, the class not
  the instance, user-visible acceptance, dogfood, gates that are never bypassed,
  the orchestrator and implementers); what has been built (RFC-001 S0–S14 in one
  paragraph with links); what is next (T5 builders onto `gup`, T6 flagship
  gallery and docs-as-tests, T7 crate split, LOD and mobile), each pointing at
  the strategic review.
- **Doctests**: in `src/lib.rs`,

  ```rust
  #[cfg(doctest)]
  mod architecture_docs {
      #[doc = include_str!("../docs/TECHNICAL_APPROACH.md")]
      struct TechnicalApproach;
      #[doc = include_str!("../docs/IMPLEMENTATION_STRATEGY.md")]
      struct ImplementationStrategy;
  }
  ```

  Snippets use `gup::`; WGSL snippets are fenced `wgsl` (not compiled).

- **Not rewritten here**: the tutorials (T6), `MISSION_AND_GOALS.md`,
  `MARKET_ANALYSIS.md`, research documents.

## Escalation Triggers

- A section cannot be written without describing behaviour that does not exist
  on `main` (a gap to report, not to paper over).
- A snippet needs an API change to compile.

## Out of Scope

- Tutorials and the gallery: T6.
- Other docs under `docs/` that describe deleted systems (`LOD_SYSTEM.md`,
  `GRID_SYSTEM.md`, …): list them in the retrospective as a follow-up.

## Technical Tasks

- [x] Draft TECHNICAL_APPROACH to the outline; make the snippets compile.
- [x] Draft IMPLEMENTATION_STRATEGY.
- [x] README, `docs/README.md`, RFC status; the grep.

## Dependencies

### Prerequisite Stories

- GUP-488: RFC-001 S14c Delete Old Core and Flip 📋 — snippets use `gup::`.
- GUP-479, GUP-482: secondary y and `Window` 📋 (soft: described if landed, else
  omitted).

### Enables Stories

- T6 docs-as-tests builds on the doctest include pattern.

## Testing Strategy

- **Doctests**: `cargo test --doc -p gup`.
- **Lint**: `mask all-check` (prettier, mdl).

## Success Metrics

- [x] Both documents describe `main` and every snippet compiles.

## Risk Assessment

- **Low**: snippets rot as the API moves. _Mitigation_: they are doctests now.
- **Low**: trybuild `.stderr` churn: none (no gup-core edits).

## Definition of Done

- [x] First commit within 30 minutes of starting; then commit at each checkpoint
      (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] `cargo test --doc -p gup` passes
- [x] Lint and format clean: `mask all-fix`
- [x] `mask wasm-browser`: not needed (docs and a `cfg(doctest)` module only; no
      layout, input or wasm change); say so in the Implementation Summary
- [x] WASM size: no library code changes; record "unchanged"
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `docs/TECHNICAL_APPROACH.md` (449 lines) and `docs/IMPLEMENTATION_STRATEGY.md`
  (114 lines) rewritten to the Decisions outlines. Both are included as doctests
  from `src/lib.rs` under `#[cfg(doctest)]`; `cargo test --doc -p gup` passes
  (10 doc tests plus the two lib examples; one `compile_fail`).
- `README.md` (Quick Start, Architecture Highlights, Development Status,
  Examples, Workspace Crates; the stale `debug` feature section and the deleted
  type-macro section removed), `docs/README.md`, `docs/INDEX.md`,
  `CONVENTIONS.md` and RFC-001's Status line updated. CLAUDE.md and the maskfile
  had no references to deleted tasks. The AC3 grep finds nothing.
- Deviations: the `#[derive(Mark)]` example is a link to the compiled test
  `crates/gup-core/tests/derive_mark.rs` (embedding it broke the 450-line
  limit); the egui, Bevy and canvas hosts are described in prose with links
  (their snippets need a window or wasm32).
- `mask all-check` passes. `mask wasm-browser` not needed (docs and a
  `cfg(doctest)` module only). WASM size unchanged (no library code change).

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Doctests from Markdown

- **Challenge**: A doctest in package `gup` is a crate named `gup`, so
  `#[wgsl_function]` and `#[derive(Mark)]` need an explicit `path = "..."`.
- **Solution**: `path = "arch_energy"` and `path = "arch_docs::square"`.
- **Pattern**: Prettier re-wraps link targets; reference-style links keep the
  line budget.

### Architectural Decisions

#### Line budget

- **Decision**: Findings links are reference-style with one anchored link per
  section, the rest named in text.
- **Trade-off**: Some findings are named, not linked.

### Development Workflow Insights

- Doctests caught three wrong snippets on the first run (type inference, missing
  `path`).

### Follow-up Stories

Docs under `docs/` that still describe deleted systems (not rewritten here, per
Out of Scope): `GRID_SYSTEM.md`, `LOD_SYSTEM.md`, `CUSTOM_MARK_GUIDE.md`,
`TYPE_CONSTRUCTION_GUIDE.md` (the macros are gone), `PERFORMANCE_GUIDE.md`,
`SHADER_PERFORMANCE_BENCHMARKING.md`, `graphics-programming.md`,
`text-rendering-architecture.md`, `mark-system/*`, `tutorials/*` (T6),
`research/*` and `existing-solutions-analysis.md` (historical). Also the
`gup-core` crate docs in `crates/gup-core/src/lib.rs` still say "one mark, one
layer" and "not re-exported until S14", and name the old `RenderContext`. A
docs-cleanup story (delete or archive the stale guides) is worth writing; none
was created here since T6 owns the tutorials.
