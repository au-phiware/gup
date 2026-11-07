---
implementer: sonnet
implementer-reason:
  the design is settled in Decisions (quantised sort keys for windowed layers, a
  small change inside `DrawOrder`); the tests and the budget row follow
  `tests/window.rs` and `zoom_bench`
touches:
  [
    crates/gup-core/src/order.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/tests/window.rs,
    crates/gup-core/examples/zoom_bench.rs,
    docs/planning/rfcs/RFC-001_Core_Architecture.md,
  ]
---

# GUP-492: Bounded Draws for Windowed Size-Ordered Layers

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Ready **Created**:
2026-10-23 **Revised**: 2026-11-07

## Context

GUP-482 (RFC-001 "S12 findings", lines 3671–3745) sorts a windowed size-ordered
layer's batches in runs that end at chunk boundaries (`DrawOrder::push_runs`,
`crates/gup-core/src/order.rs`), so whole chunks leave in data order and
eviction writes no column bytes. The draw order is exact (largest first), but
its segments, each a range of consecutive stored rows drawn as one instanced
draw, cannot merge across runs sorted apart. With sizes that do not follow data
order (a random size column) a windowed layer draws about one draw per row: a
100,000-row window issues about 100,000 draws a frame. Compaction cannot help as
things stand: `needs_compaction` requires the segments to more than double the
count at the last full evaluation (`floor`), and an unordered windowed layer
starts near one segment per row. Unwindowed layers are unaffected: compaction
re-sorts them into one run.

The original plan listed three designs: an index buffer in draw order, a GPU
sort and size buckets. Reading today's code settles the choice. Every column is
an instance-step vertex buffer bound per chunk (`render.rs`, the
`VertexStepMode::Instance` layouts; `LayerGpu::record` binds buffers when the
chunk changes). An index buffer indexes vertices, not instances, so ordering
instances through one needs vertex pulling from storage buffers with every chunk
bound at once. That is alternative 3 of "GUP-431 findings" (lines 3139–3162),
rejected there for a second glue path and for devices with no vertex-stage
storage buffers, and a GPU sort (alternative 4) needs the same plus a compute
pipeline. Both would also grow the WASM scatter, which the `+2%` rule holds
(`PERF_BUDGETS.md`, `wasm.scatter.gz_bytes`). Size buckets (alternative 6) need
none of that: they only change the sort key.

## Read First

