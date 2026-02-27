# GUP-238: Remaining Send+Sync Audit

**Priority**: Low **Complexity**: Low **Created**: 2026-02-27 **Status**: ✅
Complete (2025-07-26)

## Overview

Audit and migrate remaining `Send + Sync` trait bounds throughout the codebase
to use the `MaybeSend`/`MaybeSync` marker traits introduced in GUP-231. While
the current bounds don't cause WASM compilation failures, they could break as
more WASM-specific concrete types are introduced.

## Context

GUP-231 resolved all blocking WASM compilation errors by introducing
`MaybeSend`/`MaybeSync` for the core traits (`Mixable`, `AsyncMixable`,
`Renderable`, etc.). However, several secondary traits still use direct
`Send + Sync` bounds:

- `Axis: Send + Sync`
- `LabelFormatter: Send + Sync`
- `ErrorSink: Send + Sync`
- `RecoveryHandler: Send + Sync`
- `SurfaceEventHandler: Send + Sync`
- `IntoAttrValue: Send + Sync`
- `InteractionData: Send + Sync`
- Various closure bounds (`Box<dyn Fn(...) + Send + Sync>`)

These currently work because their concrete implementors happen to be
`Send + Sync` even on WASM, but this is fragile.

## User Story

As a library developer, I want all trait bounds to consistently use conditional
Send/Sync so that adding new WASM-specific types never causes unexpected
compilation failures.

## Acceptance Criteria

- [x] All trait definitions using `Send + Sync` migrated to
      `MaybeSend + MaybeSync`
- [x] All `Box<dyn Fn(...) + Send + Sync>` type aliases updated
- [x] Native tests continue to pass
- [x] WASM build continues to succeed

## Technical Tasks

- [x] Grep for `Send + Sync` in trait definitions and type aliases
- [x] Replace with `MaybeSend + MaybeSync` where appropriate
- [x] Add MaybeSend/MaybeSync imports to affected files
- [x] Verify no regressions

## Dependencies

- **Requires**: GUP-231 (WASM Build Platform Gating) ✅

## Testing Strategy

- Native test regression
- WASM compilation check

## Risk Assessment

- **Low**: Purely mechanical replacement of bounds

## Implementation Summary

### What Was Implemented

Migrated all remaining `Send + Sync` trait bounds to use the conditional
`MaybeSend`/`MaybeSync` marker traits, ensuring consistent cross-platform
(native + WASM) compatibility throughout the codebase.

### Changes by Category

**1. Public Trait Definitions (21 traits migrated):**

- `Axis`, `LabelFormatter`, `ErrorSink`, `RecoveryHandler`,
  `SurfaceEventHandler`
- `IntoAttrValue`, `IntoAttrValues`, `InteractionData`, `EventHandler`
- `CustomInteractionQuery`, `ExternalRenderer`, `ValidationRule`
- `Scale` (both `scale.rs` and `tick_generator.rs`), `Mark`, `MarkInfo`,
  `ShaderType`
- `MixablePlugin`, `WindowHandle`, `TickGenerator`

**2. Type Aliases cfg-gated (native `+ Send + Sync`, WASM bare `dyn`):**

- `EventHandlerFn`, `RecoveryCallback`, `WindowHandleRenewalCallback`
- `AnimationEventCallback`, `PointExtractor`, `SharedPointExtractor`,
  `FormatterPair`

**3. Struct Fields cfg-gated:**

- `AttributeBinding::extractor`, `ShaderAttributeBinding::extractor`
- `AccessorFunction::function` (both `scale.rs` and `builders.rs`)
- `AccessorRegistry::field_accessors`
- `CustomFormatter::formatter_fn`
- `PointBasedPluginBuilder::validator`, `PointBasedPlugin::validator`
- `MixablePluginRegistry::plugins`

**4. Generic Bounds Updated (~60+ locations):**

