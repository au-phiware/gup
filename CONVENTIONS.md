# Gup Development Conventions

This document outlines high-level conventions and project standards for the Gup
visualization library.

## Project Standards

### File Structure

- **`src/`** - Core library implementation
- **`examples/`** - Working examples and demonstrations
- **`docs/`** - Comprehensive documentation
- **`docs/patterns/`** - Specialized development patterns and story learnings
- **`benches/`** - Performance benchmarks

### Documentation Standards

All public APIs must include:

- Comprehensive doc comments with examples
- Runnable code examples that pass `cargo test --doc`
- Both simple and advanced usage patterns
- Clear error conditions and handling

### Code Quality

- **Zero warnings policy** - All code must compile without warnings
- **Comprehensive testing** - Unit tests, integration tests, and GPU-specific
  tests
- **Performance validation** - All features must meet performance targets
- **Cross-platform compatibility** - Support for native and WebAssembly targets

### Error Handling

- Use the `GupResult<T>` type for all fallible operations
- Provide context-rich error messages that aid debugging
- Include component descriptions in composition error messages
- Chain errors appropriately to preserve context

### API Design Principles

- **Composability** - All components should implement appropriate composition
  traits
- **Type Safety** - Use the type system to prevent runtime errors
- **Performance** - GPU-first design with zero-copy where possible
- **Ergonomics** - Provide both simple defaults and detailed configuration
  options

### Naming Conventions

- **Traits** - Descriptive names that avoid conflicts (e.g., `ShaderFunction`
  not `Function`)
- **Types** - Clear, domain-specific names that indicate purpose
- **Methods** - Follow Rust conventions with `snake_case`
- **Constants** - Use `SCREAMING_SNAKE_CASE` for compile-time constants

### Version Compatibility

- **WebGPU Standards** - Track evolving WebGPU specifications
- **wgpu Dependency** - Maintain compatibility with latest stable wgpu versions
- **Rust Edition** - Use latest stable Rust edition features

## Testing Standards

### GPU Testing Requirements

- Tests requiring GPU access must use `cargo test -- --test-threads=1`
- Comprehensive resource cleanup to prevent test interference
- Performance regression testing for all GPU operations
- Cross-platform validation on multiple backends

### Test Categories

1. **Unit Tests** - Individual component functionality
2. **Integration Tests** - Multi-component interactions
3. **Performance Tests** - Benchmark critical paths
4. **Example Tests** - Verify all examples compile and run
5. **Documentation Tests** - Ensure doc examples work

## Performance Standards

- **Real-time Performance** - Target 60+ FPS for interactive visualizations
- **Memory Efficiency** - Minimize GPU memory allocations and copies
- **Scalability** - Handle datasets from thousands to millions of points
- **Startup Time** - Fast initialization for responsive user experience

## Development Workflow

### Before Committing

1. Run `mask all-fix` to resolve lint and format issues
2. Ensure all tests pass with `cargo test -- --test-threads=1`
3. Verify examples compile and run correctly
4. Update documentation for API changes

### Story Development

- Follow story templates in `docs/planning/stories/`
- Include retrospectives with learnings and patterns
- Document performance measurements and benchmarks
- Identify follow-up stories for discovered work

### Story Implementation Learnings (from GUP-091)

#### Infrastructure vs Visual Implementation Separation

- **Infrastructure stories** should focus on core systems, data structures, and
  APIs
- **Visual implementation stories** should handle actual rendering pipeline
  integration
- This separation allows for iterative development and reduces scope creep
- Example: Grid system infrastructure (GUP-091) vs future visual rendering
  integration

#### Demo and Example Strategy

- **Console demos** are valuable for infrastructure validation and API
  demonstration
- **Visual demos** require full rendering pipeline integration - defer to
  follow-up stories
- Focus on comprehensive unit tests over complex visual examples during
  infrastructure phase
- Window framework demos can validate GPU context setup without full rendering

#### Performance Target Setting

- Set **architectural performance targets** early (e.g., <0.05ms for 20 grid
  lines)
- Design systems to **meet targets through efficient algorithms**, not just
  implementation
- Document **performance measurement approach** in acceptance criteria
- Use performance targets to **guide architectural decisions** during
  implementation

#### Testing Strategy for GPU Systems

- **Unit tests are critical** for infrastructure validation - aim for 15+
  comprehensive tests
- **GPU resource management** requires single-threaded testing
  (`--test-threads=1`)
- **Test data structures and algorithms** thoroughly before visual integration
- **Lint compliance** must be achieved before completion - use `mask all-fix`
  proactively

#### API Design for Complex Systems

- Use **builder patterns with fluent APIs** for complex configuration
  (GridCapableBuilder)
- Provide **sensible defaults** that work out-of-box with customization options
- **Static methods** can resolve borrowing issues in complex renderer
  architectures
- **Trait-based extension** allows adding capabilities to existing systems
  without breaking changes

## Documentation Structure

For detailed development patterns and examples, see:

- **[CLAUDE.md](CLAUDE.md)** - Development environment and general patterns
- **[docs/graphics-programming.md](docs/graphics-programming.md)** -
  GPU-specific programming patterns
- **[docs/patterns/](docs/patterns/)** - Story-specific learnings and
  specialized patterns
- **[README.md](README.md)** - Project overview and quick start

## Copyright and Licensing

All code files must include the short copyright notice header:

```rust
// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
```

This project is licensed under GPL-3.0-or-later. See [COPYING](COPYING) for full
license text.
