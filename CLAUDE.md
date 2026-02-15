# CLAUDE.md

## Development Environment

This project uses **Nix flakes** for reproducible development environments.

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
- `mask clean` - Clean build artifacts

### Testing and Quality

- `mask test` - Run tests for all projects
- `cargo test -- --test-threads=1` - Run tests with single threading (required
  for GPU tests)

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

### GUP-015: GPU Debugging Tools (Completed 2025-08-06)

**Key Technical Learnings:**

#### Comprehensive GPU Debug Infrastructure Design

- **Challenge**: GPU development required specialized debugging tools for buffer
  inspection, shader profiling, and memory layout validation
- **Solution**: Created modular debug system with `GpuBufferInspector`,
  `ShaderProfiler`, and `MemoryLayoutValidator` components
- **Pattern**: Use unified `GpuDebugContext` to coordinate all debugging
  operations with centralized configuration
- **Architecture**: Separate modules for distinct concerns while providing
  unified API surface

#### Staging Buffer Management for GPU-CPU Data Transfer

- **Challenge**: Efficient GPU buffer content inspection requires complex
  staging buffer operations
- **Solution**: Implemented cached staging buffer system with automatic size
  management and reuse
- **Pattern**: Use `HashMap<u64, Buffer>` for staging buffer cache with buffer
  size as key
- **Critical**: Always use `COPY_SRC` usage flags on GPU buffers and `MAP_READ`
  on staging buffers for inspection
- **Best Practice**: Implement `clear_cache()` method for memory management in
  debug tools

#### Memory Layout Validation for Rust ↔ WGSL Compatibility

- **Challenge**: Preventing alignment issues like those encountered in GUP-013
  requires systematic layout checking
- **Solution**: Created `MemoryLayoutValidator` with `std::mem::offset_of!()`
  and compile-time validation
- **Pattern**: Use validation functions for each GPU struct with field offset
  verification
- **Critical Learning**: `vec2<f32>` fields must be aligned to 8-byte
  boundaries, complex structs to 16-byte boundaries
- **Testing**: Add `validate_common_gpu_structs()` to catch alignment issues
  early in development

#### Shader Profiling with Performance Regression Detection

- **Challenge**: Profiling GPU shader execution without impacting performance
  significantly
- **Solution**: Implemented timing-based profiling with baseline comparison and
  regression detection
- **Pattern**: Use `Instant::now()` for CPU timing with
  `device.poll(WaitForSubmissionIndex)` for synchronization
- **Performance**: Achieved <5% profiling overhead through efficient timing and
  minimal GPU state changes
- **Regression Detection**: Configurable threshold-based alerting with severity
  levels

#### Async GPU Operations and Futures Integration

- **Challenge**: GPU buffer mapping requires proper async handling with futures
- **Solution**: Use `futures_channel::oneshot::channel()` for async buffer
  mapping with proper polling
- **Pattern**: Always pair `buffer_slice.map_async()` with
  `device.poll(PollType::Wait)` for synchronization
- **Critical**: Handle mapping errors gracefully with descriptive error messages
- **Best Practice**: Unmap buffers immediately after data extraction to avoid
  resource leaks

#### Serialization and Export for Debug Data

- **Challenge**: Making GPU debug data accessible to external analysis tools
- **Solution**: Comprehensive serde support with JSON and CSV export
  capabilities
- **Pattern**: Use `#[derive(Serialize, Deserialize)]` on all debug data
  structures
- **Export Formats**: JSON for structured data, CSV for spreadsheet analysis
- **Performance**: Limit data exports (10K elements max) to prevent performance
  issues

**Architectural Decisions:**

#### Modular Debug System Design

- **Decision**: Separate `GpuBufferInspector`, `ShaderProfiler`, and
  `MemoryLayoutValidator` modules
- **Reasoning**: Each debug area has distinct concerns and can be used
  independently
- **Integration**: `GpuDebugContext` provides unified interface while
  maintaining modularity
- **Future**: Easy to extend with additional debug capabilities (texture
  inspection, pipeline analysis)

#### Staging Buffer Caching Strategy

- **Decision**: Cache staging buffers by size rather than creating new ones for
  each operation
- **Reasoning**: Buffer creation/destruction overhead significant for frequent
  debug operations
- **Trade-off**: Memory usage vs performance - configurable cache with manual
  clearing capability
- **Performance**: Dramatically reduces debug operation overhead for repeated
  buffer inspections

#### Compile-Time vs Runtime Debug Features

- **Decision**: Use `#[allow(dead_code)]` annotations for future timestamp query
  features
