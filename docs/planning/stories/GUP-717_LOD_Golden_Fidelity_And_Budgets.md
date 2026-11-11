---
implementer: sonnet
implementer-reason:
  measurement and a golden over a finished feature; thresholds come from the
  spike's table and every AC is a command or a named image
touches:
  [
    crates/gup-core/tests/lod.rs,
    crates/gup-core/examples/zoom_bench.rs,
    tests/golden/,
  ]
---

# GUP-717: LOD Golden, Fidelity Check and Budget Rows

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-11

## Context

GUP-716 ships `Lod::Auto` with the tier one finer than the spike's decision. The
spike's fidelity numbers were measured at 10M rows in a prototype (RFC-001
"GUP-631 findings", Fidelity). This story pins them in the library: a golden of
a LOD scatter, a test that compares it with the exact draw by ΔE00 (so the bias
and the grid cannot drift), and the benchmark rows the orchestrator records in
`PERF_BUDGETS.md`.

## Read First

- RFC-001 "GUP-631 findings": "The table" and "Fidelity" (lines ~4370-4452).
- `crates/gup-core/examples/zoom_bench.rs`: options (lines 8-60), parsing
  (108-160), the `draws.median` metric (~532).
- `crates/gup-core/PERF_BUDGETS.md` lines 20-80 (the machine, the
  `zoom_culled.*` rows).
- `crates/gup-visual-regression`: `Rgba8::delta_e`, golden helpers; an existing
  golden test such as `crates/gup-core/tests/scatter_png.rs`.
- `crates/gup-core/tests/lod.rs` (GUP-716).

## User Story

> "As the maintainer, I want LOD's picture and cost pinned by tests and budget
> rows, so that a change to tiers or the grid cannot quietly degrade a large
> scatter."

## Acceptance Criteria

### AC1: Fidelity against the exact draw

- [ ] `tests/lod.rs`: a 1,000,000-row scatter (sorted x over 0..1000, y a seeded
      uniform-step random walk, as `scale_bench`), 400 x 300 chart, rendered
      with `Lod::Auto` and `Lod::Off` in full view and in a 10x zoom on both
      axes (`Chart::x_view`/`y_view`). Share of pixels with ΔE00 > 10
      (`delta_e`): at most **1.0% of the plot rect's pixels** in the full view
      and at most **2.0%** in the zoom view (spike with bias: 0.63% / 1.24% at
      10M, 1080p). The test prints both shares.
- [ ] Draw counts: under `Auto` the 1M scatter's drawn instances are under 10%
      of the rows in the full view (assert), equal to rows when zoomed so far in
      that tiers run out.

### AC2: Golden

- [ ] `tests/golden/lod_scatter_1m.png`, the `Auto` full view, blessed by the
      repository's golden procedure and read by eye: a continuous band of discs,
      the plot's axes and ticks, no marks outside the plot area, no visible gaps
      inside the band. The retrospective names the file and what was read.

### AC3: Benchmark and budget rows

- [ ] `zoom_bench` gains `--lod <auto|off>` (default off: its current output is
      unchanged) and `--columns` if it cannot already feed `Columns`, printing
      with `--metric-prefix lod`: `lod.gpu_pass.median_ms`,
      `lod.gpu_pass.p95_ms`, `lod.cpu.median_ms`, `lod.draws.median`,
      `lod.instances.median`, for a 10M-row layer in full view and zoom
      (`--zoom` as it has today). In release on the development machine, load
      under 2: GPU median at most 8 ms in both views (spike, with bias, at
      1080p: 3.34 / 3.89), CPU median at most 1.5 ms (spike 0.38 / 0.63).
- [ ] The retrospective proposes `lod.gpu_pass.median_ms`, `lod.cpu.median_ms`
      and `lod.build_ms_per_10m` (GUP-714) as `PERF_BUDGETS.md` rows, with the
      measured values and the spike's for comparison (the orchestrator records).

## Decisions

- **Reference is `Lod::Off` in the same test**, not a stored image, so the
  threshold tests the tier rule and not the GPU's antialiasing.
- **Plot-rect denominator** (the stricter of the spike's two) for the full view.
- **Zoom threshold 2.0%** where the spike measured 1.2%: the zoom's residual is
  antialiasing at the edges and varies with size; at 400 x 300 it is a larger
  share.
- **No new public API.**

## Escalation Triggers

- The full-view share exceeds 1.0% of the plot rect (the bias or the grid is not
  as in the spike): report the measured share and the image.
- `zoom_bench` cannot feed `Columns` at 10M without the rows path's memory (stop
  and report; do not benchmark the rows path as a substitute).
- GPU median above 8 ms in either view.

## Out of Scope

- Pick, hover, SVG (GUP-718); appends (GUP-719); unsorted layers (GUP-720);
  recording `PERF_BUDGETS.md` (the orchestrator).

## Technical Tasks

- [ ] Fidelity tests and draw-count assertions in `tests/lod.rs`.
- [ ] The golden and its blessing.
- [ ] `zoom_bench --lod`; run it three times at 10M, take medians.
- [ ] Retrospective with the proposed rows.

## Dependencies

### Prerequisite Stories

- GUP-716: `Selection::lod` and Per-Frame Tier Draws 📋.

### Enables Stories

- GUP-632's README wording (the budget rows).

## Testing Strategy

- **Integration tests**: AC1 (two views) and AC2.
- **Performance**:
  `zoom_bench --lod auto --points 10000000 --metric-prefix lod`.

## Success Metrics

- [ ] Shares under 1.0% / 2.0%; GPU median under 8 ms at 10M.

## Risk Assessment

- **Low**: 1M rows at 400 x 300 is denser per pixel than the spike's 10M at
  1080p and the shares may differ. _Mitigation_: the escalation trigger reports
  it rather than loosening the threshold.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the golden read)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code changes
- [ ] WASM size: unchanged: no library code change
- [ ] `mask perf-budget` table pasted and proposed rows listed
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
