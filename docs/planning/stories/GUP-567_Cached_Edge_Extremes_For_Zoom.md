---
implementer: sonnet
implementer-reason:
  a measurement spike with a stop rule, then a fixed cache design with an exact
  equivalence test; no open design choices
touches: [crates/gup-core/src/selection.rs]
---

# GUP-567: Cached Edge Extremes for Zoom

## Story Overview

**Initiative**: Strategic Review 2026-11 (N5 Performance at Scale) **Status**:
✅ Complete (2026-11-01) **Created**: 2026-10-26

## Context

GUP-528 insets each end of an explicit axis by the overhang of the marks that
reach it, and re-solves whenever the application sets a domain. `zoom_bench`
calls `set_domain` every frame, so `zoom_culled.cpu.median_ms` (4,194,304
x-sorted rows in 64 chunks of 65,536) went from 0.69 to 1.24 ms
(`crates/gup-core/PERF_BUDGETS.md`, change log, "GUP-528 merged").

The solve is `Layer::edge_marks` (`selection.rs` lines 1346–1505). For each
chunk whose statistics say a mark may reach an edge, it decodes the chunk
(`ColumnStore::chunk_values`, a fresh `Vec<f64>` of 65,536 values) and, for
constant-size marks, scans it with `in_domain_extremes` (lines 2071–2113) for
the in-domain values nearest each end, while a shared budget of
`EDGE_ROW_BUDGET` = 65,536 rows lasts (`plot/inset.rs` line 14); later chunks
fall back to statistics. In the benchmark the y axis is log-scaled and every
x-sorted chunk spans all of y, so every frame decodes and scans one full chunk:
O(rows) work for an answer that a sorted copy gives in O(log rows).

This story measures that share first, then caches, per decoded chunk and column,
the finite values sorted, and answers the extremes by binary search, with
results identical to today's.

## Read First

- `crates/gup-core/src/selection.rs`: `edge_marks` (lines 1346–1505),
  `in_domain_extremes` (lines 2071–2113), `fn epoch` (line 1970).
- `crates/gup-core/src/plot/inset.rs` (all 129 lines: `EDGE_ROW_BUDGET`,
  `EdgeAxis`).
- `crates/gup-core/src/column.rs` `chunk_values` (line 1225).
- `crates/gup-core/examples/zoom_bench.rs`: the options block (lines 9–40), the
  `zoom` method (lines 229–243), and how `--sorted`/`--chunk-rows` build the
  plot (lines 615–630).
- `crates/gup-core/PERF_BUDGETS.md` lines 74–78 and the GUP-528 change-log entry
  (lines 495–505).

## User Story

> "As a developer zooming a multi-million-row chart from code, I want the
> per-frame edge solve to cost microseconds, so that programmatic zoom is as
> smooth as it was before data-aware insets."

## Acceptance Criteria

### AC1: Spike, where the time goes (stop rule)

- [x] A committed headless benchmark,
      `crates/gup-core/examples/edge_inset_bench.rs` (copyright header): the
      same rows as `zoom_bench --points 4194304 --chunk-rows 65536 --sorted`
      (through `tests/common/scatter.rs`, as `zoom_bench` does), resolved
      offscreen at 1920×1080 with `Context::new_blocking`, 60 warm-up then 600
      frames of `zoom_bench`'s `zoom(t)` path, timing only `Plot::resolve`. It
      prints `metric edge.resolve.median_ms` and `metric edge.resolve.p95_ms`.
- [x] At the parent commit, with an uncommitted `Instant` around the
      `edge_marks` calls in `plot.rs`, record the median time per frame spent in
      them. **Rule**: if it is below 0.3 ms, stop and report (the regression is
      elsewhere); otherwise continue.

### AC2: Sorted-values cache

- [x] The cache in Decisions is implemented; the constant-size branch of
      `edge_marks` uses it instead of `chunk_values` plus `in_domain_extremes`.
- [x] Property test in `selection.rs`: for 2,000 seeded cases (100–5,000 values
      with some NaN, a `Linear` or `Log` encoding, random domains inside,
      straddling and outside the data, forward and reversed ranges),
      `sorted_extremes` on the sorted finite values returns exactly what
      `in_domain_extremes` returns on the unsorted values (bit-equal `f32`).
- [x] Invalidation tests: appending rows to the last chunk, an eviction
      (`Window::LastRows`), and re-evaluating the columns each make the next
      solve match a fresh plot's insets.

