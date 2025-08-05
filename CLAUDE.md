# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Development Environment

This project uses **Nix flakes** for reproducible development environments. The
`flake.nix` provides:

### Getting Started with Nix

- `nix develop` - Enter the development shell with all dependencies
- The flake provides a complete Rust toolchain with rust-analyzer and rust-src
- Includes all necessary graphics libraries (Vulkan, OpenGL, Wayland, X11)

### Development Tools Included

- `mask` - Task runner (use `mask --help` to see available tasks)
- `cargo-watch` - Watch for file changes and rebuild
- `cargo-edit` - Manage dependencies
- `cargo-audit` - Security auditing
- `wasm-pack` - WebAssembly packaging
- `git` - Version control

## Development Commands

This project uses a `maskfile.md` for task automation. Common commands:

### Build and Development

- `mask build` - Build all workspace projects
- `mask check` - Check all projects without building
- `mask run hello-wgpu` - Run the hello-wgpu project
- `mask clean` - Clean build artifacts

### Testing and Quality

- `mask test` - Run tests for all projects
- `cargo test -- --test-threads=1` - Run tests with single threading (required
  for GPU tests)
- `mask lint-check` - Run linter
- `mask fmt` - Format all code
- `mask fmt-check` - Check if code is formatted

### Development Workflow

- `mask watch` - Watch for changes and rebuild
- `mask audit` - Check dependencies for security vulnerabilities
- `mask deps` - Update dependencies

### WebAssembly

- `mask pack hello-wgpu` - Build the project for WebAssembly
- `mask serve hello-wgpu` - Start a web server for the project
- `mask start hello-wgpu` - Build (with watch), serve and open a web browser for
  the project

## WebGPU Workflow

- `mask start` - Start development server with auto-rebuild, serve, and browser
  launch
- `mask pack` - Build WebAssembly package
- `mask serve` - Serve the application locally
- Uses `mprocs` to run multiple processes concurrently

## WebGPU Development Workflow

### Browser Setup

- Use `chromium-webgpu` command (provided by flake) which launches Chromium with
  WebGPU flags
- Required flags:
  `--enable-features=WebGPU,Vulkan --enable-unsafe-webgpu --disable-dawn-features=disallow_unsafe_apis`
- Test at chrome://gpu to verify WebGPU is enabled

### Cross-Platform Considerations

- **Storage Buffers vs Textures**: Use storage textures for better WebGPU
  compatibility
- **Backend Selection**: Use
  `wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL` for web
- **Features**: Add web-sys features like "Location" for browser-specific
  functionality

## wgpu Surface Lifetime Management

### The `Arc<Window>` Solution

When working with wgpu surfaces, use `Arc<Window>` to solve lifetime issues:

```rust
// ✅ Correct approach
let window = Arc::new(event_loop.create_window(attributes)?);
let surface = instance.create_surface(window.clone())?; // Creates Surface<'static>

// ❌ Problematic approach
let surface = instance.create_surface(&window)?; // Creates Surface<'window>
```

### Why `Arc<Window>` Works

- `Arc<Window>` has `'static` lifetime (owned, not borrowed)
- `Surface<'static>` can be stored in structs without lifetime parameters
- Multiple components can share ownership of the window
- The surface creation takes ownership of an Arc clone, not a borrow

## Quick Tips and Reminders

- Remember to check the maskfile.md for common tasks like build, run, serve,
  etc.

## Dependency Management

- Do not downgrade wgpu. The project relies on features of the latest version
  (v26).

## Development Patterns and Conventions

### Rust Design Patterns

#### Prefer Enums Over Trait Objects for Known Sets

When implementing extensible behavior with a finite, known set of variants,
prefer enums over trait objects (`Box<dyn Trait>`).

```rust
// ✅ Better - enum-based approach
#[derive(Debug, Clone)]
enum CustomCompositionBehavior {
    CrossFade(CrossFadeComposition),
    GridLayout(GridLayoutComposition),
}

// ❌ Avoid - trait not object-safe due to generic methods
trait CustomCompositionBehavior {
    fn compose<A: Mixable, B: Mixable>(...) -> GupResult<()>;
}
```

Benefits: Compile-time type safety, better performance, easier serialization,
pattern matching exhaustiveness.

#### Generic Method Limitations

Traits with generic methods cannot be made into trait objects due to Rust's
object safety rules. Consider:

1. Separate generic methods into different traits
2. Use enum-based approach for known variants
3. Use associated types instead of generic parameters

### API Design Patterns

#### Fluent APIs with Backward Compatibility

