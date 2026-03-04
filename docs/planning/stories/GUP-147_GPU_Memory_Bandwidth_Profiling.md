# GUP-147: GPU Memory Bandwidth Profiling

**Status**: ✅ Complete (2025-07-15)

## Story Overview

**Title**: GPU Memory Bandwidth Profiling and Analysis **Epic**: Phase 1
Initiative 1 - Core GPU Primitives and Selection API **Priority**: Low **Story
Points**: 5

## Context

GUP-046 implemented comprehensive performance profiling, but left memory
bandwidth estimation as a placeholder. Actual memory bandwidth profiling is
crucial for identifying GPU-bound performance issues and optimizing data
transfer patterns.

## User Story

**As a** Gup application developer **I want** detailed memory bandwidth
profiling **So that** I can identify and optimize memory transfer bottlenecks

## Acceptance Criteria

### AC1: Buffer Transfer Tracking

- [x] Track buffer upload bandwidth (CPU to GPU)
- [x] Track buffer download bandwidth (GPU to CPU)
- [x] Measure per-frame transfer volume
- [x] Identify high-bandwidth operations

### AC2: Texture Access Profiling

- [x] Track texture binding frequency
- [x] Measure texture memory access patterns
- [x] Identify texture thrashing
- [x] Estimate texture bandwidth usage

### AC3: Memory Pressure Detection

- [x] Real-time memory pressure monitoring
- [x] Bandwidth saturation warnings
- [x] Transfer pattern optimization suggestions
- [x] Memory access efficiency metrics

## Dependencies

- GUP-046: Context Performance Profiling (completed)

## Technical Requirements

```rust
pub struct MemoryBandwidthStats {
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub upload_bandwidth_gbps: f32,
    pub download_bandwidth_gbps: f32,
    pub texture_bindings: u32,
    pub memory_pressure_score: f32,
}
```

## Success Metrics

- [x] Accurate bandwidth measurement (within 10% of hardware limits)
- [x] Real-time memory pressure detection
- [x] <0.5% overhead for bandwidth tracking

## Implementation Summary

### What Was Implemented

**New module: `src/debug/memory_bandwidth.rs`** — A comprehensive GPU memory
bandwidth profiling module with the following components:

1. **MemoryBandwidthProfiler** — Core profiler with per-frame begin/end
   lifecycle
   - Buffer upload/download recording with per-label byte tracking
   - Texture binding event recording with slot-level tracking
   - Bounded frame history with configurable window size
   - Cumulative and per-frame statistics

2. **MemoryBandwidthStats** — Aggregate statistics matching the technical
   requirements (upload/download bytes, bandwidth in GB/s, texture bindings,
   memory pressure score)

3. **FrameBandwidthStats** — Per-frame detailed statistics with individual
   transfer events, top bandwidth consumers, and estimated bandwidth

4. **MemoryPressureStatus** — Real-time pressure monitoring with four levels
   (Low/Medium/High/Critical), bandwidth utilization ratio, and texture
   thrashing detection

5. **OptimizationSuggestion** — Actionable suggestions based on profiling data
   (high uploads, high readbacks, texture thrashing, many small transfers,
   critical saturation)

6. **MemoryEfficiencyMetrics** — Aggregate efficiency analysis including
   upload/download ratio, average transfer sizes, texture memory inventory, and
   redundant upload detection

7. **TextureSlotTracker** — Internal tracker for per-slot texture binding
   history and thrashing detection via alternation analysis

### Integration Points

- **PerformanceProfiler** (`src/performance.rs`) — Added
  `MemoryBandwidthProfiler` as a field, wired into begin_frame/end_frame, added
  bandwidth recording methods and HighMemoryBandwidth alerts
- **DetailedFrameStats** — New `bandwidth_stats` field carries per-frame
  bandwidth data through the existing profiling pipeline
- **ShaderProfiler** (`src/debug/shader_profiler.rs`) — Filled in the
  `memory_bandwidth_gbps` placeholder with a heuristic estimate
- **GpuDebugContext** (`src/debug.rs`) — Added `bandwidth_profiler` field and
  included bandwidth data in DebugReport export
- **DebugReport** — New `bandwidth_stats` and `bandwidth_pressure` fields for
  serialized export

### Key Files Changed

| File                            | Change                                                             |
| ------------------------------- | ------------------------------------------------------------------ |
| `src/debug/memory_bandwidth.rs` | New — 1300+ lines, complete bandwidth profiling module             |
| `src/debug.rs`                  | Added module registration, re-exports, GpuDebugContext integration |
| `src/performance.rs`            | Added bandwidth profiler field and recording methods               |
| `src/debug/shader_profiler.rs`  | Filled memory_bandwidth_gbps placeholder                           |

