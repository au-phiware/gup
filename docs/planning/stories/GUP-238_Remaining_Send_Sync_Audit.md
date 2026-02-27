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
