---
implementer: opus
implementer-reason:
  eviction cuts across the column store, size draw order, line bridges, brush
  bits, CPU and GPU picks, culling and Retain; the API and invariants are fixed
  but the per-structure renumbering is design work with no precedent
---

# GUP-482: RFC-001 Step S12c: `Window::LastRows` Eviction

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-19

## Context

GUP-480 lets a chart grow without bound: an `Appender` pushes rows, each resolve
uploads only the new ones. A live chart that runs for hours needs the opposite
too: drop the oldest rows. RFC-001 §3 says "Windowing (`Window::LastRows(n)`)
evicts whole chunks", and example (e) writes
`.window(Window::LastRows(1_000_000))`.

Several later findings say what eviction must keep consistent:

- **S4a** (RFC lines 2190–2195): "Evicting whole chunks leaves `row_base` values
  that do not start at 0; picking must map rows through the window."
- **S4b** (line 2318): "Appends already work under every policy. Evicting a
  released chunk loses nothing on the CPU."
- **GUP-431** (line 3155): "`Window::LastRows` (S12) evicts whole chunks, so it
  must drop their rows from the `DrawOrder` segments too."
- **GUP-432** (line 3301): "Evicting a whole chunk drops its bridge with it. The
  remaining first chunk's segments are unaffected."
- **GUP-457** (S9f): culling reads per-chunk f64 stats; **GUP-458**: brush bits
  are indexed by stored row; **GUP-455**: GPU pick ids are
  `row_base + instance_index`.

The size draw order is the hard case. Each appended batch is sorted largest
first before it is stored, so within a batch, stored order is not data order. A
chunk boundary can split a batch: evicting the first chunk then drops that
batch's largest rows, not its oldest. The invariant in Decisions makes that
safe: a chunk goes only when every row in it is older than the newest `n`, so it
does not matter in which order rows beyond the window leave.

This story is `opus` because each structure above needs its own renumbering, and
getting one wrong shows up as a wrong pick, a mis-dimmed row or a mis-culled
chunk, not a crash.

## Read First

- RFC-001 lines 226–236 ("Append"), 2100–2115 and 2186–2195 (S4a), 2318 (S4b),
  3140–3160 (GUP-431 "Proposed adjustments"), 3285–3305 (GUP-432).
- [GUP-480](GUP-480_RFC_001_S12a_Appender_Handle.md) "Decisions".
- `crates/gup-core/src/column.rs`: `Chunk` (lines 495–545), `ColumnStore` (lines
  918–1110) and `append` (line 1010), `chunk_rows_for` (line 961).
- `crates/gup-core/src/order.rs` (whole: `DrawOrder`, `row_of`, `draw_entries`,
  compaction).
- `crates/gup-core/src/selection.rs`: the struct, `append`, `released`,
  `write_bits` (lines 500–540), `Retain` handling (`release`).
- `crates/gup-core/src/pick.rs` (CPU pick items, lazy grid),
  `crates/gup-core/src/gpu_pick.rs` lines 40–80 and 240–400 (generations),
  `crates/gup-core/src/cull.rs`.
- GUP-457, GUP-458 and GUP-455 "Decisions" (stories in this directory).

## User Story

> "As a developer of a long-running live chart, I want the chart to keep only
> the most recent rows, so that memory, upload and draw cost stay bounded
> however long it runs."

## Acceptance Criteria

### AC1: API

- [ ] `gup_core::Window` and `Selection::window`, `ScatterChart::window` and
      `LineChart::window` exist as in Decisions, with doctests that append past
      the window and assert `len()` stays bounded.

### AC2: The window invariant

For a scatter layer in 64-row chunks (`max_chunk_rows(64)`) with
`Window::LastRows(200)`, fed in batches of 1, 27, 36, 100, 300 and 5 rows to
2,000 rows in total, and the same for a size-ordered layer (`size` from a
column) and a `Line` layer:

- [ ] After every resolve, each of the newest 200 data rows is drawn, and the
      layer holds at most `200 + 2 × chunk_rows` rows.
- [ ] After the last batch, the image equals a fresh plot of exactly the rows
      the layer still holds (byte-identical), so draw order, line bridges and
      colours are what a fresh build gives.
- [ ] Column bytes written over the whole run equal the appended rows × stride:
      eviction writes no column bytes. Its other writes (chunk uniforms, brush
      bits) are counted and listed in the Implementation Summary.
- [ ] Auto domains follow the window: the x domain after the run covers the kept
      rows and not row 0.

### AC3: Everything indexed by row

On the size-ordered layer after evictions:

- [ ] `Plot::pick_row` and `pick::<T>` at each kept row's anchor return that
      row's `RowId` (data row index counted from the layer's first row ever) and
      `&T`; at an evicted row's old position, nothing.
- [ ] A GPU pick (`request_pick`/`poll_picks`) at each of 10 kept rows agrees
      with the CPU pick; a pick requested before an eviction and polled after it
      is dropped, not mapped to another row.
