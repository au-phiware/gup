---
implementer: sonnet
implementer-reason:
  documentation rewrite to a fixed outline; every new snippet is a doctest, so
  accuracy is checked by a command
touches:
  [
    crates/gup-core/src/lib.rs,
    crates/gup-core/Cargo.toml,
    crates/gup-core/src/svg.rs,
    CLAUDE.md,
  ]
---

# GUP-498: Current gup-core Crate Docs

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 🚧 In Progress
**Created**: 2026-10-24

## Context

`gup-core`'s crate-level documentation (`crates/gup-core/src/lib.rs` lines 4–83)
was written during the RFC-001 migration and was not revised at the flip. It
says there is "still one mark ([`Circle`]) and one layer per plot", that legends
"are placed by hand until RFC-001 S7", that the crate "is not re-exported by the
`gup` crate until the RFC-001 S14 flip", that the old `gup` crate's
`RenderContext` and `GupContext` take their device from `Context::shared`, and
that `SvgTarget` writes "guides only". None of this is true any more: there are
four marks and `#[derive(Mark)]`, any number of layers, automatic legends, `gup`
is a facade over `gup-core`, the old types are deleted, and `SvgTarget` writes
data marks with the default `svg` feature. The Cargo `description` still reads
"Gup's new core (RFC-001)". These are the first pages docs.rs would show
(STRATEGIC_REVIEW_2026-11 §3, "gup-core crate docs ... N0, first story").

`CLAUDE.md` has the same problem: it says the project is on wgpu v26 (the
workspace is on 27) and documents `mask pack hello-wgpu`, a project that does
not exist, and a WebGL fallback the web target does not have.

GUP-489 made `README.md`, `docs/TECHNICAL_APPROACH.md` and
`docs/IMPLEMENTATION_STRATEGY.md` current and doctest-backed. This story does
the same for the crate docs, using `gup`'s own crate docs (`src/lib.rs`) as the
model.

## Read First

- `crates/gup-core/src/lib.rs` lines 1–165 (crate docs, modules, re-exports).
- `src/lib.rs` lines 1–75 (`gup`'s crate docs: the style and doctest form to
  follow).
- `crates/gup-core/Cargo.toml` lines 1–22 and the `[features]` table.
- `crates/gup-core/src/svg.rs` lines 60–95 (`VectorTarget`, `SvgTarget` docs).
- `crates/gup-core/tests/svg_marks.rs` lines 16–35 and 82–110 (how a plot is
  resolved and written through `SvgTarget`).
- `docs/TECHNICAL_APPROACH.md` "Targets" and "Hosts" sections (find with
  `grep -n '^## ' docs/TECHNICAL_APPROACH.md`), for the facts the crate docs
  summarise.

## User Story

> "As a Rust developer reading gup-core's docs on docs.rs or in my editor, I
> want them to describe the crate as it is, with examples that compile, so that
> I can trust them."

## Acceptance Criteria

### AC1: the crate docs follow the outline in Decisions

- [ ] `crates/gup-core/src/lib.rs`'s `//!` block is rewritten to the outline in
      Decisions.
- [ ] This prints nothing:

      ```bash
      grep -n -i -E 'one mark|one layer|placed by hand|not re-exported|S14 flip|RenderContext|GupContext|old `gup`|new core|RFC-001 S[0-9]|guides only' \
        crates/gup-core/src/lib.rs
      ```

- [ ] Every type, function and module named in the crate docs is an intra-doc
      link, and
      `RUSTDOCFLAGS="-D warnings" cargo doc -p gup-core --no-deps --all-features`
      passes.

### AC2: the snippets are doctests

- [ ] The crate docs hold three snippets: the existing `no_run` hello world
      (`gup_core::show`), a headless render that asserts the image's dimensions,
      and an SVG export that asserts the document contains `<circle`. The second
      and third run (no `no_run`, no `ignore`).
- [ ] `cargo test -p gup-core --doc -- --test-threads=1` passes, and the count
      of passed doctests is 2 higher than on `main` (record both counts).

