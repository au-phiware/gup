---
implementer: sonnet
implementer-reason:
  mapping per mark kind, draw order and the skip rule are fixed in Decisions;
  follows the CPU mirrors picking already uses
---

# GUP-467: SVG Output Draws Marks

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-18)
**Created**: 2026-10-18

## Context

`SvgTarget` (`crates/gup-core/src/svg.rs`) writes guides (axes, ticks, text,
legends) but not data marks. Its module docs say "Data marks are not written
yet", and a scene with an `ItemKind::Marks` item is an error; callers pass
`Scene::guides` to get the rest. An SVG export of a plot therefore has no data.
Dogfood task 3's "SVG has data points" check is a tracked expected failure until
this lands (GUP-465 owns the dogfood tasks).

GUP-445 added CPU read-back (`ColumnStore::cpu_values`, `cpu_eval`) that picking
uses to evaluate mark positions on the CPU. GUP-430 gave marks fill-opacity and
stroke, and GUP-431 added `DrawOrder` (`crates/gup-core/src/order.rs`). The
pieces needed to emit vector marks exist.

## Read First

- `crates/gup-core/src/svg.rs` lines 1–30 (module docs), 350–380
  (`VectorTarget for SvgTarget`, the `Marks` error).
- `crates/gup-core/src/scene.rs` lines 130–175 (`ItemKind::Marks`, `MarkBatch`).
- `crates/gup-core/src/selection.rs` (`Retain` at line 57; how picking evaluates
  CPU positions) and `crates/gup-core/src/column.rs` line 1042 (`cpu_values`).
- `crates/gup-core/src/order.rs` (`DrawOrder`, `draw_entries`,
  `polyline_spans`).
- `crates/gup-core/tests/svg.rs` and `tests/msaa.rs` (resvg rasterising and
  comparison helpers; resvg 0.45 is a dev-dependency).

## User Story

> "As a developer exporting a chart for a report, I want the SVG to contain the
> data marks as vector shapes, so that the file is a complete, scalable copy of
> what the window shows."

## Acceptance Criteria

### AC1: Marks are written

- [x] An `ItemKind::Marks` item no longer errors in `SvgTarget`; it writes one
      `<g>` per layer with the layer's clip.
- [x] Circle writes `<circle>`, Rect `<rect>`, Segment `<line>` with
      `stroke-linecap="square"`, Line `<polyline>` with
      `stroke-linejoin="round"` and `stroke-linecap="round"`.
- [x] Fill colour, `fill-opacity` and stroke (width, colour) come from the
      resolved GUP-430 values; per-row colours are written per element.
- [x] Rows are written in `DrawOrder` order; null rows (NaN position or null
      mask) are skipped, as on the GPU.

### AC2: Matches the PNG goldens

- [x] For each existing gup-core golden scene (the PNG tests in `tests/*_png.rs`
      and `tests/msaa.rs`), a test renders the SVG, rasterises it with resvg at
      the PNG's size and compares to the PNG golden within that harness's
      existing tolerance. Scenes whose layers are skipped under AC3 are listed
      in the test with the reason and are excluded, not loosened.
- [x] The implementer reads at least two of the rasterised SVGs (one scatter,
      one line) and confirms by eye that marks lie inside the plot area.

### AC3: Layers without CPU positions

- [x] A layer whose mark has no CPU description (user `#[derive(Mark)]` marks)
      or whose columns are not retained (`Retain::GpuOnly`, or `Auto` above
      `Retain::AUTO_MAX_ROWS`; `Retain::Rows` drops CPU columns of uploaded
      chunks) is skipped, with an XML comment
      `<!-- layer N skipped: <reason> -->` in the SVG, not an error.
- [x] The story retrospective lists which layers and which `Retain` modes hit
      this.

### AC4: Dogfood and size

- [x] The dogfood task 3 "SVG has data points" expected failure is removed (grep
      for the string in `examples/` and `docs/`; if the task binary is owned by
      an unmerged GUP-465 change, remove it on whichever side exists and note it
      in the retrospective).
- [x] The file size of the SVG for a 10 000-point scatter is reported in the
      retrospective (bytes, and gzipped).

### AC5: Docs and callers

- [x] The `svg.rs` module docs no longer say marks are unwritten; they describe
      the skip rule. Every caller or doctest that uses `Scene::guides` only to
      avoid the error is updated or left with a comment saying why.

## Decisions

- Positions and colours are evaluated by the CPU mirrors picking uses
  (`cpu_values`/`cpu_eval`), not a new path, so SVG and picks agree.
