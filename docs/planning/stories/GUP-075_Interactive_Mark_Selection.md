# GUP-075: Interactive Mark Selection

**Story ID**: GUP-075  
**Title**: Interactive Mark Selection  
**Status**: 🚧 In Progress  
**Priority**: Medium  
**Effort**: 6 story points  
**Created**: 2025-08-04  
**Dependencies**: GUP-011 (Mark-Shader Integration)

## Summary

Implement GPU-accelerated interactive selection system for marks, enabling
efficient click detection, hover effects, and multi-selection for large datasets
without CPU bottlenecks.

## Background

Current selection system in GUP-011 has basic event handling infrastructure but
lacks efficient implementation for large datasets. Interactive data
visualization requires:

- Fast hit testing for mouse clicks on marks
- Hover effects that respond in real-time
- Multi-selection with drag rectangles
- Visual feedback for selection state
- Integration with data filtering and highlighting

For large datasets (>100K points), CPU-based hit testing becomes prohibitively
slow. GPU-based selection can leverage parallel processing for sub-millisecond
response times.

## Requirements

### Functional Requirements

1. **GPU-Based Hit Testing**
   - Render marks to off-screen selection buffer with unique IDs
   - Use mouse coordinates to query selection buffer for clicked mark
   - Support sub-pixel accuracy for small marks

2. **Selection State Management**
   - Track selected/unselected state for each mark
   - Support single-selection and multi-selection modes
   - Implement selection persistence across data updates

3. **Visual Selection Feedback**
   - Highlight selected marks with configurable styling
   - Hover effects that activate on mouse movement
   - Selection indicators (outlines, color changes, size scaling)

4. **Interactive Selection Tools**
   - Rectangle selection for selecting multiple marks
   - Lasso selection for arbitrary shape selection
   - Keyboard modifiers for additive/subtractive selection

### Non-Functional Requirements

1. **Performance**: \<1ms response time for selection operations on 100K+ marks
2. **Visual Quality**: Smooth hover transitions, anti-aliased selection
   indicators
3. **Usability**: Intuitive selection behavior matching desktop application
   standards

## Acceptance Criteria

1. **Hit Testing Implementation**
   - [ ] Off-screen selection buffer rendering with unique mark IDs
   - [ ] Mouse coordinate to mark ID mapping in \<1ms
   - [ ] Sub-pixel accuracy for marks smaller than 1px screen space
   - [ ] Correct hit testing with marks of different shapes/sizes

2. **Selection State System**
   - [ ] Efficient selection state storage (bitset for 1M+ marks)
   - [ ] Selection persistence during data updates/filtering
   - [ ] Undo/redo support for selection operations
   - [ ] Selection state serialization for save/load

3. **Visual Feedback**
   - [ ] Configurable selection highlighting (color, outline, scale)
   - [ ] Smooth hover animations (\<16ms transition time)
   - [ ] Selection indicators don't interfere with mark rendering
   - [ ] Support for multiple selection visual styles

4. **Interactive Tools**
   - [ ] Rectangle selection with real-time visual feedback
   - [ ] Lasso selection using mouse path
   - [ ] Keyboard modifiers (Ctrl, Shift) for selection modes
   - [ ] Touch support for mobile devices

5. **Integration**
   - [ ] Selection events trigger callbacks with selected data
   - [ ] Integration with filtering system (show only selected)
   - [ ] Export selected data to various formats
   - [ ] Selection statistics (count, summary data)

## Technical Design

### Selection Buffer Rendering

```rust
pub struct SelectionRenderer {
    selection_texture: wgpu::Texture,
    selection_view: wgpu::TextureView,
    readback_buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
}

impl SelectionRenderer {
    pub async fn query_mark_at_position(&self, x: f32, y: f32) -> Option<MarkID> {
        // Render marks with unique colors to selection buffer
        // Read pixel at (x,y) to get mark ID
    }
}
```

### Selection State Management

```rust
pub struct SelectionState {
    selected_marks: BitSet,        // Efficient storage for large datasets
    hover_mark: Option<MarkID>,
    selection_mode: SelectionMode,
    undo_stack: Vec<SelectionOperation>,
}

#[derive(Debug, Clone)]
pub enum SelectionOperation {
    Select(Vec<MarkID>),
    Deselect(Vec<MarkID>),
    Clear,
    RectangleSelect { rect: Rectangle, additive: bool },
    LassoSelect { path: Vec<Point2>, additive: bool },
}
```

### Interactive Selection Tools

```rust
pub struct SelectionTool {
    kind: SelectionToolKind,
    state: ToolState,
    visual_overlay: SelectionOverlay,
}

pub enum SelectionToolKind {
    Point,                    // Single click selection
    Rectangle,               // Drag rectangle selection
    Lasso,                   // Free-form path selection
    Brush { radius: f32 },   // Brush-based selection
}

pub struct SelectionOverlay {
    vertex_buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    animation_state: AnimationState,
}
```

### Visual Feedback System

```rust
pub struct SelectionStyler {
    selected_style: MarkStyle,
    hover_style: MarkStyle,
    transition_duration: Duration,
    animations: HashMap<MarkID, StyleAnimation>,
}

#[derive(Debug, Clone)]
pub struct MarkStyle {
    pub color_multiplier: [f32; 4],
    pub outline_color: [f32; 4],
    pub outline_width: f32,
    pub scale_factor: f32,
    pub z_offset: f32,          // Bring selected marks to front
}
```

## Implementation Plan

### Phase 1: Hit Testing Infrastructure (2 points)

- Implement selection buffer rendering system
- Create off-screen texture for mark ID encoding
- Add mouse coordinate to mark ID mapping
- Basic single-click selection

### Phase 2: Selection State and Visual Feedback (2 points)

- Implement efficient selection state storage
- Add visual highlighting for selected marks
- Create hover effect system with smooth transitions
- Undo/redo for selection operations

### Phase 3: Advanced Selection Tools (2 points)

- Rectangle selection with drag feedback
- Lasso selection with path rendering
- Keyboard modifier support
- Touch gesture support for mobile

## Performance Considerations

### GPU Selection Buffer Optimization

- Use R32_UINT texture format for mark IDs (4 billion unique marks)
- Implement mark ID pooling to reuse freed IDs
- Batch selection buffer updates to minimize GPU stalls

### Memory Efficiency

- Use BitSet for selection state (1 bit per mark vs 1 byte per bool)
- Implement mark ID compression for sparse selections
- Stream selection data for datasets >10M points

### Visual Performance

- Render selection overlays on separate pass to avoid mark shader complexity
- Use instanced rendering for selection indicators
- Implement temporal coherence for animation systems

## Risks and Mitigations

1. **Risk**: Selection buffer readback latency on some GPUs
   - **Mitigation**: Implement async readback with frame delay, add CPU fallback

2. **Risk**: Complex interaction between selection and other systems
   - **Mitigation**: Design clean interfaces, comprehensive integration tests

3. **Risk**: Touch selection accuracy on mobile devices
   - **Mitigation**: Implement touch tolerance zones, larger hit targets

## Success Metrics

- \<1ms response time for click selection on 100K marks
- Smooth 60fps hover animations with \<16ms transition
- Rectangle selection handles 1M+ marks in \<10ms
- Memory usage \<1MB for selection state of 1M marks
- Zero visual artifacts during selection operations

## Future Considerations

- Integration with brushing and linking across multiple views
- Advanced selection patterns (select by data range, regex)
- Collaborative selection in multi-user environments
- Selection analytics and user behavior tracking