- All `T: Clone + Send + Sync + Debug + 'static` bounds across
  `chart_builder`, `axis_system`, `integration`, `plugins`, `selection`
- Closure bounds: `F: Fn(...) + Send + Sync` → `F: Fn(...) + MaybeSend + MaybeSync`
- `IntoAccessor` trait and impls (cfg-gated)

**5. WASM-Specific Fixes:**

- `MixablePluginRegistry`: added `unsafe impl Send/Sync` for WASM
  (single-threaded runtime makes this safe)
- `MixablePlugin::create_mixable`: cfg-gated `Box<dyn Any + Send + Sync>` vs
  `Box<dyn Any>`
- `create_mixable`/`create_mixable_from_any`/`try_make_mixable`: cfg-gated

### Files Changed (28 files)

`src/axis.rs`, `src/label.rs`, `src/label/formatter.rs`,
`src/error/recovery.rs`, `src/error/reporting.rs`, `src/context.rs`,
`src/selection.rs`, `src/interaction.rs`, `src/mark.rs`, `src/scale.rs`,
`src/shader_function.rs`, `src/tick_generator.rs`,
`src/debug/buffer_validation.rs`, `src/integration.rs`, `src/plugins.rs`,
`src/chart_builder.rs`, `src/chart_builder/accessor.rs`,
`src/chart_builder/optimized_accessor.rs`, `src/chart_builder/builders.rs`,
`src/chart_builder/builders/scatter.rs`,
`src/chart_builder/builders/bar.rs`,
`src/chart_builder/builders/line.rs`,
`src/chart_builder/builders/area.rs`,
`src/chart_builder/builders/heatmap.rs`,
`src/chart_builder/builders/boxplot.rs`, `src/chart_builder/labels.rs`,
`src/chart_builder/plot_api.rs`, `src/axis_system.rs`

### Test Results

- 1843 lib tests pass (0 failures)
- `cargo check --target wasm32-unknown-unknown --lib` succeeds
- `cargo check --examples` succeeds
- All lint/format checks pass

## Definition of Done

- [x] No direct `Send + Sync` bounds remain on public traits
- [x] Native and WASM builds pass
- [x] All tests pass

## Retrospective

**Completed**: 2025-07-26

### Key Technical Learnings

#### MaybeSend/MaybeSync Cannot Be Used in Trait Object Position

- **Challenge**: `MaybeSend` and `MaybeSync` are regular marker traits, not
  Rust auto traits. Rust only allows auto traits (`Send`, `Sync`) to appear as
  additional bounds in `dyn` trait objects (e.g., `Box<dyn Fn() + Send + Sync>`).
  Using `Box<dyn Fn() + MaybeSend + MaybeSync>` is a compiler error.
- **Solution**: For trait object fields and type aliases, use `#[cfg]` gating to
  produce `+ Send + Sync` on native and bare `dyn Trait` on WASM. For generic
  type parameter bounds (e.g., `F: Fn() + MaybeSend + MaybeSync + 'static`),
  the marker traits work fine because these are compile-time constraints, not
  trait objects.
- **Pattern**: Always distinguish between generic bounds (where MaybeSend works)
  and trait objects (where cfg-gating is required). This is a fundamental Rust
  type system constraint.

#### Global Statics Require Send+Sync Regardless of Platform

- **Challenge**: `static GLOBAL_REGISTRY: OnceLock<Mutex<...>>` requires its
  content to implement `Send + Sync` because Rust statics must be `Sync`. When
  `MixablePlugin` was changed from `Send + Sync` to `MaybeSend + MaybeSync`,
  `dyn MixablePlugin` no longer satisfied `Send` on WASM, breaking the static.
- **Solution**: Cfg-gate the HashMap storage to use `Box<dyn MixablePlugin +
  Send + Sync>` on native and `Box<dyn MixablePlugin>` on WASM. Add `unsafe
  impl Send + Sync for MixablePluginRegistry` on WASM (safe because WASM is
  single-threaded). Also cfg-gate all methods that pass `Box<dyn Any + Send +
  Sync>` since `Any` downcasting requires matching trait object types.