- **Reasoning**: WebGPU timestamp queries not universally supported yet, but
  infrastructure ready
- **Pattern**: Implement timing-based profiling now, upgrade to timestamp
  queries when available
- **Future-Proofing**: Debug infrastructure designed to support hardware
  timestamp queries

#### Performance Monitoring Integration

- **Decision**: Include performance baseline and regression detection in core
  debug tools
- **Reasoning**: Performance monitoring essential for GPU development, should be
  built-in
- **Implementation**: Configurable thresholds, multiple severity levels,
  historical tracking
- **Workflow**: Integrate with test infrastructure for automated performance
  regression detection

**Development Workflow Insights:**

#### Debug Tool Development Methodology

- **Step 1**: Implement basic functionality with comprehensive error handling
- **Step 2**: Add performance optimization (caching, efficient resource usage)
- **Step 3**: Integrate with existing systems (error types, test infrastructure)
- **Step 4**: Add export and analysis capabilities for external tool integration
- **Step 5**: Comprehensive testing and documentation with realistic examples

#### GPU Debug Tool Testing Strategy

- **Essential**: Run GPU debug tests with `--test-threads=1` to avoid resource
  conflicts
- **Pattern**: Test debug tools with realistic GPU resources (actual buffers,
  pipelines)
- **Validation**: Verify debug tools don't significantly impact application
  performance
- **Coverage**: Test both successful operations and error conditions (invalid
  buffers, etc.)

#### Memory Safety in Debug Tools

- **Critical**: Use `bytemuck::Pod + bytemuck::Zeroable` traits for all GPU data
  structures
- **Pattern**: Validate buffer sizes match expected element counts before
  casting
- **Error Handling**: Comprehensive bounds checking and descriptive error
  messages
- **Resource Management**: Proper buffer unmapping and cache cleanup for
  long-running debug sessions

#### Documentation and Usability for Debug Tools

- **Example Code**: Comprehensive example in `examples/gpu_debug_demo.rs`
  showing all features
- **Error Messages**: Context-rich error messages with specific guidance for
  common issues
- **API Design**: Simple function calls for common operations (`dump_buffer()`,
  `profile_compute()`)
- **Integration**: Clear integration patterns with existing error handling and
  test infrastructure

### GUP-017: Error Handling Framework (Completed 2025-08-07)

**Key Technical Learnings:**

#### Comprehensive Error Hierarchy Design with thiserror

- **Challenge**: Creating a robust error system that covers all possible failure
  modes in GPU graphics programming
- **Solution**: Implemented 25+ error types using thiserror with structured
  error context and rich diagnostic information
- **Pattern**: Use enum-based error hierarchy with `#[derive(Error)]` for
  compile-time safety and pattern matching
- **Critical**: Maintain backward compatibility with legacy error constructors
  while introducing new structured error types

#### Test-Driven Error Validation with Realistic Scenarios

- **Challenge**: Ensuring error handling logic works correctly under real-world
  failure conditions
- **Solution**: Created comprehensive test suite with 25 error handling tests
  covering all error scenarios
- **Pattern**: Use deterministic error injection (every Nth call) rather than
  random injection for reliable test results
- **Learning**: Memory pressure tests require accurate percentage calculations
  and proper threshold values (81.25% vs 93.75% usage)

#### Automatic Fallback Strategy Implementation

- **Challenge**: Graceful degradation when primary GPU operations fail
- **Solution**: Multi-tier fallback system (GPU→CPU, WebGPU→WebGL, quality
  reduction, complexity reduction)
- **Pattern**: Priority-based fallback strategies with performance impact
  tracking and success rate monitoring
- **Critical**: Platform-specific fallbacks need conditional compilation
  (`#[cfg(target_arch = "wasm32")]`) to avoid unreachable code

#### Resource Management Under Memory Pressure

- **Challenge**: Automatic cleanup and recovery from GPU memory exhaustion
- **Solution**: Emergency cleanup with 7 distinct strategies (evict unused,
  compact memory, reduce sizes, clear caches)
- **Pattern**: Age-based resource eviction with configurable thresholds (300s
  default) and priority-based cleanup ordering
- **Learning**: Test cleanup strategies with resources older than configured
  thresholds to ensure actual cleanup occurs

#### Error Context and Recovery Suggestion System

- **Challenge**: Providing actionable error information with automatic recovery
  guidance
- **Solution**: Rich error context with system diagnostics, recovery
  suggestions, and success probability estimates
- **Pattern**: Generate context-specific recovery suggestions based on error
  type and system state
