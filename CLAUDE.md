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

## Story Retrospectives (Quick Reference)

Detailed retrospectives are appended to each story document in
`docs/planning/stories/`. Key cross-cutting learnings:

| Story   | Topic                    | Key Takeaway                                                            |
| ------- | ------------------------ | ----------------------------------------------------------------------- |
| GUP-011 | Mark-Shader Integration  | String-based WGSL injection; pipeline caching with hash keys            |
| GUP-012 | GPU Interaction System   | Compute shaders for hit testing; `--test-threads=1` for GPU tests       |
| GUP-013 | GPU Position Precision   | Rust↔WGSL struct alignment; `std::mem::offset_of!()` validation        |
| GUP-014 | Interaction Performance  | Workgroup size 256; grid spatial indexing; batch/stream query APIs      |
| GUP-015 | GPU Debugging Tools      | Staging buffer caching; memory layout validator; <5% profiling overhead |
| GUP-017 | Error Handling Framework | 25+ thiserror types; multi-tier fallback; chaos engineering testing     |
| GUP-018 | Chart Builders           | Fluent API; zero-cost abstraction over Selection; generic builders      |
| GUP-102 | Demo GPU Resource Mgmt   | Single render pass per frame; separate static vs dynamic resources      |

### Recurring Patterns

- **GPU tests**: Always `cargo test -- --test-threads=1` (parallel GPU tests
  segfault from resource contention, not code bugs).
- **WGSL alignment**: `vec2<f32>` needs 8-byte alignment; use `#[repr(C)]` +
  `bytemuck::Pod` + explicit padding.
- **Quality gate**: Run `mask all-fix` before every commit.
- **Single render pass**: Never create multiple render passes from one command
  encoder.
- **Enum over trait objects**: Prefer enums for known variant sets (object
  safety limitations).

For detailed learnings, see the Retrospective section in each story document.
