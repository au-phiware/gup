---
implementer: opus
implementer-reason:
  a new core abstraction (the per-chunk tier structure every later LOD story
  reads) and a radix sort that must meet a measured build bound
touches: [crates/gup-core/src/lod.rs, crates/gup-core/src/lib.rs]
---

# GUP-714: LOD Tiers in the Column Store

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-11

## Context

GUP-631 chose a decimation pyramid for scatters of tens of millions of rows
(RFC-001 "GUP-631 findings"): per chunk, one Morton sort of the rows, from which
every tier is a prefix of one list of representatives (a row represents its cell
at the tier `16 - floor(h / 2)`, `h` the highest bit in which its key differs
from its predecessor's). The spike built it in
`crates/gup-core/examples/lod_spike.rs` (`sorted_keys`, `PyramidChunk::build`,
lines ~385-470) over one global grid spanning the layer's extent, with a
comparison sort, in 562-826 ms per 10M rows.

This story ships the structure in the library, CPU only and not yet drawn:
`crates/gup-core/src/lod.rs`. Two things differ from the prototype. The grid is
anchored in data units, per chunk, so a layer whose extent grows (appends) keeps
its built chunks valid. The sort is a radix sort. Nothing renders from it until
GUP-715 and GUP-716.

## Read First

- RFC-001 "GUP-631 findings" (line 4319 on): "The prototypes" (B), "The
  decision", "Interaction under LOD" ("Appends").
- `crates/gup-core/examples/lod_spike.rs`: `part1by1`, `sorted_keys`,
  `PyramidChunk::build`, `View::tier` (lines 340-470).
- `crates/gup-core/src/column.rs`: `Chunk` (`row_base`, `rows`, `columns`,
  stats) and `ColumnStore::value` (GUP-664), for reading a row's x and y.
- `crates/gup-core/examples/scale_bench.rs` (the phases and how rows are fed).

## User Story

> "As a developer of a 100M-row scatter, I want each chunk to know which rows
> stand for which cells at every zoom level, built once and cheaply, so that a
> frame can draw tens of thousands of rows instead of millions."

## Acceptance Criteria

### AC1: Tiers nest, representatives are lowest-index

- [ ] `lod::Tiers::build(xs: impl Fn(u32) -> f64, ys: impl Fn(u32) -> f64, rows: u32) -> Tiers`
      (signature in Decisions) over a seeded random-walk chunk of 100,000 rows
      returns `counts` that are non-decreasing and a `order` of `counts[stored]`
      distinct chunk-local rows.
- [ ] For every tier `t <= stored`, the set `order[..counts[t]]` equals the
      brute-force set: the lowest-index row of each occupied cell of the tier's
      grid (checked by a `HashMap` over `(cx >> (16 - t), cy >> (16 - t))`).
      Tiers nest: `order[..counts[t]]` is a prefix of `order[..counts[t + 1]]`.
- [ ] `counts[0] == 1` for a non-empty chunk, and rows with a non-finite x or y
      are never representatives (they are skipped in the build).

### AC2: Storage rule

- [ ] Tiers are stored while `counts[t] <= rows / 2`; `stored` is the finest
      such tier and `order.len() == counts[stored]`. A chunk of 1,000 rows on a
      grid where every row has its own cell at tier 4 stores tiers 0-3 only
      (test). A chunk whose rows all coincide stores tier 0 only.

### AC3: The grid depends on the chunk alone

- [ ] The tier-16 grid has cells of `2^ex` by `2^ey` data units (integers `ex`,
      `ey`, possibly negative), anchored at multiples of those sizes: the key of
      a row is
      `part1by1(floor(x / 2^ex) - ox) | part1by1(floor(y / 2^ey) - oy) << 1`
      with `(ox, oy)` the chunk's own minimum cell. `ex`, `ey` are the smallest
      integers with `floor(max / 2^ex) - floor(min / 2^ex) < 65_536` over the
      chunk's finite rows.
- [ ] `build` over a chunk's rows gives the same `Tiers` however many rows the
      layer holds around it: a test builds chunk 0 of a 3-chunk layer and of a
      layer of chunk 0 alone and asserts equal `ex`, `ey`, `order`, `counts`
      (appends that grow the layer's extent leave built chunks valid).

### AC4: Build cost

- [ ] `scale_bench` gains a `lod_build` phase (a `--lod` flag) printing build ms
      per 10M rows for a sorted random walk (the spike's data) in release, one
      thread. On the development machine with load under 2 it prints at most 650
      (the spike's comparison sort: 562-826). Peak extra memory during a chunk's
      build is at most 16 B a row of that chunk (reported).
- [ ] `lod.build_ms_per_10m` is proposed as a `PERF_BUDGETS.md` row in the
      retrospective (the orchestrator records it).

## Decisions

- **Where**: new module `crates/gup-core/src/lod.rs`, `pub(crate)` items only
  (the public `Lod` enum arrives in GUP-716); `mod lod;` in `lib.rs`. No prelude
  change.
- **Type**:

  ```rust
  pub(crate) struct Tiers {
      /// Cell size at the finest tier, as powers of two: 2^ex by 2^ey data units.
      pub ex: i32,
      pub ey: i32,
      /// The finest stored tier (0..=16).
      pub stored: u32,
      /// Chunk-local rows in progressive order: tier t is `order[..counts[t]]`.
      pub order: Vec<u32>,
      /// `counts[t]` for t in 0..=stored.
      pub counts: Vec<u32>,
  }
  impl Tiers {
      pub fn build(rows: u32, xy: impl Fn(u32) -> (f64, f64)) -> Tiers;
      /// Representatives of tier `t`, or None past the stored tiers (draw all).
      pub fn prefix(&self, t: u32) -> Option<u32>;
  }
  ```

  One closure reading x and y together keeps the read to one pass.

- **Sort**: stable LSD radix sort of `(key: u32, row: u32)` pairs, three passes
  of 11 bits (32 key bits, 2,048 buckets). Stability keeps equal keys in row
  order, so a run's first element is its lowest row (no index in the key, unlike
  the prototype's `key << 20 | index`). Then the progressive order by a counting
  sort over `first_tier` (at most 17 values), stable, so each tier block is in
  row order (what the spike measured). GUP-720 reorders blocks; do not.
- **Tier meaning** (as the spike): tier `t` has `2^t` by `2^t` cells over the
  chunk's 65,536-cell span; `first_tier = 16 - floor(h / 2)` for the highest
  differing bit `h` of consecutive keys, `0` for the first key, and "never" for
  a repeated key (a later row of the finest cell).
- **Non-finite rows** are skipped, not clamped: a NaN position draws nothing
  today.
- **Time-boxed to the CPU**: the build is a plain function over closures; the
  wiring to `Chunk`/`ColumnStore` is GUP-715's.
- **Not in this story**: a GPU sort (the spike's note: next step only if the
  radix sort misses AC4).

## Escalation Triggers

- The radix build exceeds 650 ms per 10M rows with load under 2 (report the
  number and the profile; do not change the storage rule to compensate).
- A chunk's extent cannot fit 16 bits for any `(ex, ey)` (cannot happen for
  finite f64 within range; a chunk of subnormals or `1e300` data does: stop and
  report the case).
- `ColumnStore::value` or a chunk's shared columns cannot give x and y of one
  row in O(1) (the closure's assumption).

## Out of Scope

- Uploading representatives, the glue (GUP-715); the `Lod` setter and per-frame
  tier choice (GUP-716); appends and tail chunks (GUP-719); spatial runs
  (GUP-720); size order (GUP-721).

## Technical Tasks

- [ ] `lod.rs`: key computation (`ex`, `ey`, origin), radix sort, `first_tier`,
      counting sort, storage rule, `prefix`.
- [ ] Unit tests for AC1-AC3 (brute-force cells; growing-layer invariance).
- [ ] `scale_bench --lod`: the `lod_build` phase and its output line.
- [ ] Doc comments on every item; `mask all-fix`.

## Dependencies

### Prerequisite Stories

- GUP-631: Level of Detail Spike ✅ — the design and its measurements.
- GUP-664: Shared Columns as the CPU Copy ✅ — O(1) reads of a row.

### Enables Stories

- GUP-715, GUP-716 and through them GUP-717-721.

## Testing Strategy

- **Unit tests**: brute-force equivalence at 100,000 rows (random walk, uniform,
  clustered, all-equal, with NaNs); nesting; storage rule; chunk invariance.
- **Performance**: `scale_bench --lod --rows 10000000` in release, three runs,
  median.

## Success Metrics

- [ ] Build at most 650 ms per 10M rows (spike: 562-826).

## Risk Assessment

- **Medium**: a radix sort may not beat the comparison sort's 650 ms on one
  thread on this machine. _Mitigation_: AC4 is a threshold with an escalation,
  and the three-pass sort moves 8 B pairs, which should be several times cheaper
  than `sort_unstable` on `u64`.
- **Low**: the per-chunk grid (not the spike's global one) changes tier
  boundaries between chunks. _Mitigation_: tiers are chosen per chunk in GUP-716
  from its `ex`, `ey`; fidelity is checked there (GUP-717).

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output: not applicable (no drawing)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported (expected unchanged:
      the module is unused until GUP-716, so it is dropped)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