- **Implementation**: Use error correlation IDs, timestamps, and serializable
  diagnostic data for external analysis

#### Chaos Engineering for Reliability Testing

- **Challenge**: Validating system stability under unpredictable error
  conditions
- **Solution**: Configurable error injection framework with deterministic
  injection rates for reproducible testing
- **Pattern**: `ChaosEngine` with controlled error injection (10% rate = every
  10th call) for predictable test behavior
- **Validation**: System maintains >85% success rate even with 10% error
  injection, demonstrating resilience

**Architectural Decisions:**

#### Modular Error Handling Architecture

- **Decision**: Separate modules for error_context, fallback, recovery,
  reporting, and resource management
- **Reasoning**: Each concern has distinct responsibilities and can be
  developed/tested independently
- **Trade-off**: Slightly more complex module structure but much better
  separation of concerns and maintainability
- **Future**: Easy to extend with additional error handling capabilities without
  modifying core error types

#### Async Recovery Operations

- **Decision**: Use async/await for all recovery and fallback operations
- **Reasoning**: Resource cleanup, memory compaction, and fallback
  initialization can be time-consuming
- **Pattern**: Non-blocking recovery operations that don't freeze the main
  application thread
- **Implementation**: Proper async error propagation with `GupResult<T>`
  throughout the async call chain

#### Configuration-Driven Error Handling

- **Decision**: Configurable thresholds, timeouts, and fallback strategies
  rather than hardcoded values
- **Reasoning**: Different applications have different resource constraints and
  performance requirements
- **Pattern**: Default configurations that work well out-of-box with
  customization options for advanced users
- **Examples**: Memory pressure thresholds, cleanup intervals, recovery attempt
  limits, rate limiting windows

#### Serializable Error Data for Analysis

- **Decision**: Full serde support for all error types and diagnostic
  information
- **Reasoning**: Enable external error analysis tools, telemetry systems, and
  debugging workflows
- **Pattern**: `#[derive(Serialize, Deserialize)]` on error types with JSON/CSV
  export capabilities
- **Benefit**: Error data can be exported for trend analysis, debugging, and
  system monitoring

**Development Workflow Insights:**

#### Test Suite Organization and Quality Gates

- **Essential**: Separate test files for different error handling concerns (unit
  tests, integration tests, chaos tests)
- **Pattern**: Use `cargo test -- --test-threads=1` for GPU-related tests to
  avoid resource conflicts
- **Quality Gates**: All tests must pass before considering error handling
  complete (246/246 tests passing)
- **Learning**: Memory pressure and resource cleanup tests require careful
  timing and threshold management

#### Error Handling Performance Validation

- **Target**: <5% performance overhead for error handling infrastructure
- **Measurement**: Comprehensive benchmarking of error context creation,
  fallback operations, and cleanup strategies
- **Optimization**: Use efficient data structures (HashMap caching) and avoid
  unnecessary allocations in error paths
- **Validation**: Error handling must not significantly impact normal operation
  performance

#### Backward Compatibility Strategy

- **Approach**: Maintain all legacy error constructors while introducing new
  structured error types
- **Pattern**: Provide both `GupError::render_error()` and
  `GupError::RenderError { message }` for migration flexibility
- **Testing**: Comprehensive backward compatibility tests ensure existing code
  continues to work
- **Migration Path**: Clear migration guidance from legacy to structured error
  types

#### Cross-Platform Error Handling Consistency

- **Challenge**: Ensuring identical error handling behavior on native and
  WebAssembly platforms
- **Solution**: Platform-specific fallback implementations with consistent API
  surface
- **Pattern**: Use conditional compilation for platform differences while
  maintaining unified error types
- **Testing**: Validate error handling behavior on both native and WASM targets

### GUP-018: Observable Plot-style Chart Builders (Completed 2025-08-08)

**Key Technical Learnings:**

#### Observable Plot API Design Patterns

- **Challenge**: Creating intuitive, type-safe Observable Plot-compatible API
  while maintaining GPU performance
- **Solution**: Fluent interface pattern with zero-cost abstractions over Phase
  1 Selection primitives
- **Pattern**:
  `plot().data(vec).scatter(x("field"), y("field")).color(color("category"))`
  compiles to efficient GPU operations
- **Critical**: Type-safe accessor functions with compile-time validation
  prevent runtime field access errors

#### Trait Bound Resolution for Multiple Mark Systems

- **Challenge**: Compilation errors due to conflicting Mark trait definitions
  (selection::Mark vs mark::Mark)
- **Solution**: Explicit trait qualification using `crate::selection::Mark`
  throughout chart builder system