### AC3: Results unchanged

- [x] Every GUP-528 and GUP-540 test passes unchanged, and every golden passes
      without re-blessing.

### AC4: Target

- [x] After the change, `edge_marks`' median time per frame (same temporary
      instrumentation) is at most 0.05 ms, and `edge.resolve.median_ms` drops by
      at least 80% of AC1's `edge_marks` time. Both numbers, before and after,
      go in the Implementation Summary.
- [x] If the environment has a display (`WAYLAND_DISPLAY` or `DISPLAY` set), run
      `scripts/perf_budget.sh` and report `zoom_culled.cpu.median_ms` (target at
      most 0.90); otherwise say so. The orchestrator re-records
      `PERF_BUDGETS.md`; the worker does not edit it.

## Decisions

- **Cache shape**, a field of `Selection`:

  ```rust
  /// Finite values of decoded chunk columns, sorted ascending, for the
  /// edge solve (GUP-567). Keyed by (epoch, chunk, column, chunk rows).
  edge_cache: Mutex<EdgeCache>,

  struct EdgeCache {
      epoch: u64,
      entries: Vec<SortedChunk>, // oldest first
  }
  struct SortedChunk {
      chunk: usize,
      column: usize,
      rows: u32,
      values: Vec<f64>, // finite only, sorted with `total_cmp`
  }
  ```

  A `Mutex` because `edge_marks` takes `&self` and `Selection` is `Sync`. On a
  different `epoch` the entries are cleared. An entry matches on `chunk`,
  `column` and `rows` (an append grows the last chunk's rows, so it misses). The
  total cached values per layer are capped at `2 * EDGE_ROW_BUDGET`; inserting
  past the cap drops the oldest entries.

- **Lookup**:
  `fn sorted_extremes<T>(sorted: &[f64], axis: &EdgeAxis, c: &dyn DynColumnEncoding<T>) -> Option<(Option<f32>, Option<f32>)>`
  mirrors `in_domain_extremes`: the same constant-column case (first and last
  sorted values equal), then, with `t` monotonic in the value (the same
  assumption the scan already makes), `partition_point` for the first value with
  `t >= 0` and the last with `t <= 1` in key order (`sign` flips the order for a
  reversed range); the answer is those values' `t`, or `(None, None)` when none
  lies in `[0, 1]`.
- **Budget accounting unchanged**: a cache hit still charges the chunk's rows
  against the budget, so which chunks are answered exactly, and so every inset,
  is identical to today. The saving is the decode, the allocation and the scan.
- **Kept**: `in_domain_extremes` stays (now `#[cfg(test)]`) as the oracle for
  AC2's property test.
- **Not cached**: the branch with size columns (each row's own reach), which
  needs every row; it is not on the benchmark's path.

## Escalation Triggers

- AC1's rule stops the story.
- AC2's property test finds a case where binary search and the scan disagree
  (the monotonic assumption fails for some encoding).
- After the cache, AC4's targets are missed: report the instrumented breakdown
  and stop.

## Out of Scope

- Changing the budget or skipping chunks whose x range lies outside the x domain
  when solving y (a behaviour change; a separate story if wanted).
- Re-recording `PERF_BUDGETS.md` (orchestrator).
- GPU sorting and other N5 work (GUP-492).

## Technical Tasks

- [x] AC1 benchmark and spike measurement at the parent commit.
- [x] `EdgeCache`, `SortedChunk`, `sorted_extremes`; use them in `edge_marks`.
- [x] Property and invalidation tests.
- [x] Measure after; `scripts/perf_budget.sh` if a display is available.

## Dependencies

### Prerequisite Stories

- GUP-528: Data-Aware Edge Insets ✅ — the solve being sped up.

### Enables Stories

None directly.

## Testing Strategy

- **Unit tests**: the property test and invalidation tests.
- **Integration tests**: GUP-528/540 tests and goldens unchanged.
- **Performance**: AC1 and AC4 with the committed headless benchmark;
  `zoom_bench` through `scripts/perf_budget.sh` where possible.

## Success Metrics

- [x] `edge_marks` at most 0.05 ms per frame on the 4M-row zoom.
- [x] `zoom_culled.cpu.median_ms` back under 0.90 ms.

## Risk Assessment

- **Medium**: each miss sorts a chunk (about 65,536 values, a few ms), so a zoom
  whose edge moves into a new chunk every frame would be slower than a scan.
  _Mitigation_: the benchmark's edge stays in one chunk; report p95 before and
  after, and escalate if p95 rises by more than 20%.
- **Low**: memory, at most 1 MB per layer. _Mitigation_: the cap.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no output change (AC3)
- [x] `mask wasm-browser` not needed: no layout, input or wasm code changes
      (insets are identical)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`Selection` holds `edge_cache: Mutex<EdgeCache>` (`selection.rs`): per (chunk,
column, chunk rows) the finite decoded values sorted with `total_cmp`, cleared
when `epoch` changes, capped at `2 * EDGE_ROW_BUDGET` values (oldest dropped).
The constant-size branch of `edge_marks` answers with `sorted_extremes` (binary
search with `partition_point`); the size-column branch is unchanged. Budget
accounting is unchanged, so insets are identical. `in_domain_extremes` is now
`#[cfg(test)]`, the oracle of the property test.

Files: `crates/gup-core/src/selection.rs`,
`crates/gup-core/examples/edge_inset_bench.rs` (new). Four new tests (2,000-case
property test; append, eviction and re-evaluation invalidation tests, each
checked to fail when the matching key is removed). All 261 `gup-core` lib tests,
every integration test and golden pass unchanged; `mask merge-check --dogfood`
and `cargo test --doc -p gup` pass.

Measurements (release, load average 3-7 on a shared machine, 1920x1080
offscreen, `edge_inset_bench`; temporary `Instant` around the `edge_marks`
calls, 659 frames including warm-up):

| Metric                        | Before | After |
| ----------------------------- | ------ | ----- |
| `edge_marks` median per frame | 0.676  | 0.019 |
| `edge_marks` p95 per frame    | n/a    | 0.027 |
| `edge.resolve.median_ms`      | 0.906  | 0.167 |
| `edge.resolve.p95_ms`         | 1.069  | 0.196 |

AC1 rule: 0.676 ms is above 0.3 ms, so the story continued. AC4: 0.019 ms is at
most 0.05 ms, and the resolve drop (0.739 ms) is 109% of the `edge_marks` time.
`scripts/perf_budget.sh --skip-wasm` (display present):
`zoom_culled.cpu.median_ms` 0.892, 0.795, 0.874 over three runs (recorded 1.24,
target 0.90); at the parent commit in the same session 1.993 (load).
`zoom.cpu.median_ms` (100k rows) measured 0.845, 0.872, 0.784 after and 0.829 at
the parent against a recorded 0.69: machine drift, not this change.
`mask wasm-size`: `wasm.scatter.gz_bytes` 348,546 (recorded 345,682, limit
352,596), `over_wgpu` 316,186, `per_row_type` 1,426. `PERF_BUDGETS.md` is for
the orchestrator to re-record.

## Retrospective

**Completed**: 2026-11-01

### Key Technical Learnings

#### Invalidation tests need a changed value that still binds

- **Challenge**: The first eviction and re-evaluation tests passed even with the
  epoch check removed, because the replacement rows no longer reached an edge,
  so the chunk was skipped and the stale entry was never read.
- **Solution**: Replacement rows still reach the edges, nearer than before, and
  each test was mutation-checked by removing the key it is about.
- **Pattern**: Prove a cache test can fail by deleting the key it guards.

#### Warm versus cold comparison

- **Challenge**: Comparing with a fresh plot differs by chunk layout under
  eviction.
- **Solution**: Resolve with the stale-able cache, clear it, resolve again, and
  require identical insets.

### Architectural Decisions

#### Cache stays inside `Selection`

- **Decision**: As in the story, a `Mutex` field, no new trait surface.
- **Trade-off**: A miss sorts a chunk (a few ms); a zoom whose edge moves to a
  new chunk each frame costs more than the scan did. The benchmark stays in one
  chunk; p95 fell (1.069 to 0.196 ms).

### Development Workflow Insights

- Heredocs in Bash calls were refused in this worktree; the Write tool plus
  `cat >>` worked.
- `zoom.cpu.median_ms` (100k) swings 0.78-0.87 ms with load; compare alternately
  against the parent before blaming a change.

### Follow-up Stories

None. A y solve that skips chunks whose x range is outside the x domain is
already listed under Out of Scope.
