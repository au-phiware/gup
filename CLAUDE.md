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

## Sharing build output between checkouts

The main checkout, agent worktrees and the pre-commit hook's snapshot can share
one build directory (GUP-411):

- `.cargo/config.toml` gives every checkout its own workspace-member artifacts
  and leaves dependencies shared. It needs no setup. Without it, cargo names
  members' artifacts the same in every checkout and judges freshness by mtime,
  so a checkout could lint, test or run another checkout's code.
- Share the build directory, not the target directory. Run this in the checkout
  (the dev shell runs it on entry, except in CI or over an explicit
  `CARGO_TARGET_DIR`):

  ```bash
  eval "$(scripts/cargo_env.sh [BUILD_DIR])"
  ```

  It exports `CARGO_BUILD_BUILD_DIR=BUILD_DIR` (default `~/.cache/gup/build`),
  shared, and `CARGO_TARGET_DIR=BUILD_DIR/checkouts/<checkout>-<hash>`, this
  checkout's own. The target directory holds the unhashed final artifacts (the
  binary `cargo run` executes, examples, docs, `.wasm`, visual-regression
  output), which a shared target directory lets another checkout overwrite. On
  the same filesystem they are hard links, so they cost almost no disk.

- **Worktree agents**: a shell inherits the environment of the checkout that
  started it. Run the line above from the worktree in every shell, with the
  orchestrator's build directory, e.g.
  `cd <worktree> && eval "$(scripts/cargo_env.sh /tmp/gup-target)"`.
- **Concurrency**: safe. Cargo locks the build directory while it compiles, so
  builds from different checkouts take turns; tests and `cargo run` programs run
  after the lock is released. No checkout uses another's member artifacts.
- **Cost** (measured 2026-10-10): a cold `mask all-check` fills 3.3 GB of build
  directory in about 4.5 minutes. `mask all-check` in a second checkout added
  2.1 GB, in a third 0.4 GB; building gup-core's tests adds 1 GB per checkout.
  Clippy artifacts are shared, so a checkout re-lints when another checkout
  linted last: `mask all-check` then takes about 90 s instead of 26 s.
- Never `cargo clean` a shared build directory; delete your own
  `CARGO_TARGET_DIR` instead.
- A commit older than GUP-411 has no `.cargo/config.toml`. In a worktree nested
  inside a checkout that has one, cargo uses the outer checkout's and the build
  stops with an explanation. Elsewhere, give it its own build directory.

## Dependency Management

- Do not downgrade wgpu. The project relies on features of the latest version
  (v26).

## Development Patterns

See `CLAUDE.local.md` for coding guidelines (copyright headers, lint commands).
See `.github/agents/story-worker.md` for Rust design patterns, API patterns,
error handling, and recurring GPU/WGSL learnings used when implementing stories.