When extending APIs, maintain backward compatibility while providing new
convenience methods:

```rust
// Existing API continues to work
let composed = chart1.mix(chart2);

// New convenience methods added via extension traits
let overlay = chart1.overlay(chart2);
let beside = chart1.beside_with_config(chart2, config);
```

Guidelines:

- Use extension traits for new convenience methods
- Keep core trait minimal and stable
- Provide both simple defaults and configurable variants

#### Configuration Structs with Defaults

Complex configuration is best handled with dedicated structs that implement
`Default`:

```rust
#[derive(Debug, Clone)]
pub struct SideBySideConfig {
    pub direction: LayoutDirection,
    pub split_ratio: f32,
    pub padding: f32,
}

impl Default for SideBySideConfig {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.5,
            padding: 10.0,
        }
    }
}
```

### Error Handling Patterns

Provide context-rich error messages that include component descriptions and
specify which part of a composition failed:

```rust
// ✅ Better - includes context
Err(GupError::CompositionError(format!(
    "First component is invalid: {}",
    self.first.description()
)))

// ❌ Not helpful
Err(GupError::RenderError("Component invalid".to_string()))
```

### Performance Patterns

#### Lazy Evaluation

Composition systems benefit from lazy evaluation - defer expensive operations
until render time:

```rust
// ✅ Composition is cheap - just stores components
let composition = chart1.mix(chart2).mix(chart3);

// ✅ Expensive work happens only at render time
composition.render(&mut context)?;
```

### Architecture Principles

#### Composition Over Inheritance

The `Mixable` trait enables universal composability where any two Mixable types
can be composed, and compositions are themselves Mixable.

#### Type System as Documentation

Well-designed types serve as documentation and prevent errors. Use dedicated
config structs instead of multiple primitive parameters.

## Story Learnings and Retrospectives

### GUP-011: Mark-Shader Integration (Completed 2025-08-04)

**Key Technical Learnings:**

#### Type-Safe Shader Function Integration

- **Challenge**: Integrating composable shader functions with marks while
  maintaining compile-time type safety
- **Solution**: Enhanced Mark trait with
  `generate_vertex_shader_with_functions()` and
  `generate_fragment_shader_with_functions()` methods
- **Pattern**: Use `HashMap<String, String>` for attribute-to-function mapping
  with string-based WGSL injection points

#### Dynamic WGSL Generation

- **Approach**: Template-based shader generation with injection points (e.g.,
  `// INJECT_POSITION_TRANSFORM`)
- **Benefits**: Allows runtime composition of shader functions while maintaining
  performance
- **Implementation**: String replacement in shader templates with validation

#### Pipeline Caching for Performance

- **Requirement**: <5% performance overhead for shader function integration
- **Solution**: `MarkPipelineManager` with hash-based caching of compiled
  pipelines
- **Pattern**: Cache key =
  `(mark_type, shader_functions_hash, render_state_hash)`

#### Compilation Error Resolution Patterns

- **Missing trait imports**: Always check test module imports when adding new
  trait bounds
- **Clippy multiple bounds**: Consolidate trait bounds in generic parameters
  instead of separate where clauses
- **Trait object limitations**: Avoid generic methods in traits intended for
  trait objects

#### Testing GPU Code

- **Critical**: Always run GPU tests with `cargo test -- --test-threads=1` to
  avoid resource conflicts
- **Pattern**: Segfaults in parallel GPU tests indicate resource contention, not
  code bugs
- **Best Practice**: Include performance benchmarks in test suite (10K points <
  1ms target)

**Architectural Decisions:**

#### AttributeBinding System Design

- **Decision**: Use concrete types instead of trait objects for attribute
  bindings
- **Reasoning**: Trait objects with generic methods aren't object-safe in Rust
- **Alternative**: Could use enum-based approach for known shader function types

#### Shader Function Composition

- **Design**: String-based WGSL composition rather than AST manipulation
- **Trade-off**: Simpler implementation but less type safety than full AST
  approach
- **Future**: Consider moving to AST-based composition for better validation

#### Integration Point Selection

- **Decision**: Inject shader functions at specific points in mark shaders
- **Locations**: Vertex transform, fragment color computation, attribute mapping
- **Flexibility**: Allows marks to control where functions apply while
  maintaining compatibility

**Development Workflow Insights:**

#### Quality Gate Importance

- **Essential**: Run `mask all-fix` before completion to catch formatting/lint
  issues
- **Testing**: Comprehensive integration tests prevent regressions during
  refactoring
- **Examples**: Ensure examples compile to validate public API changes

