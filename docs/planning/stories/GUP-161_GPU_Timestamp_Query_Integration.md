# GUP-161: GPU Timestamp Query Integration

## Story Overview

**Title**: Integrate GPU Timestamp Queries for Accurate Performance
Measurement  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Low  
**Story Points**: 3  
**Status**: 📋 Planned

## Context

GUP-156 implemented pattern performance benchmarks, but they only measure
CPU-side overhead (command encoding, submission, polling). To accurately
validate the <5ms target for 100K points, we need GPU timestamp queries to
measure actual fragment shader execution time on the GPU.

The current benchmarks are useful for detecting CPU-side regressions but don't
capture the true rendering cost of pattern generation in fragment shaders.

## User Story

**As a** performance engineer  
**I want** accurate GPU execution time measurements  
**So that** I can validate fragment shader performance and identify GPU
bottlenecks

## Acceptance Criteria

### AC1: GPU Timestamp Query Setup

- [ ] Enable `TIMESTAMP_QUERY` wgpu feature
- [ ] Create timestamp query pools
- [ ] Handle query result readback
- [ ] Support query resolution (timestamp period)

### AC2: Pattern Rendering Measurements

- [ ] Measure fragment shader execution time for each pattern
- [ ] Capture timestamps for render passes
- [ ] Calculate GPU time from query results
- [ ] Validate <5ms target with actual GPU measurements

### AC3: Benchmark Integration

- [ ] Integrate timestamp queries into pattern benchmarks
- [ ] Report both CPU and GPU metrics
- [ ] Compare CPU overhead vs GPU execution time
- [ ] Document measurement methodology

## Dependencies

### Prerequisite Stories

- GUP-156: Pattern Performance Benchmarking ✅

## Technical Tasks

- [ ] Add `TIMESTAMP_QUERY` feature to benchmark device creation
- [ ] Implement timestamp query pool management
- [ ] Add render pass timestamp annotations
- [ ] Implement query result readback and parsing
- [ ] Update pattern benchmarks with GPU metrics
- [ ] Document GPU vs CPU time interpretation

## Success Metrics

- Accurate GPU timing for pattern rendering
- <5ms target validated with GPU measurements
- Both CPU and GPU metrics available
- Performance insights from CPU/GPU breakdown

## Definition of Done

- [ ] GPU timestamp queries functional
- [ ] Pattern benchmarks report GPU time
- [ ] <5ms target validated or exceeded
- [ ] Documentation updated with GPU metrics
- [ ] CI integration consideration documented
