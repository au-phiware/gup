# GUP-370: GPU Timestamp Query Profiling

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-27
**Revised**: 2026-10-04

**Rescoped 2026-10-04**: per the
[October 2026 strategic review](../STRATEGIC_REVIEW_2026-10.md#backlog-triage),
this story no longer proposes a new `GpuTimingContext` type or a `gpu-profiling`
feature flag. GUP-292 already delivered a generic, reusable
[`GpuTimer`](../../../src/gpu_timer.rs) (two-slot timestamp query set +
resolve/readback), built for exactly this kind of per-dispatch GPU timing. The
remaining work is to instrument the treemap compute passes with that existing
type, not to build a second one.

## Context

GUP-312 implemented GPU compute shaders for treemap layout but verified
performance through wall-clock timing rather than GPU-side timestamp queries.
GUP-292 subsequently added `GpuTimer` (`src/gpu_timer.rs`) — a lightweight
wrapper around a two-slot `wgpu::QuerySet` with
`compute_pass_timestamp_writes()`, `resolve()` and `read_elapsed_ns()` — for
exactly this purpose, and it already returns `None` cleanly when the device
lacks `Features::TIMESTAMP_QUERY`. This story instruments the treemap compute
pipeline (prefix-sum and per-depth layout passes from GUP-312) with `GpuTimer`
so treemap performance work has nanosecond-precision GPU-side timing instead of
wall-clock timing, without introducing a parallel timing abstraction.

## User Story

> "As a developer optimising the GPU treemap compute pipeline, I want each
> compute pass timed with the existing `GpuTimer`, so that I can measure GPU
> execution time precisely and detect performance regressions without building
> new profiling infrastructure."

## Acceptance Criteria

- [ ] Each treemap compute dispatch (prefix sum, per-depth layout pass) is
      wrapped with a `GpuTimer` instance obtained via
      `GpuTimer::new(device,     queue)`.
- [ ] When `GpuTimer::new` returns `None` (no `TIMESTAMP_QUERY` support), the
      treemap compute path runs unmodified — no new fallback timing code is
      added.
- [ ] Per-pass timings are surfaced through the existing performance reporting
      path (`src/performance.rs` / `src/performance_export.rs`), not a new
      ad-hoc struct.
- [ ] No new Cargo feature flag is introduced; `GpuTimer` is already
      zero-overhead when unused (the caller simply does not construct it).

## Technical Tasks

- [ ] Identify the treemap compute dispatch sites added by GUP-312
      (`src/chart_builder/builders` or `src/lod`/treemap compute module —
      confirm exact path at implementation time) and wrap each with
      `GpuTimer::compute_pass_timestamp_writes()`.
- [ ] Call `timer.resolve(&mut encoder)` after each pass and
      `timer.read_elapsed_ns(device)` to collect the result.
- [ ] Feed collected per-pass nanosecond timings into the existing performance
      report structures used elsewhere in the codebase.
- [ ] Add a unit/integration test asserting timestamp values are monotonically
      increasing and, where `TIMESTAMP_QUERY` is unavailable, that the compute
      path still completes correctly.

## Dependencies

### Prerequisite Stories

- GUP-312: GPU Compute Treemap ✅ — provides the compute dispatch sites to
  instrument.
- GUP-292: GPU Timestamp Query Profiling ✅ — delivered the `GpuTimer` type this
  story reuses (`src/gpu_timer.rs`).

## Testing Strategy

- Verify timestamp values are monotonically increasing when `TIMESTAMP_QUERY` is
  supported.
- Verify the treemap compute path still produces correct layout results when
  `GpuTimer::new` returns `None`.
- Run with `--test-threads=1`.

## Risk Assessment

- **Low**: `GpuTimer` already exists, is tested, and handles the
  feature-detection fallback. The remaining work is wiring it into the treemap
  dispatch sites, not designing new GPU timing infrastructure.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Retrospective added