- **Pattern**: Use fully qualified trait paths when multiple traits with same
  name exist in scope
- **Learning**: Rust's trait resolution requires explicit disambiguation in
  complex module hierarchies

#### Zero-Cost Abstraction Implementation

- **Challenge**: High-level Observable Plot API should not compromise GPU
  performance
- **Solution**: Chart builders compile directly to Selection operations with no
  runtime overhead
- **Pattern**: Generic builders with `build_with_data()` method that transforms
  to low-level GPU primitives
- **Validation**: Performance benchmarks show identical execution speed between
  direct Selection usage and chart builder API

#### Accessor Function Architecture

- **Challenge**: Type-safe field access with Observable Plot-style string-based
  field mapping
- **Solution**: `AccessorFunction<T>` wrapper with `AccessorValue` enum for
  runtime type safety
- **Pattern**: `x("field")` creates compile-time validated accessor function
  with runtime field extraction
- **Implementation**: Use `Box<dyn Fn(&T) -> AccessorValue + Send + Sync>` for
  flexible accessor storage

#### Comprehensive Test Strategy for Chart APIs

- **Challenge**: Testing fluent APIs requires validation of method chaining,
  data conversion, and error handling
- **Solution**: Multi-layered testing with unit tests, integration tests, and
  doctest validation
- **Pattern**: Test each builder individually, then test composition and
  integration with Selection system
- **Critical**: Run tests with `cargo test -- --test-threads=1` for GPU resource
  management

#### Documentation and API Usability

- **Challenge**: Observable Plot users expect extensive documentation with
  working examples
- **Solution**: Comprehensive doctests with `no_run` flag for GPU-dependent
  examples
- **Pattern**: Include both basic usage and advanced configuration examples in
  module documentation
- **Learning**: Use `use gup::prelude::*;` pattern to simplify imports for
  library users

**Architectural Decisions:**

#### Fluent Interface vs Builder Pattern

- **Decision**: Implement fluent interface with method chaining for Observable
  Plot compatibility
- **Reasoning**: Observable Plot users expect `chart.x().y().color()` chaining
  syntax
- **Trade-off**: Slightly more complex implementation but significantly better
  user experience
- **Implementation**: Each method returns `Self` with updated internal state

#### Generic vs Concrete Chart Builders

- **Decision**: Generic builders `ChartBuilder<T>` parameterized by data type
- **Reasoning**: Enables compile-time type safety and zero-cost abstractions
- **Pattern**: `ScatterPlotBuilder<DataPoint>` with accessor functions that
  operate on `&DataPoint`
- **Benefit**: Eliminates runtime type casting and provides excellent error
  messages

#### Accessor Function Flexibility

- **Decision**: Support both string-based field access and closure-based
  accessors
- **Reasoning**: String-based matches Observable Plot, closures provide maximum
  flexibility
- **Pattern**: `x("field")` for simple cases, `x(|d| d.complex_calculation())`
  for advanced usage
- **Implementation**: `Into<AccessorFunction<T>>` trait for seamless conversion

#### Error Handling Strategy

- **Decision**: Rich error types with context-specific validation messages
- **Reasoning**: Chart building can fail in many ways (missing data, invalid
  accessors, GPU issues)
- **Pattern**: `ChartBuilderError` enum with detailed error context and
  suggestions
- **Integration**: Seamless conversion to `GupError` for consistent library
  error handling

**Development Workflow Insights:**

#### Incremental Implementation Strategy

- **Approach**: Implement core infrastructure first, then individual chart
  builders
- **Order**: ChartBuilder trait → ScatterPlot → Line → Bar → Area → Heatmap
- **Validation**: Test each builder thoroughly before proceeding to next
  implementation
- **Learning**: Solid foundation enables rapid implementation of additional
  chart types

#### Observable Plot Compatibility Testing

- **Strategy**: Create examples that mirror Observable Plot documentation
  patterns
- **Validation**: Ensure API feels natural to users familiar with Observable
  Plot
- **Documentation**: Side-by-side comparisons with Observable Plot syntax where
  possible
- **Future**: Consider creating Observable Plot migration guide

#### Integration with Existing Selection System

- **Challenge**: Maintain backward compatibility while adding high-level APIs
- **Solution**: Chart builders compile to Selection instances for seamless
  interoperability
- **Pattern**: `into_selection()` method provides escape hatch to low-level APIs
- **Validation**: Existing Selection-based code continues to work unchanged

#### Quality Assurance Workflow

- **Essential**: `mask all-fix` catches formatting and linting issues before
  commit
