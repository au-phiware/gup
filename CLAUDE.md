# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Environment

This project uses **Nix flakes** for reproducible development environments. The `flake.nix` provides:

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
- `cargo build` - Build all workspace projects
- `cargo check` - Check all projects without building
- `cargo run --bin hello-wgpu` - Run the hello-wgpu project
- `cargo clean` - Clean build artifacts

### Testing and Quality
- `cargo test` - Run tests for all projects
- `cargo clippy --all-targets --all-features -- -D warnings` - Run linter
- `cargo fmt --all` - Format all code
- `cargo fmt --all -- --check` - Check if code is formatted

### Development Workflow
- `cargo watch -x check -x test -x "clippy --all-targets --all-features -- -D warnings"` - Watch for changes and rebuild
- `cargo audit` - Check dependencies for security vulnerabilities
- `cargo update` - Update dependencies

### WebAssembly
- `wasm-pack build hello-wgpu` - Build the project for WebAssembly

## WebGPU Development Workflow

### Browser Setup
- Use `chromium-webgpu` command (provided by flake) which launches Chromium with WebGPU flags
- Required flags: `--enable-features=WebGPU,Vulkan --enable-unsafe-webgpu --disable-dawn-features=disallow_unsafe_apis`
- Test at chrome://gpu to verify WebGPU is enabled

### Development Commands
- `mask start` - Start development server with auto-rebuild, serve, and browser launch
- `mask pack` - Build WebAssembly package
- `mask serve` - Serve the application locally
- Uses `mprocs` to run multiple processes concurrently

### Cross-Platform Considerations
- **Storage Buffers vs Textures**: Use storage textures for better WebGPU compatibility
- **Backend Selection**: Use `wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL` for web
- **Features**: Add web-sys features like "Location" for browser-specific functionality

## wgpu Surface Lifetime Management

### The Arc<Window> Solution
When working with wgpu surfaces, use `Arc<Window>` to solve lifetime issues:

```rust
// ✅ Correct approach
let window = Arc::new(event_loop.create_window(attributes)?);
let surface = instance.create_surface(window.clone())?; // Creates Surface<'static>

// ❌ Problematic approach
let surface = instance.create_surface(&window)?; // Creates Surface<'window>
```

### Why Arc<Window> Works
- `Arc<Window>` has `'static` lifetime (owned, not borrowed)
- `Surface<'static>` can be stored in structs without lifetime parameters
- Multiple components can share ownership of the window
- The surface creation takes ownership of an Arc clone, not a borrow

## Quick Tips and Reminders

- Remember to check the maskfile.md for common tasks like build, run, serve, etc.

## Dependency Management

- Do not downgrade wgpu. The project relies on features of the latest version (v26).
