---
implementer: sonnet
implementer-reason:
  a measured optimisation inside one function family (the tier build's scratch
  memory) with the existing brute-force tests as the oracle
touches: [crates/gup-core/src/lod.rs, crates/gup-core/src/lod/pool.rs]
---

# GUP-748: Parallel Tier Builds That Scale

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-19

## Context

GUP-730 builds chunks' level-of-detail tiers on a pool of
`available_parallelism() - 1` worker threads (7 here), the waiting frame's
thread helping. A headless first resolve of an owned 11M-row layer (11 chunks of
1M rows,
`scale_bench --rows 11000000 --resolve-only --no-render --lod-draw auto`) takes
642-747 ms against 231-250 ms under `Lod::Off`: the builds add about 424 ms,
against 1,021 ms on one thread (GUP-716), a speedup of 2.4 on 8 threads where 11
chunks over 8 threads would take about 2 rounds of a ~90 ms build. A hosted plot
installs all 11 in 365-385 ms after its first frame.

Each build allocates its scratch afresh (16 B a row for the radix sort, then the
gathered representatives, GUP-714), so 8 parallel builds fault in about 130 MB
of new pages at once; page faults serialise on the process's memory map. Not
measured further.

## User Story

> "As a developer rendering a 100M-row scatter headless, I want the tier builds
> to use the machine's cores."

## Acceptance Criteria

- [ ] Measure where the parallel build's time goes (one chunk alone against 8 at
      once; allocation against sort against gather).
- [ ] If allocation is the cost: each worker keeps its scratch buffers between
      builds (bounded by one chunk's need), and the headless 11M first resolve
      costs at most 2x one chunk's build more than `Lod::Off`, median of three
      alternating runs.
- [ ] The builds stay bit-identical (`lod::tests` brute force, GUP-730's
      worker-equals-frame-thread test).
- [ ] `wasm32` unchanged (no pool there).

## Decisions

- No new dependency (no rayon) unless the measurement shows std threads are the
  limit.

## Escalation Triggers

- The cost is memory bandwidth, not allocation: report the figures.

## Out of Scope

- A GPU build (GUP-714's note).

## Technical Tasks

- [ ] Measurement.
- [ ] Per-worker scratch, if warranted.

## Dependencies

### Prerequisite Stories

- GUP-730: Tier Builds Off the Frame Path.

### Enables Stories

- GUP-632: Hundred Million Row Demonstration (headless renders).

## Testing Strategy

- `scale_bench --rows 11000000 --resolve-only --no-render --lod-draw auto` (with
  and without `--hosted`), three runs each, alternating with `off`.

## Success Metrics

- [ ] Headless 11M first resolve within 2x one chunk's build of `Lod::Off`.

## Risk Assessment

- **Low**: the scratch is private to a build. _Mitigation_: the existing
  brute-force tests.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