- **Testing**: Comprehensive test suite with 253 unit tests + 11 integration
  test suites + 34 doctests
- **Documentation**: All public APIs have doctests with realistic examples
- **Performance**: Zero regression in GPU performance compared to direct
  Selection usage

For more detailed patterns, see:

- `docs/graphics-programming.md` - GPU-specific programming patterns
- `docs/patterns/` - Story-specific learnings and specialized patterns

### GUP-102: Demo GPU Resource Management Fixes (Completed 2025-02-06)

**Key Technical Learnings:**

#### Single Render Pass Strategy

- **Challenge**: GPU validation errors ("Encoder is invalid") during demo mode
  switching caused by improper render pass lifecycle management
- **Root Cause**: Multiple render passes created from the same command encoder
  in a single frame (one for background clear, another for data rendering)
- **Solution**: Consolidate all rendering into a single render pass that handles
  both background clearing and data visualization
- **Pattern**: Pass clear color to the renderer method instead of creating
  separate render passes

```rust
// ✅ Correct: Single render pass handles everything
fn render_with_clear(&mut self, frame: &mut RenderFrame, clear_color: Color) {
    let mut render_pass = frame.render_pass(Some(clear_color));
    // Render circles, text, etc. all in same pass
}

// ❌ Incorrect: Multiple render passes cause validation errors
fn render_frame(&mut self, frame: &mut RenderFrame) {
    { let _clear_pass = frame.render_pass(Some(clear_color)); }
    self.renderer.render(frame); // Creates ANOTHER render pass - BAD!
}
```

#### GPU Resource Lifecycle Management

- **Challenge**: Stale GPU buffer references when switching modes caused crashes
- **Solution**: Invalidate instance buffers on data changes while preserving
  static resources (vertex buffers, pipelines)
- **Pattern**: Set instance buffer to `None` when data changes; recreate on next
  render
- **Best Practice**: Separate static resources (pipelines, vertex buffers) from
  dynamic resources (instance buffers with per-frame data)

```rust
fn update_data(&mut self, circles: Vec<CircleAttributes>) {
    self.circle_instances = circles.into_iter().map(...).collect();
    // Only invalidate the dynamic buffer, not static resources
    self.instance_buffer = None;
    // Pipeline can be reused - no need to invalidate
}
```

#### Mode Switch Safety

- **Challenge**: Rapid mode switching (100+ consecutive switches) must not cause
  crashes
- **Solution**: Proper resource invalidation combined with single render pass
  strategy
- **Testing**: Added stability tests that cycle through 120 mode switches to
  validate robustness
- **Validation**: Instance buffer correctly invalidated after each mode switch

#### Demo Application Patterns

- **Pattern**: Initialize resources lazily on first render, not during mode
  switch
- **Pattern**: Check for empty data before rendering to avoid GPU operations
  with no work
- **Pattern**: Use distinct background colors per mode for clear visual feedback
- **Best Practice**: Reduce console output during normal rendering to avoid
  performance impact

**Architectural Decisions:**

#### Render Pass Consolidation

- **Decision**: Single render pass per frame for all rendering operations
- **Reasoning**: wgpu command encoders should not create multiple render passes
  for the same frame
- **Trade-off**: Slightly more complex render methods but eliminates validation
  errors
- **Implementation**: `render_with_clear()` method accepts clear color as
  parameter

#### Resource Caching Strategy

- **Decision**: Cache static resources (vertex buffers, pipelines) across mode
  switches
- **Reasoning**: Pipeline creation is expensive; reusing reduces mode switch
  latency
- **Pattern**: Only invalidate instance buffers when data changes
- **Performance**: Mode switches are now instant with no perceptible delay

**Development Workflow Insights:**

#### GPU Validation Error Debugging

- **Step 1**: Identify all render pass creation sites in the frame rendering
  path
- **Step 2**: Trace command encoder lifecycle from creation to finish
- **Step 3**: Consolidate multiple render passes into single pass
- **Step 4**: Test with rapid mode switching to validate stability

#### Stability Testing Methodology

- **Essential**: Test 100+ consecutive mode switches to validate resource
  management
- **Pattern**: Verify instance buffer invalidation after each mode switch
- **Validation**: All data point counts match expected values for each mode
- **Quality Gate**: Zero GPU validation errors during extended operation

#### Example Code Quality Standards

- **Documentation**: Clear comments explaining single render pass strategy
- **Testing**: Comprehensive tests covering mode switching, data correctness,
  and resource lifecycle
- **Error Handling**: Graceful degradation when rendering fails
- **User Experience**: Distinct visual feedback (colors, titles) for each mode
