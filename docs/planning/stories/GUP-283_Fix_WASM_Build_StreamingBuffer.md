# GUP-283: Fix WASM Build (`StreamingBuffer` Send/Sync)

## Story Overview

**Initiative**: Core Infrastructure **Status**: 📋 Planned **Created**:
2025-07-18

## Context

The `wasm-pack build --target web` and `wasm-pack test --headless --chrome`
commands fail because `StreamingBuffer<T>` (in
`src/streaming/streaming_buffer.rs`) casts to `Box<dyn Any + Send + Sync>`, but
the underlying wgpu `Buffer` type is `!Send` on WASM targets. This blocks all
WASM packaging and browser-based integration tests.

Discovered during GUP-264 (Tauri Integration) where the WASM build was attempted
to verify the new `render_scatter` entry point.

## User Story

> "As a Gup developer, I want `wasm-pack build` and `wasm-pack test` to succeed
> so that WASM packaging, browser benchmarks, and integration tests work in CI."

## Acceptance Criteria

- [ ] `wasm-pack build --target web` succeeds.
- [ ] `wasm-pack test --headless --chrome` succeeds (or is blocked only by
      browser/WebGPU availability, not compilation errors).
- [ ] All native tests (`cargo test -- --test-threads=1`) continue to pass.
- [ ] No loss of functionality in the streaming module on native targets.

## Technical Tasks

- [ ] Audit `StreamingBuffer` and `DataStream` for `Send + Sync` requirements.
- [ ] Use `#[cfg(target_arch = "wasm32")]` to relax bounds on WASM or use
      `MaybeSend`/`MaybeSync` traits already defined in the crate.
- [ ] Verify no other WASM compilation errors exist.

## Dependencies

### Prerequisite Stories

- None (standalone fix).

### Enables Stories

- GUP-264 ✅ (WASM build validation)
- Any future WASM-dependent stories.

## Testing Strategy

- `wasm-pack build --target web` compiles cleanly.
- `cargo test -- --test-threads=1` on native shows no regressions.

## Risk Assessment

- **Low**: The fix is likely a straightforward conditional compilation change.

## Definition of Done

- [ ] `wasm-pack build --target web` succeeds.
- [ ] All native tests pass.
- [ ] `mask all-fix` exits cleanly.
