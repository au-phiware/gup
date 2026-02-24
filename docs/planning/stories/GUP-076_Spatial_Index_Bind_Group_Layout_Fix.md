# GUP-076: Spatial Index Bind Group Layout Fix

**Priority**: High  
**Complexity**: Medium  
**Created**: 2025-08-05  
**Status**: 🚧 In Progress

## Problem Statement

During GUP-014 implementation, spatial indexing infrastructure was created but
disabled due to bind group layout mismatches between the compute pipeline and
buffer binding. This story focuses on resolving these issues to enable spatial
indexing functionality.

## Current Status

- Spatial indexing framework implemented with `SpatialCell` and
  `SpatialIndexConfig`
- `spatial_index.compute.wgsl` compute shader created
- Bind group layout issues prevent spatial index activation
- Currently disabled with TODO comments in interaction system

## Technical Details

### Bind Group Layout Mismatch

The spatial index compute pipeline expects specific buffer layouts that don't
match the current binding configuration. Key issues:

- Buffer binding indices may not match pipeline expectations
- Storage buffer vs uniform buffer usage flags inconsistency
- Potential struct alignment issues between Rust and WGSL

### Error Patterns

Based on GUP-014 implementation:

```rust
// Currently disabled due to bind group layout issues
if false { // TODO: Enable when bind group layout is fixed
    self.build_spatial_index(elements).await?;
}
```

## Acceptance Criteria

- [ ] Spatial index compute pipeline successfully creates bind groups
- [ ] Buffer layouts match between Rust binding and WGSL pipeline
- [ ] Spatial indexing can be enabled in interaction system
- [ ] All existing tests continue to pass (164 tests)
- [ ] Performance improvement measurable with spatial indexing enabled

## Implementation Tasks

### 1. Diagnose Bind Group Layout Issues

- [ ] Compare expected vs actual bind group layouts
- [ ] Validate buffer usage flags (STORAGE vs UNIFORM)
- [ ] Check struct alignment between Rust and WGSL
- [ ] Review binding indices consistency

### 2. Fix Buffer Binding Configuration

- [ ] Update buffer creation with correct usage flags
- [ ] Ensure binding indices match pipeline expectations
- [ ] Validate struct memory layouts with `std::mem::offset_of!()`
- [ ] Test buffer binding with minimal example

### 3. Enable Spatial Indexing

- [ ] Remove TODO disable condition in interaction system
- [ ] Test spatial index building with real data
- [ ] Validate performance improvement over brute force approach
- [ ] Ensure backward compatibility maintained

### 4. Testing and Validation

- [ ] Create specific tests for spatial index functionality
- [ ] Benchmark performance with and without spatial indexing
- [ ] Validate cross-platform compatibility (native and WebAssembly)
- [ ] Ensure GPU resource cleanup works correctly

## Dependencies

- **Requires**: GUP-014 completion (spatial indexing infrastructure)
- **Blocks**: GUP-078 (spatial index algorithm optimization)
- **Related**: GUP-077 (performance benchmarking will validate improvements)

## Technical Risks

- **Medium**: Bind group layout issues may require architectural changes
- **Low**: Cross-platform buffer binding differences
- **Low**: Performance regression if spatial indexing overhead is high

## Success Metrics

- **Primary**: Spatial indexing successfully enabled and functional
- **Secondary**: Measurable performance improvement for large datasets
- **Quality**: Zero test regressions
- **Compatibility**: Works on both native and WebAssembly targets

## Performance Expectations

With spatial indexing enabled:

- Improved query performance for datasets >1K elements
- Reduced GPU compute time through spatial culling
- Foundation for achieving <1ms for 1M point queries target

## References

- GUP-014: Interaction Performance Optimization (completed infrastructure)
- `src/interaction.rs`: InteractionSystem implementation
- `src/shaders/spatial_index.compute.wgsl`: Spatial indexing compute shader