- RFC-001 "GUP-431 findings", "Mechanism" and "Alternatives rejected" (lines
  3064–3170), and "S12 findings" (lines 3671–3745: "Windowed size orders sort
  per chunk", "Compaction", "What the decisions got wrong").
- `crates/gup-core/src/order.rs`: `DrawOrder` (all of it, lines 1–260: `key`,
  `push_runs`, `merge`, `needs_compaction`, `mark_evaluated`, `spans`) and its
  tests (`size_order_case`, `fragmented_layers_compact_into_one_run`).
- `crates/gup-core/src/selection.rs`: `size_order` (line ~853), `runs` (line
  ~1180), the compaction check (line ~471) and the evaluation that calls
  `mark_evaluated` (lines ~820–840).
- `crates/gup-core/tests/window.rs`: `window_run`, `Run`, `Kind::Sized`, `row`
  (lines 1–265).
- `crates/gup-core/examples/zoom_bench.rs`: the options (lines 8–60), parsing
  (lines 108–160) and the `draws.median` metric (line ~532).
- `crates/gup-core/PERF_BUDGETS.md`, the `zoom_culled.*` rows (lines 74–78).

## User Story

> "As a developer of a live bubble chart, I want a windowed layer sized by a
> column to draw in a bounded number of draws, so that a large window stays
> interactive."

## Acceptance Criteria

### AC1: Bounded draws

- [ ] A windowed (`Window::LastRows(100_000)`, `Retain::Auto`) `Circle` layer
      sized by a column of 16 levels (`1.5^i`, `i` in 0..16, drawn at random per
      row, seeded), fed 1,000 rows at a time to 300,000 rows, has
      `Prepared::draw_calls() ≤ max(64 + held_rows / 1024, 2 × chunks × 16)`
      after every append (the test asserts it after each one; for these numbers
      the bound is at most 162). Before this story the same run draws more than
      50,000.
- [ ] The same run under `Retain::GpuOnly` (no compaction possible) is bounded
      by `16 × runs_held`, where `runs_held` is the number of appended runs
      still stored; the test asserts it and prints the count.
- [ ] An unwindowed layer's `draw_calls()` and stored order are unchanged: the
      existing `DrawOrder` unit tests pass without edits other than the new
      constructor argument.

### AC2: Image equality and no column bytes

- [ ] For the AC1 run (400 × 300, levels as above, marks overlapping so order
      shows), the image equals a fresh unwindowed plot of the held rows, within
      `DiffTolerance::default()` as `window_run` compares (every level is in its
      own bucket, so the bucketed order is the exact order).
- [ ] Eviction writes no column bytes:
      `written.columns.bytes == uploaded_rows × stride` as `window_run` asserts
      for `Kind::Sized` today; the AC2–AC4 tests of GUP-482 in `tests/window.rs`
      pass unchanged.
- [ ] Mixed sizes within a bucket draw in data order: a `DrawOrder` unit test
      with two sizes 5% apart asserts they are one bucket (equal keys, stored
      rows first) and sizes 30% apart are two buckets.

### AC3: Frame cost and budget row

- [ ] `zoom_bench` gains `--bubbles` (a size column of the 16 levels, drawn
      randomly by a fixed seed) and `--window N` (`Window::LastRows(N)`, rows
      fed in 1,000-row appends before the measured frames). Run as
      `--bubbles --window 100000 --points 300000 --metric-prefix zoom_window` in
      release, it prints `zoom_window.draws.median`,
      `zoom_window.gpu_pass.median_ms` and `zoom_window.cpu.median_ms`. The
      story proposes these as rows for `PERF_BUDGETS.md` in its retrospective
      (the orchestrator records them) and states the same layer's numbers before
      the change, measured first.
- [ ] Decision rule: if `zoom_window.cpu.median_ms` exceeds 4.0, set
      `BUCKET_BITS` to 2 and measure again; if it still exceeds 4.0, stop (see
      Escalation Triggers).

### AC4: Documentation

- [ ] RFC-001 gets a "GUP-492 findings" section: the chosen design, the
      alternatives rejected (below, with the code reasons), the measured draws
      and frame numbers of AC3, the fidelity rule (sizes within one bucket draw
      in data order), and the `Retain::GpuOnly` limit.
- [ ] The `DrawOrder` module doc and `Selection::window` docs say that a
      windowed size order is exact between buckets of 12.5% relative size and in
      data order within one.

## Decisions

- **Chosen design: quantised sort keys for windowed layers.** `DrawOrder` takes
  a `bucketed: bool`; when true, `key()` clears the low `23 - BUCKET_BITS`
  mantissa bits of the `f32` key (`BUCKET_BITS: u32 = 3`, a private const next
  to `COMPACT_SEGMENTS`):
  `f32::from_bits(k.to_bits() & !((1 << (23 - BUCKET_BITS)) - 1))`. This is
  monotone for positive and negative keys, leaves `-∞` (null) and zero alone,
  and gives equal keys to sizes within 12.5%. Rows of equal key sort by data
  index (the packed-key sort already does) and merge stored-first (`merge`'s
  `e >= k`), so a bucket draws in data order. Nothing else in `order.rs`
  changes.
- **Why it bounds draws**: a chunk's run holds at most one range per bucket, and
  runs of different buckets no longer interleave, so a full evaluation of a
  windowed layer gives at most `chunks × buckets_used` segments, which is what
  `floor` records. Compaction (already gated by `all_rows_held()` and the
  `2 × floor` rule) then holds draws to the bound of AC1. Under
  `Retain::GpuOnly` rows cannot be evaluated again, so draws grow with runs;
  that limit is documented, not fixed here.
- **API and call sites**:
  `DrawOrder::new(column: usize, reversed: bool, bucketed: bool)`;
  `Selection::size_order` passes `self.window != Window::All`. No public API
  changes.
- **Unwindowed layers keep the exact order**: `bucketed` is false, so their
  segments, spans, picking and tests are byte-for-byte as before.
- **Rejected unless measured** (record in the RFC):
  - **Index buffer in draw order**: instances come from instance-step vertex
    buffers; ordering them needs vertex pulling with every chunk bound at once
    (GUP-431 alternative 3: second glue path, hand-written format conversion, no
    devices without vertex-stage storage) and `O(rows)` index bytes rewritten at
    every eviction. Reconsider only if a story needs an exact order over
    arbitrary sizes in a window.
  - **GPU sort**: the same limits plus a compute sort pipeline; reconsider if
    vertex pulling arrives for another reason (GUP-431 alternative 4).
  - **Exact merge across chunks**: impossible with per-chunk buffers and random
    sizes in fewer draws than rows.
- **Exactness is relaxed for windowed layers only**, by at most 12.5% in size
  within a bucket; the cost is accepted because the exact order costs a draw per
  row.
- **Benchmark flags** live in `zoom_bench` (`--bubbles`, `--window N`); no new
  example.

## Escalation Triggers

- `Selection::window` can change the window after the order was built, so
  `bucketed` would be stale (check that setting a window re-evaluates or builds
  the order after it; if not, stop).
- GUP-628's columnar layers or GUP-631's level of detail changed `runs`,
  `size_order`, spans or compaction, so the plan's reading of them is wrong.
- AC1's bound fails by more than a few segments, which would mean `floor` or
  compaction behaves otherwise than described.
- `zoom_window.cpu.median_ms` exceeds 4.0 with `BUCKET_BITS = 2`.
- An existing windowed test draws marks whose sizes differ by less than 25% and
  now fails on order (do not change its data without reporting it).

## Out of Scope

- Vertex pulling, an index buffer or a GPU sort (rejected above).
- Bounding draws under `Retain::GpuOnly` (documented limit).
- Level of detail for windows (GUP-631 and its follow-ups).
- Recording `PERF_BUDGETS.md` (the orchestrator does).
- Size order over a polyline mark (it never sorts).

## Technical Tasks

- [ ] Measure the pre-change numbers first: run the new `zoom_bench` flags (or,
      before they exist, the AC1 test) on the unmodified order and record draws,
      GPU pass and CPU median.
- [ ] Add `bucketed` and `BUCKET_BITS` to `DrawOrder` (`new`, `key`, docs);
      update `size_order` and the `DrawOrder` unit tests' constructor calls.
- [ ] Add the 5% / 30% bucket unit test (AC2) and the AC1 draw-count tests
      (`Retain::Auto` and `GpuOnly`) with the image comparison in
      `tests/window.rs`.
- [ ] Add `--bubbles` and `--window N` to `zoom_bench`; run it (AC3) and apply
      the decision rule.
- [ ] Write the RFC findings and the doc comments (AC4).
- [ ] Search `tests/`, `examples/` and `crates/gup-core/wasm-size/` for windowed
      layers sized by a column; confirm each still passes or opt it out.

## Dependencies

### Prerequisite Stories

- GUP-482: RFC-001 S12c `Window::LastRows` eviction ✅.
- GUP-628: Columnar Layers 📋 — sequencing only: it edits `selection.rs`
  (`runs`, evaluation); start this story after it merges.
- GUP-631: Level of Detail Spike 📋 — sequencing only: it edits `selection.rs`
  and `render.rs` and its findings say how LOD meets `Window::LastRows`; start
  this story after it merges and read that section.

### Enables Stories

- None scheduled.

## Testing Strategy

- **Unit tests**: bucket boundaries; ties in a bucket draw in data order;
  existing `DrawOrder` tests unchanged.
- **Integration tests**: `tests/window.rs` — draw counts after every append
  under both `Retain` policies, image equality with a fresh plot, column bytes.
- **Visual validation**: read the saved PNG of the AC2 run beside the fresh
  plot's; circles of every level must be visible and the larger drawn beneath.
- **Performance**: `zoom_bench` as AC3, before and after.

## Success Metrics

- [ ] Draws for the 100,000-row window drop from over 50,000 to at most 162
      under `Retain::Auto`.
- [ ] `zoom_window.cpu.median_ms` ≤ 4.0.
- [ ] `wasm.scatter.gz_bytes` within +2% (expected: under 300 B).

## Risk Assessment

- **Medium**: sizes spread over many binades give many buckets (8 per binade),
  so `chunks × buckets_used` grows; a size range of 1,000× is about 80 buckets
  and 400 draws over five chunks. _Mitigation_: `BUCKET_BITS = 2` halves it
  (AC3's rule).
- **Low**: windowed charts lose exact order within 12.5% of size. _Mitigation_:
  documented; overlaps between marks that close in size hide the same area.
- **Low**: compaction re-uploads the window when segments double. This is
  GUP-482's existing amortisation; the test notes the column bytes it writes
  under `Retain::Auto` apart from AC2's pure-append run.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the AC2 PNG read)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] `mask perf-budget` table pasted and proposed rows listed (the orchestrator
      records them)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
