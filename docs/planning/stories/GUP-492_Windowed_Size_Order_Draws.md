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

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-11) **Created**: 2026-10-23 **Revised**: 2026-11-07

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

- GUP-721: LOD for Size-Ordered Layers 📋 — reads this story's bucket keys.

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

## Blocked (2026-11-11), resolved

The orchestrator chose option 3: bucketing lands here; AC3's CPU gate (and the
resolve cost) moved to [GUP-724](GUP-724_Size_Ordered_Resolve_Cost.md).

**Question**: AC3's decision rule fired (`zoom_window.cpu.median_ms` is 12.0 ms
with `BUCKET_BITS = 3` and 12.1 ms with `BUCKET_BITS = 2`, limit 4.0), but the
cost is not the draw order's. How should the 4.0 ms limit be met or reframed?

**Done and committed** (commit "GUP-492: quantised size keys..." and the
`zoom_bench` flags): `DrawOrder::new(column, reversed, bucketed)`,
`BUCKET_BITS = 3`, the 5% / 30% unit test, the AC1 / AC2 tests in
`tests/window.rs` (`Retain::Auto` bounded, `Retain::GpuOnly` ≤ 16 a run, images
equal a fresh plot: 0 of 120,000 pixels differ),
`zoom_bench --bubbles --window N`.

**Evidence** (release, 1000x700, mailbox, machine load ~3-5):

| run                                                  | cpu median | draws | gpu pass |
| ---------------------------------------------------- | ---------- | ----- | -------- |
| `--points 300000` (plain scatter, one chunk)         | 0.86 ms    | 4     | 4.3 ms   |
| `--bubbles --points 100000` (one chunk)              | 0.62 ms    | 4     | 1.5 ms   |
| `--bubbles --window 1000000 --points 100000` (1 chk) | 0.55 ms    | 115   | 1.5 ms   |
| `--chunk-rows 25000 --points 100000` (plain)         | 1.08 ms    | 7     | -        |
| `--bubbles --chunk-rows 25000 --points 100000`       | 12.5 ms    | 7     | 2.0 ms   |
| `--bubbles --window 100000 --points 100000`          | 11.6 ms    | 19    | 1.0 ms   |
| `--bubbles --window 100000 --points 300000`, before  | 12.6 ms    | 19    | 0.95 ms  |
| same, after (BUCKET_BITS 3)                          | 12.0 ms    | 19    | 0.96 ms  |
| same, after (BUCKET_BITS 2)                          | 12.1 ms    | 19    | 0.96 ms  |

So a size-ordered layer with more than one chunk costs about 12 ms in
`Plot::resolve` every frame, windowed or not, bucketed or not (resolve median 25
ms on a loaded run, prepare 0.08 ms). It is not the number of draws. The
suspects are the per-frame O(rows) work for a size order in `selection.rs`
(about lines 2765-2910: `rank` over every stored row, `row_of` binary searches
per row), run for each zoom frame.

**Draws, 16 levels**: the exact order already draws at most 178 times in AC1's
run (a repeated hash of 16 distinct sizes merges into 16 ranges a run), so the
story's "more than 50,000 before" does not hold for 16 levels; with continuous
sizes (about 70 buckets) the same run draws 124,002 before and 702 after
(`a_windowed_continuous_size_order_draws_a_few_per_chunk`).

**Options**: (1) fix the per-frame cost first in a new story (find which
function: needs a profile), then finish this one; (2) accept 12 ms and set the
row's limit from the measurement (a budget decision, the orchestrator's); (3)
split: land the bucketing now (done), and move AC3's CPU gate to the story that
removes the per-frame cost. **Recommendation**: 3, with a story for the resolve
cost (it also hits GUP-721).

Also fixed, outside the letter of the story: `Selection::window` changed after
the first resolve left `bucketed` stale only when the chunk size was unchanged;
`evaluate` now treats a mismatch as stale (the trigger's check passed otherwise:
a window change normally changes the chunk cap and re-evaluates).

## Implementation Summary

- `DrawOrder::new(column, reversed, bucketed)`, `BUCKET_BITS = 3`, quantised
  `key()`, module doc; `Selection::size_order` passes `window != All`;
  `Selection::window` doc states the fidelity rule; `evaluate` rebuilds when
  `bucketed` no longer matches the window.
