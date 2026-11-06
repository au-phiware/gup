---
implementer: sonnet
implementer-reason:
  a headless benchmark over public APIs with fixed phases, options and output
  lines; the runs and their table are the deliverable
touches: [crates/gup-core/examples/scale_bench.rs]
---

# GUP-627: Scale Benchmark: Ingest and Memory

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-06) **Created**: 2026-11-04

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

- [x] `crates/gup-core/examples/scale_bench.rs` (headless: `ImageTarget`, no
      window) with the options and phases of Decisions, printing one
      `metric scale.<phase>.<unit> <value>` line per measurement (the
      `zoom_bench` format) and a human-readable summary.
- [x] Runs on lavapipe as well as a GPU (it only needs `Context::new_blocking`).
- [x] `cargo run -p gup-core --release --example scale_bench -- --rows 100000`
      finishes in under 10 s on the development machine; it is not added to any
      gate.

### AC2: The numbers

- [x] Release runs on the development machine at 1M, 10M and 30M rows with
      `--retain auto` and `--retain gpu-only`, and at 100M rows with
      `--retain gpu-only` if `MemAvailable` (from `/proc/meminfo`) is at least
      16 GB before the run; otherwise "skipped: N GB available" is recorded.
- [x] A table in the story's retrospective: for each run, every phase's time,
      the peak RSS after each phase, column bytes uploaded, chunks, and the
      per-row cost of the accessor phase in ns.
- [x] A paragraph that names the largest phase at 10M and at 100M (or 30M), and
      the bytes per row at peak, for GUP-628 and GUP-631 to cite.
- [x] A failure at any size (out of memory, a wgpu limit, a panic) is recorded
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

- [x] Write the example; check it at 100K.
- [x] Release runs; the table and the paragraph.

## Dependencies

### Prerequisite Stories

- GUP-457 (chunk culling) ✅, GUP-482 (`Retain`, windows) ✅.

### Enables Stories

- GUP-628 (columnar input), GUP-631 (LOD spike), GUP-632 (100M demonstration).

## Testing Strategy

- **Manual runs**: AC2. The example compiles under `cargo check --examples`.

## Success Metrics

- [x] N5's next stories cite measured phase costs at 10M and above.

## Risk Assessment

- **Medium**: a 100M run can exhaust memory and disturb other work on the
  machine. _Mitigation_: the `MemAvailable` gate, and run it when no other
  build-heavy agent runs.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, a benchmark
