---
implementer: sonnet
implementer-reason:
  a headless benchmark over public APIs with fixed phases, options and output
  lines; the runs and their table are the deliverable
touches: [crates/gup-core/examples/scale_bench.rs]
---

# GUP-627: Scale Benchmark: Ingest and Memory

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-04

## Context

The November review says the "billion-point" claim is not demonstrated: rows
enter through per-row CPU closures, LOD is parked, and nothing above 10M rows
has been measured. `PERF_BUDGETS.md` measures zoom frames at 100K rows and at 4M
sorted rows (`zoom_bench`, GUP-457), and streaming through `Window::LastRows`,
but not what it costs to get 10M or 100M rows _in_: the closures, the f64
columns (`ColumnData::Values(Vec<f64>)`, `crates/gup-core/src/column.rs` line
285), the conversion into f32 chunks, the upload, and the peak memory of all of
it.

Track N5's design stories (columnar input, GUP-628; LOD, GUP-631) need these
numbers first, to know which part to attack. This story writes the benchmark and
records them; it changes no library code.

## Read First

- `crates/gup-core/examples/zoom_bench.rs` lines 1–80 (options style, the
  `metric` output) and around line 545 (how metrics are printed).
- `crates/gup-core/src/selection.rs` `evaluate` (lines 757–835): what the first
  resolve does with rows.
- `crates/gup-core/src/column.rs`: `ColumnStore::from_columns` (line 1049;
  public as `gup_core::column::ColumnStore`), `upload` (line 1255),
  `chunk_rows_for` (line 1023).
- `crates/gup-core/src/selection.rs` lines 40–100 (`Retain`).
- `crates/gup-core/PERF_BUDGETS.md` lines 1–60 (the machine and the table; the
  orchestrator, not the worker, records rows there).

## User Story

> "As the maintainer planning track N5, I want measured ingest time and peak
> memory at 1M to 100M rows, split by phase, so that columnar input and LOD are
> designed against numbers, not guesses."

## Acceptance Criteria

### AC1: The benchmark

- [ ] `crates/gup-core/examples/scale_bench.rs` (headless: `ImageTarget`, no
      window) with the options and phases of Decisions, printing one
      `metric scale.<phase>.<unit> <value>` line per measurement (the
      `zoom_bench` format) and a human-readable summary.
- [ ] Runs on lavapipe as well as a GPU (it only needs `Context::new_blocking`).
- [ ] `cargo run -p gup-core --release --example scale_bench -- --rows 100000`
      finishes in under 10 s on the development machine; it is not added to any
      gate.

### AC2: The numbers

- [ ] Release runs on the development machine at 1M, 10M and 30M rows with
      `--retain auto` and `--retain gpu-only`, and at 100M rows with
      `--retain gpu-only` if `MemAvailable` (from `/proc/meminfo`) is at least
      16 GB before the run; otherwise "skipped: N GB available" is recorded.
- [ ] A table in the story's retrospective: for each run, every phase's time,
      the peak RSS after each phase, column bytes uploaded, chunks, and the
      per-row cost of the accessor phase in ns.
- [ ] A paragraph that names the largest phase at 10M and at 100M (or 30M), and
      the bytes per row at peak, for GUP-628 and GUP-631 to cite.
- [ ] A failure at any size (out of memory, a wgpu limit, a panic) is recorded
      with its message, not worked around.

## Decisions

- **Data**: `(f64, f64)` rows: x the row index scaled to `0..1000`, y a seeded
  random walk (a 64-bit LCG, no new dependency); `--shuffle` permutes rows
  (default sorted by x, like `zoom_bench --sorted`, so chunk culling can act).
- **Plot**: `Plot::new()`, `Linear` x and y, one `circles` layer with `.x` and
  `.y` closures and a constant 1.5 px radius, `.retain(..)` from the option;
  1920 × 1080 (`--size WxH`).
- **Phases** (each timed with `Instant`, peak RSS read from `VmHWM` in
  `/proc/self/status` after it; on non-Linux print `n/a`):
  1. `build`: generating the rows (reported, not a cost of Gup).
  2. `accessor`: the bench itself runs the two closures over all rows into two
     `Vec<f64>` (the per-row closure cost the layer pays inside `evaluate`).
  3. `store`: `ColumnStore::from_columns` with two `F32Relative` columns and
     `chunk_rows_for` the context's limits on those vectors, then `upload`; the
     vectors are dropped after.
  4. `first_resolve`: `plot.resolve(&cx, w, h)` on the plot (which repeats 2–3
     internally plus fitting and layout).
  5. `first_frame`: `ImageTarget::render_blocking` of the scene (GPU time plus a
     readback; reported as is).
  6. `zoom`: `--zoom-frames N` (default 30) of `plot.zoom(centre, 0.97)` then
     `resolve`; report the median and p95 resolve time (CPU only; GPU pass time
     at scale is `zoom_bench --points N --sorted`'s job, run it once at 10M and
     record its `zoom_culled` lines too).
- **Metric names**: `scale.build.ms`, `scale.accessor.ms`,
  `scale.accessor.ns_per_row`, `scale.store.ms`, `scale.first_resolve.ms`,
  `scale.first_frame.ms`, `scale.zoom.resolve.median_ms`,
  `scale.zoom.resolve.p95_ms`, `scale.rss.<phase>.mb`, `scale.columns.bytes`,
  `scale.chunks`. Not added to `PERF_BUDGETS.md` by the worker; the orchestrator
  decides what becomes a budget.

## Escalation Triggers

- A 30M-row run with `--retain gpu-only` exceeds 12 GB peak RSS (the machine's
  limits, not Gup's design, would then bound the 100M run).
- `ColumnStore::from_columns` or `upload` is not reachable as public API from an
  example (then time phase 3 from inside `first_resolve` by another route and
  say how).

## Out of Scope

- Any library change (columnar input is GUP-628).
- Budgets in `PERF_BUDGETS.md` (the orchestrator's).

## Technical Tasks

- [ ] Write the example; check it at 100K.
- [ ] Release runs; the table and the paragraph.

## Dependencies

### Prerequisite Stories

- GUP-457 (chunk culling) ✅, GUP-482 (`Retain`, windows) ✅.

### Enables Stories

- GUP-628 (columnar input), GUP-631 (LOD spike), GUP-632 (100M demonstration).

## Testing Strategy

- **Manual runs**: AC2. The example compiles under `cargo check --examples`.

## Success Metrics

- [ ] N5's next stories cite measured phase costs at 10M and above.

## Risk Assessment

- **Medium**: a 100M run can exhaust memory and disturb other work on the
  machine. _Mitigation_: the `MemAvailable` gate, and run it when no other
  build-heavy agent runs.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, a benchmark
- [ ] `mask wasm-browser`: not needed, an example only
- [ ] WASM size: unchanged: no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
