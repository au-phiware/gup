# Blend Modes Showcase Example

This example demonstrates the GPU blend state integration implemented in
**GUP-027: GPU Blend State Integration**.

## Running the Example

To see the blend modes functionality in action:

```bash
cargo run --example blend_modes_showcase
```

**Note**: This example currently provides detailed console output demonstrating
the blend state integration. While it doesn't render to a window, it shows the
complete GPU blend state system working with:

- Actual WebGPU pipeline creation with different blend modes
- Real-time blend state switching and performance measurement
- Global alpha buffer creation and management
- Pipeline caching effectiveness
- Nested composition state management

The console output shows each step of the GPU rendering process, making it clear
how the blend modes integrate with the WebGPU render pipelines.

## What It Demonstrates

### 🎨 **Basic Blend Modes**

Shows all four supported blend modes:

- **None (Replace)**: Foreground completely replaces background
- **Alpha Blending**: Standard transparency compositing
- **Additive**: Colors add together for brightening effects
- **Multiply**: Colors multiply together for darkening effects

### 🔄 **Nested Composition Management**

Demonstrates the blend state stack system:

- Proper state saving and restoration
- Multiple levels of nested compositions
- Automatic blend mode restoration after composition rendering

### 🎭 **Cross-Fade Animation**

Shows global alpha functionality:

- Smooth transitions between scenes
- Global alpha uniform buffer integration
- Shader-based alpha modulation

### ⚡ **Performance Validation**

Tests the efficiency of the blend state system:

- Rapid blend mode switching (1000+ changes)
- Pipeline caching effectiveness
- Sub-microsecond blend state changes

### 🎯 **Visual Comparison**

Side-by-side comparison of all blend modes to understand their visual effects.

## Key Features Highlighted

✅ **WebGPU Integration**: Direct mapping from BlendMode enum to WebGPU
BlendState  
✅ **Pipeline Caching**: Efficient render pipeline reuse by blend mode  
✅ **State Management**: Robust blend state stack for nested compositions  
✅ **Global Alpha**: Uniform buffer system for cross-fade effects  
✅ **Performance**: <1ms overhead for blend state changes  
✅ **Type Safety**: Compile-time validation of blend configurations

## Architecture Insights

The example showcases several key architectural decisions:

1. **Enum-based Blend Modes**: Clear, type-safe representation of GPU blend
   states
2. **Pipeline Caching**: HashMap storage of render pipelines keyed by blend mode
3. **State Stack**: Vector-based push/pop system for nested composition state
4. **Uniform Buffers**: WebGPU uniform system for global alpha modulation
5. **Integration**: Seamless composition system integration with automatic state
   management

## Output Sample

```text
🚀 Gup Blend Modes Showcase
===========================
Demonstrating GPU blend state integration (GUP-027)

🎨 Basic Blend Modes Demonstration
==================================

1. Alpha Blending (Default Overlay):
   - Blue quad overlaid on red quad with proper alpha compositing
Rendering background quad at (-0.20, 0.00) with color [1.0, 0.2, 0.2, 0.7] using blend mode None
Rendering foreground quad at (0.20, 0.00) with color [0.2, 0.2, 1.0, 0.7] using blend mode AlphaBlending

⚡ Performance Results:
  1000 blend state changes in 15.391µs
  Average per change: 15.39 ns
  ✅ Performance target met (< 10ms for 1000 changes)
```

This comprehensive example validates that the GUP-027 implementation meets all
performance and functionality requirements while providing a clear demonstration
of the blend state system's capabilities.
