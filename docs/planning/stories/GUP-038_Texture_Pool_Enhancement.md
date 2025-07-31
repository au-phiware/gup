# GUP-038: Texture Pool Enhancement

## Story Overview

**Title**: Enhanced Texture Pool with Size Classes and Reuse **Epic**: Phase 1
Initiative 1 - Core GPU Primitives and Selection API **Priority**: Medium
**Story Points**: 3

## Context

The current `TexturePool` in `GupContext` is a placeholder implementation that
only creates new textures. A full texture pool implementation with size classes,
reuse patterns, and efficient memory management would improve performance and
reduce GPU memory fragmentation.

## User Story

**As a** Gup library developer **I want** an efficient texture pool that reuses
textures based on size and format **So that** I can reduce GPU memory allocation
overhead and improve rendering performance

## Acceptance Criteria

### AC1: Size-Class Based Allocation

- [ ] Implement texture size classes similar to BufferPool
- [ ] Support common texture formats (RGBA8, BGRA8, Depth24Plus)
- [ ] Round up texture sizes to power-of-2 dimensions for better reuse
- [ ] Track texture usage statistics

### AC2: Texture Reuse System

- [ ] Return textures to pool when no longer needed
- [ ] Match texture requests to compatible pooled textures
- [ ] Implement texture clearing/reset for reuse
- [ ] Support both 2D and 3D texture pooling

### AC3: Memory Management

- [ ] Automatic cleanup of unused textures
- [ ] Configurable pool size limits
- [ ] Memory pressure handling
- [ ] Integration with GupContext performance monitoring

## Technical Requirements

```rust
pub struct TexturePool {
    pools: HashMap<TextureKey, Vec<Texture>>,
    device: Arc<Device>,
    stats: TexturePoolStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureKey {
    format: TextureFormat,
    dimension: TextureDimension,
    size_class: (u32, u32, u32), // Power-of-2 rounded dimensions
    usage: TextureUsages,
}
```

## Dependencies

- GUP-004: Basic Render Context (completed)

## Success Metrics

- [ ] > 80% texture reuse rate in typical rendering scenarios
- [ ] <10% memory overhead from pooling
- [ ] Compatible with existing GupContext API