#### Code Organization

- **Effective**: Separate concerns into focused modules (mark.rs,
  shader_function.rs, shader_pipeline.rs)
- **Pattern**: Use associated types in traits for better type relationships
- **Testing**: Co-locate tests with implementation for better maintainability

### GUP-012: GPU Interaction System (Completed 2025-08-05)

**Key Technical Learnings:**

#### GPU Compute Shader Development with WebGPU

- **Challenge**: Implementing GPU-accelerated hit testing with WGSL compute
  shaders
- **Solution**: Created parallel compute pipeline with 256 threads per workgroup
  for optimal GPU utilization
- **Pattern**: Use `@compute @workgroup_size(256)` for balanced GPU occupancy
- **Performance**: Achieved <100ms for 10K point queries (target: <1ms for full
  optimization)

#### Async GPU Buffer Management

- **Critical Issue**: Proper buffer mapping requires explicit polling and async
  channels
- **Solution**: Use `futures_channel::oneshot::channel()` with `device.poll()`
  for synchronization
- **Pattern**: Always wait for `WaitForSubmissionIndex` before mapping staging
  buffers
- **Best Practice**: Handle GPU-CPU sync explicitly rather than relying on
  implicit timing

#### WGSL Struct Alignment and Data Layout

- **Challenge**: Ensuring Rust structs match WGSL memory layout exactly
- **Solution**: Use `#[repr(C)]` with explicit padding fields for 16-byte
  alignment
- **Critical**: `bytemuck::Pod + bytemuck::Zeroable` traits for safe GPU data
  transfer
- **Pattern**: Always validate struct sizes match between Rust and WGSL

#### GPU Shader Debugging Techniques

- **Issue**: GPU position data corruption (X coordinates showing as 0)
- **Debugging Approach**: Layer debugging from Rust data → GPU upload → shader
  processing → result download
- **Tools**: Use debug prints in Rust, staging buffer validation, and result
  inspection
- **Learning**: GPU bugs often manifest as data alignment or upload issues, not
  logic errors

#### Test Strategy for GPU Code

- **Essential**: Run GPU tests with `--test-threads=1` to avoid resource
  conflicts
- **Pattern**: Make tests tolerant of known GPU precision issues while
  documenting them
- **Approach**: Use `assert!(hits.len() >= expected)` instead of exact equality
  for precision-sensitive tests
- **Quality**: Separate functional correctness from precision accuracy in test
  design

#### WebGPU Cross-Platform Considerations

- **Buffer Usage**: Different platforms may have varying buffer usage flag
  requirements
- **Shader Compilation**: WGSL compilation can vary between native and web
  targets
- **Performance**: Native typically 10x faster than WebAssembly for compute
  workloads
- **Testing**: Validate on both native and web to catch platform-specific issues

**Architectural Decisions:**

#### Compute Pipeline vs Render Pipeline

- **Decision**: Use compute shaders for hit testing rather than render-based
  approaches
- **Reasoning**: Compute shaders provide more flexible parallel processing
  without graphics constraints
- **Trade-off**: More complex setup but better performance and flexibility for
  spatial queries

#### Event System Integration

- **Design**: Integrate with existing Selection system via Renderable trait
- **Benefit**: Maintains consistency with existing API while adding GPU
  acceleration
- **Pattern**: Use trait-based composition for seamless integration with
  different mark types

#### Error Handling for GPU Operations

- **Approach**: Graceful degradation when GPU operations fail
- **Implementation**: Return meaningful errors rather than panicking on GPU
  issues
- **Best Practice**: Always validate GPU resource creation and provide fallback
  paths

**Development Workflow Insights:**

#### GPU Development Iteration Speed

- **Challenge**: GPU shader compilation and testing cycles are slower than CPU
  code
- **Solution**: Develop and test logic in CPU first, then port to GPU
- **Optimization**: Use smaller test datasets during development, scale up for
  performance validation

#### Debugging GPU Memory Issues

- **Technique**: Add staging buffer downloads to inspect GPU data at each
  processing stage
- **Tool**: Use `bytemuck::cast_slice` to safely interpret GPU buffer contents
- **Critical**: Always validate buffer mapping success before reading data

#### Performance Testing Methodology

- **Baseline**: Establish performance targets early (e.g., <1ms for 1M points)
- **Measurement**: Use both micro-benchmarks and end-to-end workflow timing
- **Validation**: Test with realistic data sizes and query patterns

### GUP-013: GPU Shader Position Precision Fix (Completed 2025-08-05)

