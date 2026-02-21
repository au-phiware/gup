# GUP-128: Debug GPU Hit Test Element Detection

**Status**: 🚧 In Progress (2024-02-22)

## Story Overview

**Title**: Debug and Fix GPU Hit Test Element Detection  
**Epic**: Phase 2 Initiative 1 - Interactive Visualizations  
**Priority**: High  
**Story Points**: 5

## Context

During GUP-031 implementation, 3 interaction system tests started failing with
GPU hit tests returning 0 hits when they should find elements. The core
integration between Selection and the interaction system is complete, but the
GPU compute shader is not detecting elements at expected positions.

## User Story

**As a** visualization developer  
**I want** GPU hit testing to accurately detect elements at their positions  
**So that** interaction events fire correctly when users click on data points

## Acceptance Criteria

### AC1: Failing Tests Pass

- [ ] `test_point_query_accuracy` passes - finds elements at exact positions
- [ ] `test_multiple_queries` passes - handles batch queries correctly
- [ ] `test_different_mark_types` passes - works across Circle, Rectangle marks

### AC2: Root Cause Identified

- [ ] Document whether issue is in: element upload, coordinate transform, or
      shader logic
- [ ] Add debug logging or validation to prevent regression
- [ ] Update any incorrect assumptions in shader or Rust code

### AC3: Data Flow Validation

- [ ] Verify `InteractionElement` data uploads correctly to GPU buffers
- [ ] Verify query coordinates match element coordinate space
- [ ] Verify circle radius calculations in hit test shader

## Technical Tasks

### 1. Element Data Upload Validation

- [ ] Add debug logging for `InteractionElement` data before GPU upload
- [ ] Verify bytemuck serialization produces correct byte layout
- [ ] Check buffer sizes match expected element counts

### 2. Coordinate Space Investigation

- [ ] Document coordinate system expectations (screen vs world space)
- [ ] Verify query positions match element positions
- [ ] Check if coordinate transformations are needed

### 3. Shader Logic Debugging

- [ ] Review `test_circle_hit` function in hit_test.compute.wgsl
- [ ] Verify distance calculations are correct
- [ ] Check radius comparison logic
- [ ] Add shader validation tests

### 4. Fix Implementation

- [ ] Apply necessary fixes to element extraction, coordinates, or shader
- [ ] Ensure all 13 interaction tests pass
- [ ] Run with `--test-threads=1` as required for GPU tests

## Dependencies

- **Requires**: GUP-031 (Selection Integration) - ⚠️ Partial Complete
- **Blocks**: Full interaction system functionality
- **Enables**: Event-driven visualizations

## Success Metrics

- [ ] All 13 interaction system tests pass
- [ ] GPU hit testing works for 100K+ elements
- [ ] No performance regression from fixes

## Risk Assessment

**Medium Risk**: GPU shader debugging can be time-intensive without proper
tooling. May need to create debug visualization tools.

---

_Created from GUP-031 retrospective - identified GPU hit test issues preventing
full story completion._