- Pixel coordinates are written with at most 2 decimals.
- Marks are always vector; there is no raster fallback above a mark-count
  threshold in this story (the size in AC4 informs a later one).
- Skipping, not erroring, for layers with no CPU positions: an export that drops
  one layer with a comment beats none.
- Culled chunks (GUP-457) are not written: the SVG is the view, as the window
  shows it, and a chunk culled cannot reach a pixel of the plot rect. Culling on
  or off rasterises the same (see Evidence).
- Built-in marks are identified by `TypeId` (Circle, Rect, Segment, Line);
  everything else is a user mark and is skipped with a comment.
- A shared `ScaleRef` is read live by the mirror (not frozen): write a scene out
  before changing its scales.
- **Comparison (resolved 2026-10-18, option A, orchestrator).** Both images are
  averaged over 4x4 pixel blocks before the harness's tolerance (delta E 3.0,
  0.25% of blocks) applies, plus a cap of 5.0 on any block's delta E. Per pixel,
  9 of 17 scenes failed on antialiasing alone: resvg (tiny-skia) quantises
  vertical coverage to quarter steps (the Apr bar's top edge at y = 101.65:
  exact coverage 0.35, GPU 0.353, resvg 0.247), while geometry and colour match.
  Block averaging keeps every scene under tolerance (worst 0.10% of blocks, max
  block delta E 4.7). Seeded proofs, each of which FAILS the comparison: one of
  120 scatter circles shifted by 1 px (block delta E 11.5), in the wrong colour
  (60.8), missing (39.3).
- Code lives in `svg.rs` (or a child module `svg/marks.rs` if it exceeds ~400
  lines); no change to `VectorTarget`'s signature unless escalated.

## Escalation Triggers

- A mark kind whose CPU positions can't be evaluated for retained columns
  (`Retain::Columns`/`GpuOnly`): decide "skip with comment" and list which
  layers (AC3); stop if more than the user-mark case hits.
- `MarkBatch` does not carry what `SvgTarget` needs (scales, columns, order), so
  `VectorTarget` must change.
- A golden differs beyond tolerance because of antialiasing, not geometry.

## Out of Scope

- A raster fallback for very large mark counts.
- Embedding fonts (already `SvgOptions::embed_font`).
- Dogfood task rewrites (GUP-465).

## Technical Tasks

- [x] Find how picking evaluates CPU positions and expose a `pub(crate)` helper
      the SVG writer can call per layer.
- [x] Implement per-mark emitters and draw-order iteration.
- [x] Write the skip comment path.
- [x] Add the golden-comparison test and the 10k size test.
- [x] Remove the dogfood expected failure; update docs.

## Dependencies

### Prerequisite Stories

- GUP-445 ✅ — CPU read-back of columns.
- GUP-430 ✅ — fill-opacity and stroke.
- GUP-431 ✅ — `DrawOrder`.

### Enables Stories

- GUP-465: Dogfood Tasks on `gup-core` — task 3's SVG check passes.

## Testing Strategy

- **Unit tests**: per-mark element text; null skipping; skip comment.
- **Integration tests**: SVG vs PNG golden for every golden scene (AC2).
- **Visual validation**: read the rasterised SVGs named in AC2.
- **Performance**: report the 10k-point size.

## Success Metrics

- [x] Every golden scene with CPU positions matches its PNG via resvg.
- [x] No `ItemKind::Marks` error remains in `SvgTarget`.

## Risk Assessment

- **Medium**: resvg antialiasing differs from the GPU's, so tolerances may need
  per-scene tuning. _Mitigation_: use the existing MSAA harness tolerance and
  escalate rather than loosening.
- **Low**: large SVGs for big scatters. _Mitigation_: report only.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `src/vector.rs`: `LayerVector` (a `CpuLayer` snapshot or a skip reason) on
  every `MarkBatch`; evaluates a batch's visible spans into `VectorMark`s (nulls
  hide a row as on the GPU, a polyline splits at nulls and style changes).
  `src/svg.rs` writes them; the `Marks` error is gone.
- CPU colour mirrors: hidden `ShaderFn::cpu_color` / `EncodeFn::cpu_color`,
  implemented for Sequential, Diverging, Categorical, `ScaleRef`, chains and
  `#[wgsl_function]` colour functions with a `CpuMirror` (autoref probe).
- Chunk CPU bytes are `Arc`-shared (copy on write on append), so the snapshot is
  free; it is taken only when `Retain` keeps CPU columns.