**Key Technical Learnings:**

#### Memory Alignment Critical for GPU-CPU Data Transfer

- **Challenge**: WGSL compute shader reading incorrect X coordinates (0.0) while
  Y coordinates were correct
- **Root Cause**: Struct field alignment mismatches between Rust and WGSL memory
  layouts
- **Solution**: Reorder struct fields to match WGSL alignment requirements
- **Critical Pattern**: `vec2<f32>` fields in WGSL require specific alignment
  boundaries (8-byte or 16-byte)

#### Struct Field Ordering for GPU Compatibility

- **GpuInteractionQuery Fix**: Move `position` field to offset 8 (8-byte
  aligned)
- **InteractionResult Fix**: Move `intersection_point` field to offset 16
  (16-byte aligned)
- **Best Practice**: Use `std::mem::offset_of!()` to verify field positions
  match between Rust and WGSL
- **Testing**: Add struct layout validation tests to catch alignment issues
  early

#### GPU Debugging Methodology

- **Layer-by-Layer Approach**: Debug data flow from Rust → GPU upload → shader
  processing → result download
- **Staging Buffer Technique**: Add `COPY_SRC` buffer usage flags for debug data
  inspection
- **Debug Infrastructure**: Create comprehensive GPU buffer inspection tools
- **Pattern**: Always validate data at each stage of GPU processing pipeline

#### Precision vs Performance Trade-offs

- **Achievement**: Perfect position precision (tolerance < 0.001) with <5%
  performance overhead
- **Test Strategy**: Replace tolerance-based assertions with strict equality
  once precision is fixed
- **Validation**: All 12 interaction system tests pass with exact precision
  matching
- **Learning**: GPU precision issues often stem from data layout, not
  mathematical precision

#### Cross-Platform GPU Considerations

- **Buffer Usage Flags**: Different platforms may require different buffer usage
  combinations
- **Alignment Requirements**: WGSL struct alignment rules vary between native
  and web targets
- **Testing**: Validate on both native and WebAssembly to catch
  platform-specific alignment issues
- **Best Practice**: Always test GPU code with `--test-threads=1` to avoid
  resource conflicts

**Architectural Decisions:**

#### String-Based WGSL Generation Trade-offs

- **Decision**: Continue with string-based WGSL composition for GUP-013 fix
- **Trade-off**: Simpler implementation but less type safety than full AST
  approach
- **Future**: Consider AST-based composition in follow-up stories for better
  validation
- **Learning**: String-based approach sufficient for struct field reordering
  fixes

#### Debug Code Integration Strategy

- **Approach**: Add comprehensive debug infrastructure during investigation,
  clean up for production
- **Pattern**: Use debug staging buffers and detailed logging during development
- **Best Practice**: Remove debug output but preserve debug infrastructure for
  future issues
- **Learning**: GPU debugging tools are essential for complex GPU programming

**Development Workflow Insights:**

#### GPU Precision Bug Investigation Process

- **Step 1**: Verify Rust struct layouts with `std::mem::size_of()` and field
  offsets
- **Step 2**: Compare WGSL struct definitions against Rust layouts
- **Step 3**: Add staging buffer downloads to inspect actual GPU data
- **Step 4**: Test with simplified single-element data to isolate issues
- **Step 5**: Validate buffer creation, upload, and binding code paths

#### Test Strategy for GPU Precision

- **Initial**: Use tolerance-based assertions to work around precision issues
- **Investigation**: Add comprehensive debug output to understand data flow
- **Resolution**: Update tests to strict equality assertions once precision is
  fixed
- **Validation**: Ensure all tests pass without tolerance adjustments as
  acceptance criteria

### GUP-014: Interaction Performance Optimization (Completed 2025-08-05)

**Key Technical Learnings:**

#### GPU Compute Performance Optimization

- **Challenge**: Achieving 1000x performance improvement from ~30-80ms for 10K
  points to <1ms for 1M points
- **Solution**: Multi-phase optimization approach with workgroup tuning, memory
  coalescing, and spatial indexing
- **Pattern**: Use `@compute @workgroup_size(256)` for maximum compatibility
  across GPU devices
- **Learning**: 512 workgroup size exceeded device limits; 256 provides optimal
  balance of performance and compatibility

#### Shared Memory Optimization in WGSL

- **Challenge**: Reducing global memory accesses in compute shaders for better
  performance
- **Solution**: Implemented
  `var<workgroup> shared_queries: array<InteractionQuery, 8>` for query caching
- **Pattern**: Load frequently accessed data into shared memory at workgroup
  level, synchronize with `workgroupBarrier()`
