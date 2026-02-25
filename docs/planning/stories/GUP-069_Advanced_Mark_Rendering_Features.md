# GUP-069: Advanced Mark Rendering Features

**Status**: 🚧 In Progress  
**Priority**: Medium  
**Category**: Feature Enhancement  
**Estimated Effort**: 3 days  
**Dependencies**: GUP-068 (Mark Pipeline Integration)

## Summary

Enhance mark rendering with advanced features discovered during pipeline
integration. Build upon the solid foundation of GUP-068 to provide sophisticated
rendering capabilities for complex mark compositions and advanced use cases.

## Background

During GUP-068 implementation, several advanced rendering scenarios were
discovered that would benefit from dedicated implementation:

1. **Multi-pass Rendering**: Complex marks (e.g., stroked shapes, multi-layer
   effects) require multiple render passes
2. **Dynamic Attribute Mapping**: Runtime customization of mark attributes
   without pipeline recreation
3. **Advanced Blend Integration**: Sophisticated blending between marks within
   compositions
4. **Render Pass State Management**: Proper state isolation for mark
   compositions

## Requirements

### Core Features

1. **Multi-Pass Mark Rendering**
   - Support marks that require multiple render passes (base + outline, base +
     shadow, etc.)
   - Enable pass-specific shader variants and GPU state
   - Optimize performance for multi-pass scenarios

2. **Dynamic Attribute Mapping**
   - Runtime modification of attribute mappings without pipeline recreation
   - Support for conditional attribute assignments
   - Enable data-driven mark customization

3. **Advanced Blend Mode Integration**
   - Integration with existing blend mode system (GUP-027)
   - Mark-specific blend mode overrides
   - Composition-aware blending strategies

4. **Render Pass State Management**
   - Proper viewport and scissor management for mark rendering
   - State isolation between mark types in compositions
   - Performance optimization through state batching

### Performance Requirements

- Multi-pass rendering overhead: <20% compared to single-pass
- Dynamic attribute updates: <1ms for typical attribute changes
- State transitions: <0.1ms per mark type transition
- Blend mode transitions: <0.5ms per transition

### Integration Requirements

- Seamless integration with existing `MarkRenderer` and `MarkRegistry`
- Backward compatibility with single-pass marks
- Integration with composition system blend states
- Support for existing mark implementations without modification

## Technical Design

### Multi-Pass Rendering Architecture

```rust
pub trait MultiPassMark: Mark {
    type PassConfiguration: Clone + Send + Sync + 'static;

    fn pass_count() -> usize;
    fn pass_configuration(pass_index: usize) -> Self::PassConfiguration;
    fn create_pass_pipeline(device: &Device, pass_index: usize) -> GupResult<RenderPipeline>;
}

// Enhanced renderer for multi-pass support
impl MarkRenderer {
    pub fn render_multi_pass_marks<M: MultiPassMark>(
        &self,
        render_pass: &mut RenderPass,
        pipelines: &[Arc<RenderPipeline>],
        bind_groups: &[wgpu::BindGroup],
        instance_count: u32,
    ) -> GupResult<()>;
}
```

### Dynamic Attribute System

```rust
pub struct DynamicAttributeMapping {
    mappings: HashMap<String, AttributeBinding>,
    update_flags: HashSet<String>,
}

pub enum AttributeBinding {
    Static(Vec4),
    Dynamic(Box<dyn Fn(&dyn Any) -> Vec4 + Send + Sync>),
    Conditional(Box<dyn Fn(&dyn Any) -> Option<Vec4> + Send + Sync>),
}

impl MarkRegistry {
    pub fn update_attribute_mapping<M: Mark>(
        &mut self,
        attribute_name: &str,
        binding: AttributeBinding,
    ) -> GupResult<()>;
}
```

### Advanced Blend Integration

```rust
pub trait MarkBlendAware: Mark {
    fn preferred_blend_mode() -> BlendMode { BlendMode::AlphaBlending }
    fn supports_blend_override() -> bool { true }
    fn custom_blend_state() -> Option<wgpu::BlendState> { None }
}

impl MarkRenderer {
    pub fn render_with_blend_aware<M: MarkBlendAware>(
        &self,
        context: &mut RenderContext,
        // ... parameters
    ) -> GupResult<()>;
}
```

## Implementation Plan

### Phase 1: Multi-Pass Foundation (1 day)

- Define `MultiPassMark` trait and configuration system
- Extend `MarkRegistry` to handle multiple pipelines per mark
- Implement basic multi-pass rendering in `MarkRenderer`
- Create test mark with two-pass rendering (base + outline)

### Phase 2: Dynamic Attribute System (1 day)

- Implement `DynamicAttributeMapping` with runtime updates
- Add attribute binding system with static/dynamic/conditional variants
- Enable runtime attribute updates without pipeline recreation
- Implement caching for dynamic attribute evaluations

### Phase 3: Advanced Blend Integration (0.5 days)

- Extend marks with blend awareness capabilities
- Integrate with existing blend mode system from GUP-027
- Enable mark-specific blend overrides and custom states
- Optimize blend state transitions in rendering workflows

### Phase 4: Render Pass State Management (0.5 days)

- Implement proper state isolation for mark compositions
- Add viewport and scissor management for mark rendering
- Enable state batching optimizations for performance
- Create state transition performance validation

## Testing Strategy

### Integration Tests

- Multi-pass mark rendering with complex scenarios
- Dynamic attribute updates during rendering loops
- Blend mode transitions and state isolation
- State management with mark compositions

### Performance Tests

- Multi-pass rendering overhead measurement
- Dynamic attribute update timing validation
- State transition performance verification
- Memory usage optimization for multi-pass scenarios

### Visual Validation Tests

- Multi-pass rendering visual correctness
- Dynamic attribute changes produce expected results
- Blend mode integration works with mark compositions
- State isolation prevents unwanted interactions

## Success Criteria

1. Multi-pass marks render correctly with <20% performance overhead
2. Dynamic attributes update in <1ms for typical changes
3. Advanced blend integration works seamlessly with existing system
4. All existing mark implementations continue to work unchanged
5. Comprehensive test coverage for new advanced features
6. Clear documentation and examples for advanced mark development

## Future Integration

This story enables:

- Complex stroke and shadow effects for built-in marks
- Data-driven mark customization in visualization applications
- Advanced composition effects with sophisticated blending
- Performance-optimized rendering for complex mark hierarchies