### AC3: item docs and metadata

- [ ] `SvgTarget`'s doc line reads "A resolved scene as an SVG document: axes,
      grid, text and legends, and, with the default `svg` feature, the data
      marks." and its doctest is unchanged.
- [ ] `crates/gup-core/Cargo.toml`: `description` is the text in Decisions; the
      comment above `[dependencies]` no longer says "the old `gup` crate does
      not re-export" (it says `gup` re-exports `gup-core`, and `gup-core` never
      depends on `gup`).
- [ ] This prints nothing (other crates' crate docs):

      ```bash
      grep -n -E 'RenderContext|not re-exported|old `gup`|S14 flip' \
        crates/gup-text/src/lib.rs crates/gup-wgsl/src/lib.rs \
        crates/gup-core-macros/src/lib.rs gup-egui/src/lib.rs gup-bevy/src/lib.rs
      ```

### AC4: CLAUDE.md is current

- [ ] "Dependency Management" says wgpu 27, matching
      `crates/gup-core/Cargo.toml`
      (`grep -n 'wgpu = { version' crates/gup-core/Cargo.toml`).
- [ ] The "WebAssembly", "WebGPU Workflow" and "WebGPU Development Workflow"
      sections are replaced by the "WebAssembly and the browser" section in
      Decisions. The "wgpu Surface Lifetime Management" section is unchanged.
- [ ] `grep -n 'hello-wgpu\|Backends::GL' CLAUDE.md` prints nothing.

## Decisions

- **Crate-docs outline** (in this order; plain words, no RFC step names; at most
  about 110 `//!` lines):
  1. `# gup-core` and one paragraph: the engine behind the `gup` crate, which
     re-exports all of it; applications depend on `gup`, host integrations
     (`gup-egui`, `gup-bevy`) on `gup-core`; `gup-core` never depends on `gup`.
  2. `## What is in it`: bullets, each with links — one [`Context`] (owned or
     wrapping a host's wgpu device); the GPU [column store](mod@column) (keep
     the current bullet's facts); typed [channels](channel); [scale]s:
     [`Linear`], [`Pow`], [`Log`], [`Symlog`], [`Time`]; [`Band`], [`Point`];
     [`Sequential`], [`Diverging`], [`Categorical`]; each a [`ShaderFn`] with an
     f64 [`CpuMirror`], with a [`Legend`]; marks [`Circle`], [`Rect`],
     [`Segment`], [`Line`], and your own with `#[derive(Mark)]` and
     [`wgsl_function`]; [`Plot`] with any number of layers over shared x and y
     scales (and a secondary [`Plot::y_right`]), axes, grid, legends and title
     laid out for you, styled by [`Theme`]; the [`scatter`], [`line`] and
     [`bar`] builders ([`ChartExt`]); interaction ([`Input`], [`Response`],
     picking, tooltips, pan, zoom and brush) and streaming ([`Appender`]);
     targets [`ImageTarget`], [`TextureTarget`], [`WindowTarget`], the web
     canvas and [`SvgTarget`]; shader composition at build time (keep the
     current bullet's facts).
  3. `## Hello, world`: the current `no_run` snippet, unchanged.
  4. `## Without a window`: a `line` chart of 20 rows rendered with
     `ChartExt::render(&Context::shared()?, 640, 400)`, asserting
     `image.dimensions() == (640, 400)`, ending
     `# Ok::<(), gup_core::Error>(())`.
  5. `## SVG`: a `Plot` with a `circles` layer of 5 rows, resolved with
     `plot.resolve(&cx, 320.0, 200.0)?`, written through `SvgTarget::new()` and
     `VectorTarget::render(&resolved.scene)`, asserting
     `svg.svg().contains("<circle")`.
  6. `## Features`: a table — `window` (default): [`WindowTarget`], `show`,
     `GupApp`; `svg` (default): data marks in [`SvgTarget`] (without it, guides
     and text only); `web` (wasm32 only): `web::CanvasApp`. Write the cfg-gated
     names as code, not links, so `cargo doc` without the feature does not warn.
  7. `## Contracts`: the current four bullets, except the first: "**One device
     per process by default.** [`Context::shared`] is the process-wide context;
     host integrations wrap their host's device with [`Context::from_wgpu`]
     instead."
- **Cargo description**: "The engine behind gup: a GPU column store, typed
  channels, scales with CPU mirrors, marks, layout and render targets".
- **CLAUDE.md "WebAssembly and the browser"** (replaces the three sections):

  ```markdown
  ## WebAssembly and the browser

  - `mask wasm-browser` - build the reference scatter
    (`crates/gup-core/wasm-size/scatter`) for wasm32 and render it in headless
    Chromium through WebGPU (SwiftShader); the smoke test CI runs
  - `mask wasm-size` - its gzipped size against the budget in
    `crates/gup-core/PERF_BUDGETS.md`; `mask wasm-attrib` - where the bytes go
  - The web target is WebGPU only (`web::CanvasApp`, feature `web`); there is no
    WebGL fallback
  - `chromium-webgpu` (from the flake) launches Chromium with WebGPU enabled;
    check chrome://gpu
  ```

  and "Dependency Management" becomes: "Do not downgrade wgpu. The workspace is
  on wgpu 27 (`Cargo.lock`), and hosts must use the same major version."

- **Doctests run on CI's lavapipe**: the Visual regression workflow runs
  `cargo test -p gup-core --doc`, so GPU doctests are allowed, as in `gup`'s.

## Escalation Triggers

- A doctest in the outline cannot be written with the public API as it is (for
  example `Resolved::scene` is not public), or needs a feature that is off by
  default.
- `cargo doc` warns about a link the outline asks for and the item does not
  exist under that name.
- A fact in the outline contradicts `docs/TECHNICAL_APPROACH.md` or the code.

## Out of Scope

- `#![warn(missing_docs)]` and item docs beyond `SvgTarget`'s first line: N1.
- Publish metadata, versions and descriptions of the other crates: N3.
- Tutorials and feature guides: N1 and GUP-499.
- The generic `mask pack/serve/start/run` tasks in `maskfile.md` (left as they
  are).

## Technical Tasks

- [ ] Start commit within 30 minutes; record `cargo test -p gup-core --doc`'s
      passed count on the unchanged tree first.
- [ ] Rewrite the crate docs; run the doctests; checkpoint.
- [ ] Fix `SvgTarget`'s doc line, the Cargo description and comment.
- [ ] Edit `CLAUDE.md`.
- [ ] Run the AC greps, `cargo doc` with `-D warnings`, `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-489: RFC-001 S14d Revise the Architecture Docs ✅ — the current
  architecture text and the doctest pattern.
- GUP-496: Checkpoint Script 📋 — soft, for the DoD's checkpoint line.

### Enables Stories

- N1 docs track: `#![warn(missing_docs)]` starts from current crate docs.

## Testing Strategy

- **Doctests**: `cargo test -p gup-core --doc -- --test-threads=1` (two new
  running snippets).
- **Docs build**:
  `RUSTDOCFLAGS="-D warnings" cargo doc -p gup-core --no-deps --all-features`.
- **Search**: the AC1, AC3 and AC4 greps print nothing.

## Success Metrics

- [ ] Passed doctests: `main` + 2.
- [ ] The crate docs are at most about 110 `//!` lines (they are 80 now).

## Risk Assessment

- **Low**: a GPU doctest is slow or flaky on lavapipe. _Mitigation_: small sizes
  (640×400, 20 rows); the existing `gup` doctests already render on CI.

## Definition of Done

- [ ] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test -p gup-core --doc -- --test-threads=1` passes
- [ ] Lint and format clean: `mask all-fix`; `mask all-check` passes
- [ ] `mask wasm-browser`: not needed (docs, a doc comment and Cargo metadata;
      no layout, input or wasm code); say so in the Implementation Summary
- [ ] WASM size: "unchanged: no library code change"
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