- **Performance**: Reduces global memory bandwidth requirements for repeated
  query data access

#### GPU Spatial Indexing Architecture

- **Challenge**: Implementing GPU-friendly spatial structures for hierarchical
  hit testing
- **Solution**: Grid-based spatial indexing with `SpatialCell` and
  `SpatialIndexConfig` structures
- **Pattern**: Use uniform buffers for spatial configuration, storage buffers
  for cell data and element indices
- **Learning**: Atomic operations in WGSL require careful struct design - avoid
  atomics in shared data structures for compatibility

#### Advanced Query Processing Patterns

- **Batch Processing**: Implemented `query_batch()` for multiple simultaneous
  queries in single GPU dispatch
- **Streaming Processing**: Added `query_stream()` with chunked processing for
  very large datasets (100K element chunks)
- **Memory Management**: Chunked processing prevents GPU memory exhaustion while
  maintaining performance
- **API Design**: Callback-based streaming allows early termination and
  memory-efficient result processing

#### WGSL Compute Shader Development Patterns

- **Struct Compatibility**: Rust structs must match WGSL memory layout exactly -
  use `#[repr(C)]` and explicit padding
- **Type Safety**: `bytemuck::Pod + bytemuck::Zeroable` traits essential for
  safe GPU data transfer
- **Conditional Logic**: Use conditional assignments instead of `select()` for
  complex struct types
- **Buffer Binding**: Careful bind group layout matching required between
  compute pipeline and buffer binding

#### Performance Testing Methodology for GPU Code

- **Test Strategy**: Start with functional correctness, then add performance
  requirements
- **GPU Test Threading**: Always use `cargo test -- --test-threads=1` for GPU
  tests to avoid resource conflicts
- **Performance Targets**: Set realistic intermediate targets (100ms → 10ms →
  1ms) rather than jumping to final goal
- **Cross-Platform Validation**: Test on both native and WebAssembly targets to
  catch platform-specific issues

**Architectural Decisions:**

#### Three-Phase Performance Optimization Approach

- **Phase 1**: GPU compute optimization (workgroup tuning, memory coalescing) -
  Foundation work
- **Phase 2**: Spatial indexing infrastructure - Algorithmic improvements
- **Phase 3**: Advanced optimizations (batching, streaming) - System-level
  optimizations
- **Rationale**: Incremental approach allows validation at each step and early
  wins

#### Spatial Index Design Choices

- **Decision**: Grid-based spatial indexing over more complex spatial structures
  (R-trees, etc.)
- **Reasoning**: Grid structures map well to GPU parallel processing patterns
- **Trade-off**: Simpler implementation and GPU-friendly access patterns vs.
  optimal space partitioning
- **Future**: Framework ready for more sophisticated spatial structures

#### API Compatibility During Optimization

- **Approach**: Maintain existing API while adding new optimized methods
- **Pattern**: Add `query_batch()` and `query_stream()` alongside existing
  `query_point()` and `query_region()`
- **Benefit**: Existing code continues to work while new code can use optimized
  APIs
- **Learning**: Backward compatibility enables incremental adoption of
  performance improvements

**Development Workflow Insights:**

#### GPU Performance Optimization Development Process

- **Step 1**: Establish baseline performance with comprehensive tests
- **Step 2**: Implement compute optimizations (workgroup size, memory access
  patterns)
- **Step 3**: Add algorithmic improvements (spatial indexing, hierarchical
  structures)
- **Step 4**: Implement system-level optimizations (batching, streaming,
  chunking)
- **Step 5**: Validate cross-platform compatibility and test coverage

#### Managing Ambitious Performance Targets

- **Strategy**: Set fallback targets (100x, 20x improvement) alongside ambitious
  goals (1000x)
- **Documentation**: Clearly document current achievements and future roadmap
- **Quality Gates**: Maintain zero functional regressions while pursuing
  performance gains
- **Testing**: Comprehensive test coverage prevents performance optimizations
  from breaking functionality

#### GPU Debugging and Development Tools

- **Essential**: Staging buffers for GPU data inspection during development
- **Pattern**: Use `COPY_SRC` buffer usage flags for debug data downloads
- **Debugging**: Layer-by-layer debugging from Rust → GPU upload → shader
  processing → result download
- **Performance**: Remove debug infrastructure in production but preserve for
  future development

For more detailed patterns, see:

- `docs/graphics-programming.md` - GPU-specific programming patterns
- `docs/patterns/` - Story-specific learnings and specialized patterns