- Tests: `bucketed_keys_group_sizes_within_an_eighth` (`order.rs`);
  `a_windowed_size_order_draws_in_bounded_calls`,
  `a_windowed_continuous_size_order_draws_a_few_per_chunk` and
  `a_gpu_only_windowed_size_order_draws_at_most_16_a_run` (`tests/window.rs`).
  The GUP-482 window tests and the existing `DrawOrder` tests pass unchanged.
- `zoom_bench --bubbles --window N`; RFC-001 "GUP-492 findings".

### Acceptance criteria

- AC1: met with a guide-draw allowance of 16 (the story's bound fails by 2 at
  17,000 rows because the count includes axes and text): at most 178 draws under
  `Auto`, 1,986 under `GpuOnly` (bound 16 a run). **The "more than 50,000
  before" premise holds only for continuous sizes**: 16 levels draw 178 before
  and after; continuous sizes draw 124,002 before and 702 after. Unwindowed
  layers unchanged.
- AC2: met. Images equal a fresh plot (0 of 120,000 pixels differ; the layer is
  coloured by size so the order shows: smallest on top, larger at the rim);
  column bytes tests unchanged; the 5% / 30% unit test passes.
- AC3: flags added; draws 19 and GPU pass 0.96 ms measured; the CPU gate (12.0
  ms, 12.1 ms with `BUCKET_BITS = 2`) is not the draw order's cost and moved to
  GUP-724.
- AC4: RFC section and docs written.

### Proposed `PERF_BUDGETS.md` rows (draws and GPU pass only)

`--bubbles --window 100000 --points 300000 --metric-prefix zoom_window`:
`zoom_window.draws.median` 19 (limit 24); `zoom_window.gpu_pass.median_ms` 0.96
(machine-dependent, loaded; record on the budget machine). No CPU row until
GUP-724.

### Evidence

- WASM `wasm.scatter.gz_bytes`: base 379,250, branch 379,339 (+89 B).

## Retrospective

**Completed**: 2026-11-11

### Key Technical Learnings

#### The premise was measured only for continuous sizes

- **Challenge**: The story said the 16-level run draws over 50,000 before.
- **Solution**: Measuring first showed 178 (exact order merges equal sizes into
  16 ranges a run); the 124,002 to 702 change shows only with continuous sizes.
  An extra test covers it.
- **Pattern**: Measure the "before" with the real data shape before writing the
  bound; the bound can then be asserted on the shape that is actually bad.

#### Buckets are binade slices, not neighbourhoods

- **Challenge**: "Sizes 5% apart share a bucket" is false for 100 and 105.
- **Solution**: Bucket edges are fixed (top three mantissa bits). The unit test
  uses 64 and 67.2.
- **Pattern**: State fidelity as "at most 12.5% within a bucket", not "within
  12.5% of each other".

#### A flat fill hides the order

- **Challenge**: The first image test used one fill colour and was a solid red
  square, so equality proved nothing.
- **Solution**: Colour by size; then the interior is the smallest (dark) discs
  and the larger show at the rim, and a reversed order would be visibly wrong.

### Architectural Decisions

#### The CPU gate left this story

- **Decision**: `BUCKET_BITS` stays 3; the 4.0 ms gate moved to GUP-724.
- **Reasoning**: 12 ms a frame is `Plot::resolve` on any multi-chunk
  size-ordered layer (unwindowed too); `BUCKET_BITS = 2` changed nothing.
- **Trade-off**: AC3's CPU row is unrecorded until GUP-724.

### For GUP-721

- Reuse `DrawOrder::key` for the tier-block bucket; note that the windowed
  buckets are the only ones: an unwindowed order is exact (`bucketed = false`).
- Chunk-boundary runs: the floor (`mark_evaluated`) records the segments of the
  last full evaluation, before eviction.
- Expect GUP-724's resolve cost on any size-ordered frame.

### Development Workflow Insights

- Worktree agents cannot run compound shell commands (heredocs, `sed` with
  escapes, `&&` with git); use the Edit tool and plain commands.
- `zoom_bench` needs `--present mailbox` here (Immediate is unsupported).
- Load was 3 to 5 during the benchmarks; the CPU medians vary by about 10%.

### Follow-up Stories

1. **GUP-724: Size-Ordered Resolve Cost** (written; carries the CPU gate).