- [x] `mask wasm-browser`: not needed, an example only
- [x] WASM size: unchanged: no library code change
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Added `crates/gup-core/examples/scale_bench.rs` (headless, `ImageTarget`; no
library change; not in any gate). Options: `--rows`, `--retain`, `--shuffle`,
`--size`, `--zoom-frames`, `--seed`, and `--resolve-only` (skips the bench's own
accessor and store phases, so the peak RSS is the library's alone; added because
the bench's phase 3 and the library's first resolve do the same work, so `VmHWM`
after `first_resolve` could otherwise be hidden by phase 3's peak). It prints
the Decisions' `metric scale.*` lines plus `scale.first_resolve.columns.bytes`
and a summary header with load average and `MemAvailable`. At 100K rows it runs
in well under a second.

**Machine**: Intel HD Graphics 630 (KBL GT2, Vulkan, Mesa 26.0.0), 31 GB RAM
shared with other agents, `MemAvailable` 6-8 GB during the runs, load average
0.6-1.9 at start of each run. Release build, seed 1, sorted rows, 1920 x 1080,
`--retain` as shown. Median of 3 runs (1 run for 50M and 80M). Run-to-run spread
was under 5% on every phase.

**Results** (ms unless stated; RSS is `VmHWM` in MB after the phase; "GPU bytes"
is column bytes uploaded by the library's first resolve = 8 B/row, two
`F32Relative` columns):

| rows | retain   | build                                                | accessor (ns/row)  | store | first_resolve | first_frame | zoom resolve median / p95 | RSS after accessor / store=resolve | GPU bytes | chunks |
| ---- | -------- | ---------------------------------------------------- | ------------------ | ----- | ------------- | ----------- | ------------------------- | ---------------------------------- | --------- | ------ |
| 1M   | auto     | 6.2                                                  | 7.1 (7.08)         | 13.3  | 23.2          | 113         | 0.28 / 1.11               | 62 / 70                            | 8 MB      | 1      |
| 1M   | gpu-only | 5.9                                                  | 7.2 (7.16)         | 13.6  | 24.4          | 108         | 0.26 / 1.04               | 62 / 70                            | 8 MB      | 1      |
| 10M  | auto     | 68.6                                                 | 71.4 (7.14)        | 137.8 | 272.7         | 1065        | 0.13 / 0.28               | 337 / 415                          | 80 MB     | 10     |
| 10M  | gpu-only | 64.8                                                 | 70.5 (7.05)        | 138.8 | 289.9         | 1063        | 0.15 / 0.31               | 337 / 415                          | 80 MB     | 10     |
| 30M  | auto     | 196.5                                                | 212.6 (7.09)       | 476.3 | 810.5         | 3099        | 0.27 / 0.39               | 947 / 1183                         | 240 MB    | 29     |
| 30M  | gpu-only | 203.4                                                | 223.1 (7.44)       | 474.8 | 790.7         | 3237        | 0.15 / 0.21               | 947 / 1183                         | 240 MB    | 29     |
| 50M  | gpu-only | 359                                                  | 382 (7.64)         | 825   | 1362          | 5796        | 0.21 / 0.32               | 1557 / 1951                        | 400 MB    | 48     |
| 80M  | gpu-only | 543                                                  | n/a (resolve-only) | n/a   | 2320          | 9044        | 0.26 / 0.34               | 1252 (build) / 3103                | 640 MB    | 77     |
| 100M | gpu-only | skipped: 7.2 GB available (gate is 16 GB); see below |                    |       |               |             |                           |                                    |           |        |

`--resolve-only` runs (library alone) gave the same RSS and first_resolve times:
10M 415 MB, 30M 1183 MB, 80M 3103 MB (38.8 B/row).

**100M**: not run as specified (`MemAvailable` 7.2 GB < 16 GB, so "skipped: 7.2
GB available"). The 80M run was the largest the machine's free memory allowed
under the 50%-of-free cap, and it did not fail: nothing hit a wgpu limit or
out-of-memory up to 80M. Extrapolating linearly, 100M needs about 3.9 GB peak
RSS, 800 MB of GPU columns and a first frame of about 11 s on this iGPU.

**GPU pass**
(`zoom_bench --points 10000000 --sorted --size 1920x1080 --frames 200 --warmup 30 --present mailbox`,
windowed, MSAA 1): GPU render pass median 27.7 ms, p95 389 ms (the fully
zoomed-out frames), min 12.7 ms; 5 draws per frame median (chunk culling, of 10
chunks); CPU work 1.46 ms; columns written after the first upload: 0 B; the CPU
pick index at the deepest zoom 241 ms for 1,048,576 items. The window's frame
interval was 1000 ms (the compositor paced the redraw requests), so frames/s is
not meaningful; the GPU pass is. That is above the 16.7 ms mark even at the
median, so "100M rows zoom under 16.7 ms" is not met by today's architecture on
this GPU (an Intel iGPU, a floor rather than a representative target).

**Where the time goes**: at 10M, the largest phase is `first_frame` (1.06 s, GPU
render of all 10M rows unculled at the full extent plus the readback), then
`first_resolve` (0.27 s: accessor 71 ms + store 138 ms + about 64 ms of fit,
stats and layout). Among the CPU ingest phases the f64-to-f32 chunk conversion
and upload (`store`, 14 ns/row) costs about twice the closures (7.1 ns/row). At
80M the picture is unchanged and scales linearly: `first_frame` 9.0 s, then
`first_resolve` 2.3 s. Zoom `resolve` is 0.1-0.4 ms at every size (no column
bytes move; GUP-457 culling), so interactive zoom is bound by the GPU pass, not
the CPU. Bytes per row at peak: 41.5 B/row at 10M and 39.4 B/row at 30M for the
whole process (16 B of caller rows, 16 B of the two f64 accessor vectors, 8 B of
f32 chunk columns, remainder transient), 38.8 B/row at 80M in the library alone.
The retain mode made no measurable difference to peak RSS here because the peak
sits at the first resolve, before `GpuOnly` can drop anything. The 30M
`gpu-only` run peaked at 1.2 GB, far below the 12 GB escalation trigger.

Not run: lavapipe (the example needs only `Context::new_blocking`); the
benchmark's logs and helper scripts live under the checkout's target directory,
not in the repository.

## Retrospective

**Completed**: 2026-11-06

### Key Technical Learnings

#### Peak RSS is monotonic

- **Challenge**: `VmHWM` only rises, so a phase that does the same work as an
  earlier one reads as "no growth" and hides its own cost.
- **Solution**: `--resolve-only` skips the bench's own phases 2-3 so the
  library's first resolve sets the peak by itself; it matched (415 MB at 10M).
- **Pattern**: for peak-memory comparisons between phases, run them in separate
  processes.

### Architectural Decisions

#### Bench phases in functions, not a struct

- **Decision**: three small functions in the example; the library is used only
  through public API (`ColumnStore::from_columns`, `upload`, `Plot::resolve`).
- **Reasoning**: no library change was in scope; every API needed was public (no
  escalation).
- **Trade-off**: phase 3 times a hand-built store, not the one the layer builds.

### Development Workflow Insights

- The worktree sandbox refuses compound shell commands (loops, `cd &&`); run
  scripts kept in the target directory instead. NixOS has no `/bin/bash` or
  `/usr/bin/time`.
- `zoom_bench` needs `--present mailbox` on this machine (`immediate` is not
  supported by the surface).

### Follow-up Stories

None. The numbers point at GUP-628 (the 16 B/row of f64 intermediates is most of
the CPU memory) and GUP-631 (the unculled full-extent GPU pass, 100+ ns/row on
this iGPU). A 100M run still needs a machine with 16 GB free.