- [ ] A brush over kept rows dims exactly the others (GUP-458's check), before
      and after a further eviction.
- [ ] Culling (S9f) on and off give the same image.
- [ ] `Plot::row(id)` for an evicted id is `None`; events do not fire for it.
- [ ] Under `Retain::Rows`, `Retain::Columns` and `Retain::GpuOnly`, the run
      passes the AC2 checks that the policy can support (picks under `GpuOnly`
      via GPU picks only).

### AC4: Long runs

- [ ] A layer fed 10,000,000 rows in batches of 10,000 with `LastRows(100_000)`
      keeps its chunk count within `ceil(1.25 × 100_000 / chunk_rows) + 2`
      throughout, and no `row_base` written to the GPU exceeds the rows held
      (release build test, or an `#[ignore = "timing budget"]` test with the
      measured run time in the Implementation Summary).

### AC5: Dogfood and findings

- [ ] `t5_live`'s 100k scatter uses `.window(Window::LastRows(100_000))`, as a
      live chart would; its checks pass unchanged. (The run is too short to
      evict; AC2–AC4 prove eviction. `t5_stream`'s 150 rows are below any chunk,
      so it stays unwindowed.)
- [ ] RFC-001 gains "S12 findings (GUP-480–482)": the API, the invariant, how
      each structure renumbers, measured costs, and anything the decisions below
      got wrong.

## Decisions

- **API**:

  ```rust
  /// How many rows a selection keeps as rows are appended.
  #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
  pub enum Window {
      /// Keep every row (the default).
      #[default]
      All,
      /// Keep at least the newest `n` rows. Older rows go a whole chunk at a
      /// time, so between `n` and about `1.25 × n` rows are held.
      LastRows(usize),
  }
  impl Selection<T, M> { pub fn window(&mut self, window: Window) -> &mut Self; }
  // ScatterChart::window(self, Window) -> Self
  // LineChart::window(self, Window) -> Self: per line layer (per key).
  ```

- **Invariant**: a chunk may be evicted only from the front, and only when every
  stored row in it has a data row older than the newest `n`. Eviction is checked
  after each append (in `drain_feed`/`append`).
- **Chunk size**: a windowed layer caps rows per chunk at
  `max(1024, n.div_ceil(4))`, and the device cap below that; tests use the
  existing `max_chunk_rows` seam. Reason: whole-chunk eviction with device-sized
  chunks (millions of rows) would never evict a small window.
- **Row ids**: `RowId.row` stays the data row index counted from the layer's
  first row ever, so ids held by user code stay valid until their row is
  evicted. The selection keeps an `evicted: u64` offset beside `released`.
- **Stored rows renumber from 0** after each eviction: chunk `row_base`s are
  rewritten (uniform writes), `DrawOrder` drops the evicted stored rows from its
  keys, batches and segments and shifts the rest, brush bits are rewritten
  whole, and the CPU pick grid is rebuilt lazily. Reason: `row_base` is a `u32`
  on the GPU (S4a); without renumbering a long stream overflows it.
- **Dictionaries keep evicted keys**: codes are in uploaded columns, so a key's
  code never changes; a legend entry can outlive its rows. Follow-up if that
  proves confusing.
- **GPU picks across an eviction**: `poll_picks` drops results whose generation
  predates the most recent eviction of their layer.
- **Lines**: the bridge into the first kept chunk is dropped with the evicted
  chunk (GUP-432); a line therefore starts at the first kept row.

## Escalation Triggers

- The invariant cannot hold for size-ordered layers without re-sorting stored
  rows (any column re-upload on eviction).
- A structure needs data-row order where only stored order is kept (or the
  reverse), and the mapping would cost `O(rows)` per eviction.
- The chunk-size cap changes upload counts or images of any non-windowed test.
- `wasm.per_row_type.gz_bytes` fails.

## Out of Scope

- Time-based windows (`LastSeconds`), dictionary compaction, eviction for
  `BarChart`.
- The browser waker: GUP-481.

## Technical Tasks

- [ ] `Window`, setters, chunk cap.
- [ ] Store eviction and renumbering; `row_base` rewrites.
- [ ] `DrawOrder`, line bridges, brush bits, pick grid, GPU pick generations,
      culling stats, `Retain`.
- [ ] Tests AC2–AC4; dogfood `t5_live`; RFC findings.

## Dependencies

### Prerequisite Stories

- GUP-480: RFC-001 S12a Appender Handle 📋 — appends from a handle; `t5_live` on
  the appender.
- GUP-431, GUP-432, GUP-455, GUP-457, GUP-458 ✅ — the structures that renumber.

### Enables Stories

- None scheduled.

## Testing Strategy

- **Unit tests**: `DrawOrder` eviction (segments, `row_of`) and store eviction
  (`row_base`, stats) in their modules.
- **Integration tests**: AC2–AC4 as plot-level tests beside the append tests in
  `plot.rs` (or a new `crates/gup-core/tests/window.rs`).
- **Visual validation**: read the AC2 final images for the three layers.
- **Performance**: `mask perf-budget`; zoom rows unchanged.

## Success Metrics

- [ ] Memory and draws bounded on a 10M-row stream.
- [ ] No pick, brush or cull mismatch after evictions.

## Risk Assessment

- **High**: off-by-one renumbering in one structure. _Mitigation_: AC2's
  byte-identical comparison against a fresh build and AC3's per-structure
  checks.
- **Medium**: the chunk cap adds draw calls for large windows (`n / 4` rows a
  chunk, about 5 draws per layer). _Mitigation_: record the draw count in the
  findings.
- **Medium**: trybuild `.stderr` churn from edits near the top of
  `selection.rs`. _Mitigation_: re-bless with `TRYBUILD=overwrite` only for
  line-number diffs; escalate on message changes.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS)
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-browser` passes (this story changes code compiled to wasm and
      the pick path)
- [ ] `mask wasm-size` run and the scatter and per-row-type figures reported in
      the Implementation Summary (the orchestrator re-records the budget)
- [ ] `mask perf-budget` table pasted (story-worker step 9)
- [ ] Rendered output verified by eye: the AC2 final images, `t5_live.png`
- [ ] `mask old-path-loc` recorded (expected unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