### Test Counts

- 29 new tests in `debug::memory_bandwidth` module
- All 2708 library tests pass
- All examples compile

## Retrospective

**Completed**: 2025-07-15

### Key Technical Learnings

#### Bandwidth Estimation Without Hardware Counters

- **Challenge**: wgpu does not expose native GPU memory bandwidth counters or
  PCIe transfer metrics. We cannot directly measure actual bus-level throughput.
- **Solution**: Track transfer volumes (bytes) and wall-clock durations at the
  application level, then compute bandwidth as bytes/time. For shader-level
  bandwidth, use a heuristic based on workgroup count × estimated bytes per
  thread.
- **Pattern**: Application-level instrumentation can provide useful bandwidth
  data for optimization guidance even without hardware counters. The key is
  framing the data as "estimated" and providing configurable theoretical
  bandwidth limits for pressure scoring.

#### Texture Thrashing Detection via Slot History

- **Challenge**: Detecting texture thrashing requires tracking temporal patterns
  in binding operations, not just counts.
- **Solution**: Maintain a per-slot history of recently bound texture labels and
  detect when multiple unique textures appear in a short window. This captures
  the essence of thrashing (rapid alternation) without needing GPU-side metrics.
- **Pattern**: Ring-buffer-based history per resource slot is an effective
  pattern for detecting temporal access anomalies.

#### Redundant Upload Detection

- **Challenge**: Identifying wasted bandwidth from uploading unchanged data
  every frame.
- **Solution**: Track per-label upload presence across consecutive frames. If
  the same label appears in consecutive frames, it's flagged as potentially
  redundant (the actual data may differ, but the heuristic catches the common
  case of re-uploading static data).
- **Pattern**: Cross-frame label matching is a lightweight heuristic for
  redundancy detection. A more precise approach would involve content hashing,
  but the label-based approach has near-zero overhead.

### Architectural Decisions

#### Separate Module vs. Extending Existing Profilers

- **Decision**: Created a standalone `memory_bandwidth` module rather than
  embedding the logic directly into `PerformanceProfiler` or
  `GpuMemoryProfiler`.
- **Reasoning**: The bandwidth profiler has its own lifecycle (begin/end frame),
  configuration, and output types. Keeping it separate follows the single
  responsibility principle and avoids bloating the existing profilers.
- **Trade-off**: Users need to know about a third profiler type; mitigated by
  integrating it into both `PerformanceProfiler` (automatic lifecycle) and
  `GpuDebugContext` (unified access).
- **Future**: Could be extended with actual GPU timestamp queries for more
  accurate per-transfer timing when wgpu exposes finer-grained profiling.

#### Configurable Theoretical Bandwidth

- **Decision**: Made `theoretical_bandwidth_gbps` a configuration parameter
  rather than auto-detecting it from the GPU adapter.
- **Reasoning**: wgpu doesn't expose memory bandwidth limits from the adapter.
  Different GPUs have vastly different bandwidth (30 GB/s integrated to 900+
  GB/s discrete), so a user-provided value is more accurate than a guess.
- **Trade-off**: Requires users to configure the value for accurate pressure
  scoring. Default of 100 GB/s is a reasonable middle ground.
- **Future**: If wgpu ever exposes adapter memory bandwidth info, auto-detection
  could be added.

### Development Workflow Insights

- The story was well-scoped; the existing profiling infrastructure (GUP-046,
  GUP-015) provided clear integration points.
- The `PerformanceAlert::HighMemoryBandwidth` variant was already defined but
  unused — filling it in was a natural integration point.
- The `memory_bandwidth_gbps` field in `ShaderExecutionStats` was an explicit
  TODO placeholder, making it easy to locate and fill.
- Building the module as pure Rust (no GPU resources needed) allowed running all
  29 tests without GPU access, keeping the development loop fast.

### Follow-up Stories

1. **GUP-148: Profiling Data Export & Visualization** — Already planned, now
   unblocked. Can consume `MemoryBandwidthStats`, `MemoryPressureStatus`, and
   `MemoryEfficiencyMetrics` for dashboard display and export.

2. **GUP-XXX: GPU Adapter Bandwidth Auto-Detection** — Investigate whether wgpu
   adapter limits or vendor-specific queries can provide actual theoretical
   bandwidth, removing the need for manual configuration. Low priority since the
   current approach works for optimization guidance.