- Tests: `tests/svg_marks.rs` (5: seeded proofs, elements, skip, cull, 10k),
  `common/svg_marks.rs` wired into 17 golden scenes in the `*_png.rs` tests,
  `msaa.rs` scene, `derive_mark.rs` user-mark skip, `svg.rs` updated (the whole
  scene now writes; guides compared as before).

## Evidence

- Skipped layers: user `#[derive(Mark)]` marks; `Retain::GpuOnly` and
  `Retain::Rows` (and `Auto` above 10M rows); `Retain::Columns` and `Auto`
  render. A colour function with no CPU mirror skips with a reason.
- 10 000-point scatter: 570 108 bytes, 105 631 gzipped.
- Culling: zoomed to 4 of 64 chunks, 4 096 circles written against 65 536
  unculled; the rasterisations differ in 4 edge pixels (one quarter-step of
  coverage, resvg placing its clip layer by group bounds), nothing missing.
- Gates: `mask all-fix` clean; gup-core tests pass; `mask old-path-loc` 28910;
  `mask wasm-browser` PASS. `mask wasm-size` / `perf-budget`: scatter 323 634 B
  gz (recorded 315 804, +2.5%, FAIL on the +2% rule), over_wgpu 291 274 B
  (recorded 283 442, +7 832 B, limit 400 000 pass); zoom and culled timings and
  counters pass; `pipeline.link_create.median_ms` 1.333 against 1.332 (noise,
  not touched). The WASM growth is the vector code (the scatter wasm crate
  writes guides only, so it is mostly `cpu_color`, `Arc` chunks and the
  snapshot); the budget is not re-recorded here (CI does that).
- Dogfood t3: no "SVG has data points" expected failure exists in
  `tests/visual_regression/expected_failures.toml` or `examples/`; only
  GUP-394's historical table mentions it. Note for GUP-465: the SVG now has data
  marks, so its task 3 check should simply pass.
- Callers of `Scene::guides` left on purpose, with comments: the grid test
  (counts grid lines) and the WASM scatter size crate.

## Retrospective

**Completed**: 2026-10-18

### Key Technical Learnings

#### Comparing two rasterisers

- **Challenge**: per-pixel ΔE failed 9 of 17 scenes with correct geometry.
- **Solution**: average 4x4 blocks, keep the tolerance, add a per-block cap, and
  prove with seeded errors that real defects still fail.
- **Pattern**: when a tolerance is loosened by method, seed errors that must
  fail; a single mark of 120 is the smallest defect worth catching.

#### Snapshotting data without keeping it alive

- **Challenge**: a `Scene` carries no layer, and layers drop CPU data per
  `Retain`.
- **Solution**: `Arc`-shared chunk bytes with copy on write, snapshot only when
  the policy keeps columns.
- **Pattern**: share immutable chunks; let the policy decide ownership.

### Architectural Decisions

#### Frozen mirrors but live shared scales

- **Decision**: column encodings are cloned into mirrors at prepare; a
  `ScaleRef` clone still shares state.
- **Trade-off**: exporting after a zoom without re-resolving mixes new scales
  with old guides.
- **Future**: freeze `ScaleRef` mirrors if that bites.

### Development Workflow Insights

- The worktree guard rejects compound shell commands with variables or heredocs;
  write scripts with the Write tool and run them plainly.

### Follow-up Stories

None new. A raster fallback above a mark-count threshold stays out of scope; the
10k size (570 KB, 106 KB gz) is the input for that decision.

### WASM size follow-up (2026-10-18)

`mask wasm-attrib` against 8b6af25: gup_core +32 KB raw, all of it SVG-only code
the scatter harness linked because it calls `SvgTarget::render`:
`svg::write_layer` (with the inlined mark evaluation) 10.6 KB, the per-layer
`vector()` snapshot inside `Selection::prepare` (about +7.9 KB over its three
instantiations), `.rodata` +3 KB (format strings), and the `Arc`, mirror and
`cpu_color` plumbing. That was +7 832 B gz. The marks code is now behind the
default-on `svg` feature of gup-core (`vector`, `CpuColumns`,
`EncodeFn::cpu_color`, `DynColumnEncoding::mirror`, `SvgTarget`'s mark writer);
the harness builds without it. `ShaderFn::cpu_color` and the macro probe stay
ungated (the macro cannot see gup-core's features, and they are dead-stripped).
Without the feature `SvgTarget` still writes guides and a scene with a data
layer is an error. Sizes: scatter 316 187 B gz (+385 B over the recorded 315
804), over_wgpu 283 827 B (+385 B). `mask all-check` now also runs
`cargo clippy -p gup-core --lib --no-default-features`.