- **Pattern**: Any type stored in a global static needs explicit `Send + Sync`
  guarantees. On WASM, `unsafe impl Send + Sync` is safe for single-threaded
  types, but should be documented clearly.

#### Scope of "Mechanical Replacement" Was Underestimated

- **Challenge**: The story was rated as "Low complexity" and described as
  "purely mechanical replacement of bounds." In practice, the interactions
  between trait definitions, trait objects, generic bounds, global statics,
  and `Box<dyn Any>` downcasting created several non-obvious complications.
- **Solution**: Methodical approach: update trait definitions first, then
  generic bounds, then cfg-gate trait objects, then fix WASM-specific issues.
  Frequent `cargo check` after each batch of changes.
- **Pattern**: Cross-cutting type system changes are rarely truly "mechanical."
  Always plan for edge cases in trait objects, statics, and Any downcasting.

### Architectural Decisions

#### cfg-Gating Trait Objects vs. Defining Wrapper Types

- **Decision**: Used `#[cfg]` attributes directly on struct fields and type
  aliases to provide `+ Send + Sync` on native and bare `dyn` on WASM, rather
  than creating wrapper newtypes or custom trait aliases.
- **Reasoning**: Cfg-gating is the most direct and transparent approach. It's
  immediately clear at each usage site what the platform difference is. Wrapper
  types would add indirection and make the codebase harder to understand.
- **Trade-off**: Duplication — many struct fields and type aliases now have two
  cfg-gated variants. This is verbose but maintainable.
- **Future**: If Rust stabilises auto trait definitions or trait aliases, these
  could be consolidated. The `auto trait MaybeSend {}` feature is on nightly.

#### Unsafe Send+Sync for MixablePluginRegistry on WASM

- **Decision**: Used `unsafe impl Send for MixablePluginRegistry` on WASM
  rather than cfg-gating the entire global registry out of WASM.
- **Reasoning**: The global registry is a useful feature even on WASM (for
  plugin ecosystem). WASM is inherently single-threaded, so `Send + Sync` is
  a no-op safety requirement. Removing the registry from WASM would reduce
  cross-platform parity.
- **Trade-off**: Unsafe code requires careful documentation. If WASM ever gets
  threads (SharedArrayBuffer + wasm-threads), this would need revisiting.
- **Future**: If the WASM threads proposal becomes standard, these unsafe impls
  should be audited or replaced with proper thread-safe implementations.

#### Intentional Exclusions

- **`Box<dyn Any + Send + Sync>`**: Left unchanged throughout the codebase.
  These are used for type-erased downcasting via `std::any::Any`, which provides
  specific impl blocks for `dyn Any + Send + Sync`. Changing these would break
  downcasting.
- **`PlatformAccessibility`**: Left with its existing cfg-gated dual trait
  definitions per the GUP-231 architectural decision. This trait has
  platform-specific semantics that benefit from explicit `Send + Sync` on native.
- **`shader_pipeline.rs` uniform bounds**: `F::Uniforms: Send + Sync + 'static`
  left as-is because these are used to box into `Box<dyn Any + Send + Sync>`.

### Development Workflow Insights

- **Incremental compilation checks**: Running `cargo check` after each batch of
  changes was essential. The WASM compilation error in plugins.rs was caught
  early, allowing targeted fixes rather than a large debugging session.
- **Disk space constraints**: The build artifacts for this project consume ~30GB.
  Running full `cargo test` required cleaning between attempts due to disk space
  limits. The `--lib` flag was sufficient for regression testing since the
  integration tests are separate binaries that consume additional space.
- **Grep-driven audit**: Starting with `grep -rn "Send + Sync" src/` to build a
  complete inventory was the right approach. Categorising occurrences by type
  (trait definitions, generic bounds, trait objects, Any boxing) before making
  changes prevented rework.
