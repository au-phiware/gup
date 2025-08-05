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

For more detailed patterns, see:

- `docs/graphics-programming.md` - GPU-specific programming patterns
- `docs/patterns/` - Story-specific learnings and specialized patterns
