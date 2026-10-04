# GUP-389: Delete Unwired Dead Subsystems

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 🚧 In Progress **Created**:
2026-10-04

## Context

This story implements the "delete outright" half of **T1** from the
[October 2026 strategic review](../STRATEGIC_REVIEW_2026-10.md#t1--prune-and-public-surface-sm-low-risk),
per [Decision 1](../STRATEGIC_REVIEW_2026-10.md#decisions): "delete the unwired
subsystems outright (git history keeps them), **except** GPU culling/LOD, which
moves to an experimental crate" (covered separately by GUP-390). The review's
central finding is that ~40k LOC of the codebase is unwired: code that compiles
and has its own tests, but has no path from any chart builder or public API to a
user's screen.

Verified against `main` on 2026-10-04 (grep evidence; re-run before deleting,
since this is a one-time snapshot):

- **Shader transpiler** (`#[shader_fn]`, `src/shader_ast/`,
  `gup-macros/src/transpile/`): `attr_shader()` — the entry point that would
  make a `ShaderFn` usable by a chart — has **zero callers outside
  `src/selection.rs`** (`grep -rn attr_shader src/` excluding that file: empty).
  `grep -rln "shader_fn\]" src/ examples/ gup-bevy gup-egui gup-ios gup-android`
  (the macro's actual usage) returns no production call sites — the macro is
  exercised only by its own tests. `cargo check -p gup-ios` already emits
  `warning: function 'check_unreachable' is never used` from
  `gup-macros/src/transpile/validation.rs:213`, i.e. the transpiler's own
  dead-code is visible even in unrelated builds.
- **Mixable ecosystem** (`src/mixable.rs`, `src/mixable/`,
  `src/async_mixable.rs`, `src/async_mixable/`, `src/integration.rs`,
  `src/plugins.rs`, `RenderLayerManager` in `src/chart_builder.rs:3185`):
  `Mixable` is implemented and re-exported widely, but no chart builder in
  `src/chart_builder/builders/*.rs` uses it — the only production consumers are
  `src/examples.rs`/`src/examples/blend_modes.rs` (itself slated for deletion
  below) and the system's own files. `RenderLayerManager`
  (`src/chart_builder.rs:3185-3283`) has a `Default` impl and methods but no
  caller outside its own `impl` blocks.
- **`MarkRenderer` and friends** (`src/mark/renderer.rs`,
  `src/mark/batch_renderer.rs`, ~2,900 lines combined): chart builders build and
  render marks through `ComposedChart`/`Selection::prepare_render_bound`, not
  through `MarkRenderer`. Re-verify call sites with
  `grep -rn "MarkRenderer::\|BatchRenderer::" src/chart_builder/` before
  deleting (expected: empty, or test-only).
- **`plot_api` / `FieldAccessor` / `BoundChartBuilder`**
  (`src/chart_builder/plot_api.rs`, `src/chart_builder/accessor.rs`): this is
  the most subtly broken of the group, not merely unused. `FieldAccessor`
  (`accessor.rs:18`) is a string-wrapping struct with **no actual
  field-reflection logic**.
  `impl<T> From<FieldAccessor> for AccessorFunction<T>`
  (`src/chart_builder/builders.rs:574-577`) routes through
  `AccessorFunction::from_field()` (`src/chart_builder/builders.rs:530`), which
  **always returns `AccessorValue::Float(0.0)`** regardless of the named field —
  confirmed by reading its body. `ConfiguredChart::build()`
  (`src/chart_builder/plot_api.rs:373-378`) **always returns `Err`** ("Use async
  build pattern" — no async variant exists). `BoundChartBuilder`
  (`src/chart_builder.rs:317`) is exercised in
  `examples/observable_plot_showcase.rs:330` only as
  `let _bound_chart = BoundChartBuilder::new(...)` — constructed and immediately
  discarded, never rendered. All three are re-exported through `src/prelude.rs`
  today, meaning a user following the `x("field")` / `y("field")` style shown in
  the crate's own Observable-Plot-inspired examples gets silent zeros.
- **Duplicate scale/pipeline-cache systems**: `src/pipeline_cache.rs` and
  `src/chart_builder/pipeline_cache.rs` both define `PipelineCache` /
  `PipelineCacheStats`. Confirm which one GUP-239 (Pipeline Caching Chart
  Builder, ✅) actually wired up before deleting the other — do not delete both.
- **Error-recovery modules** (`src/error/recovery.rs`, `cache.rs`,
  `resource.rs`, `fallback.rs`, `reporting.rs`, `error_context.rs`,
  `lazy_context.rs` — 3,797 lines total):
  `grep -rn "error::recovery\|RecoveryStrategy\|ErrorRecovery" src/*.rs src/chart_builder/*.rs src/mark/*.rs`
  found only `src/mixable.rs` as a caller outside the `error` module itself —
  i.e. this is reachable only through the Mixable ecosystem also being deleted
  in this story.
- `src/examples.rs` and `src/examples/` (the in-crate `examples` module, not the
  top-level `examples/` directory) only demonstrates the Mixable composition API
  being deleted here.

## User Story

> "As a contributor reading the `gup` source for the first time, I want the
> crate to contain only code that is reachable from a public, working API, so
> that I don't waste time learning or building on a subsystem with zero
> production callers."

## Acceptance Criteria

### AC1: Each deletion is justified by a caller check

- [ ] For every module/type deleted, the story (or its retrospective) records
      the grep command used to confirm zero production callers, re-run
      immediately before deletion (the evidence above is a starting point, not a
      substitute — code may have changed).
- [ ] If a caller check finds an unexpected production caller for something on
      the delete list, that specific item is **not** deleted in this story; note
      it in the retrospective as a follow-up decision instead of silently
      keeping it half-migrated.

### AC2: The crate builds clean after deletion

- [ ] `cargo build` and `cargo check --examples` succeed with the deleted
      modules removed from `src/lib.rs`.
- [ ] No `#[allow(dead_code)]` is added to silence a warning about code that
      should instead have been deleted (if a warning appears, trace it to a real
      remaining caller or finish the deletion).

### AC3: Dependent examples and tests are updated, not left broken

- [ ] `examples/observable_plot_showcase.rs` (uses `BoundChartBuilder`,
      `plot_api`) is updated to drop the dead API usage or deleted if nothing
      else in it is worth keeping — confirm which at implementation time by
      reading the rest of the file.
- [ ] Any test in `src/mixable.rs`, `src/integration.rs`, `src/plugins.rs`,
      `src/chart_builder/plot_api.rs`, `src/mark/renderer.rs`,
      `src/mark/batch_renderer.rs`, `src/error/recovery.rs` and siblings is
      removed along with its module (not left dangling).
- [ ] `src/lib.rs`'s `pub use mixable::*;`, `pub use plugins::*;`,
      `pub use integration::*;`, `pub use examples::*;` glob re-exports are
      removed (this overlaps with GUP-391's prelude work — coordinate so GUP-391
      doesn't re-add them).

### AC4: Deleted subsystems are discoverable in history, not silently erased

- [ ] The commit message(s) list every top-level module/path deleted, so
      `git log --diff-filter=D -- src/<path>` finds this story's commit for
      anyone looking for the removed code later.

## Technical Tasks

- [ ] Re-run all grep caller-checks from the Context section against the current
      tree; update the list of files to delete if anything has changed since
      2026-10-04.
- [ ] Delete `src/shader_ast/`, `gup-macros/src/transpile/`, and the
      `#[shader_fn]` macro entry point in `gup-macros` — confirm
      `src/shader_pipeline.rs`'s reference to `shader_ast` (found via grep) is
      either also dead or is updated to not depend on the deleted module.
- [ ] Delete `src/mixable.rs`, `src/mixable/`, `src/async_mixable.rs`,
      `src/async_mixable/`, `src/integration.rs`, `src/plugins.rs`, and the
      `RenderLayerManager` struct/impls in `src/chart_builder.rs:3185-3283+`.
- [ ] Delete `src/mark/renderer.rs` and `src/mark/batch_renderer.rs` after
      confirming zero chart-builder callers.
- [ ] Delete `src/chart_builder/plot_api.rs`, the `FieldAccessor` type and its
      free functions in `src/chart_builder/accessor.rs`, the
      `From<FieldAccessor>` impl and `AccessorFunction::from_field` in
      `src/chart_builder/builders.rs`, and `BoundChartBuilder` in
      `src/chart_builder.rs`.
- [ ] Determine which of `src/pipeline_cache.rs` /
      `src/chart_builder/pipeline_cache.rs` is live (cross-reference GUP-239)
      and delete the other.
- [ ] Delete `src/error/recovery.rs`, `cache.rs`, `resource.rs`, `fallback.rs`,
      `reporting.rs`, `error_context.rs`, `lazy_context.rs` (re-verify each is
      only reachable via Mixable before deleting all seven — some may have
      independent callers not caught by the Context-section grep).
- [ ] Delete `src/examples.rs` and `src/examples/`.
- [ ] Update `src/lib.rs` to remove `pub mod` / `pub use` lines for everything
      deleted.
- [ ] Update or delete `examples/observable_plot_showcase.rs`.
- [ ] Run `cargo build`, `cargo check --examples`,
      `cargo test -- --test-threads=1` and fix any fallout.

## Dependencies

### Prerequisite Stories

- None — this is additive pruning with no code dependency on other wave-1
  stories, though it should land before GUP-391 (prelude purge) since that story
  removes the glob re-exports this one also touches.

### Enables Stories

- **Prerequisite for [RFC-001](../rfcs/RFC-001_Core_Architecture.md) step S0**
  (accepted 2026-10-04): the new `crates/gup-core` work starts cleaner with the
  old crate's unwired ~37k LOC already gone rather than needing to be
  ignored/worked around during the new core's build-out.
- GUP-391 (Curated prelude and public surface purge) is now ⏸ Parked
  (superseded by RFC-001 S7/S14) — this story's deletions are no longer a
  prerequisite for it, but remain independently valuable pruning work either
  way.

## Testing Strategy

- **Build verification**: `cargo build --workspace`, `cargo check --examples`,
  `cargo test -- --test-threads=1` all succeed after deletion.
- **Regression**: the T0 visual regression harness (GUP-388, if landed first) or
  the existing chart-builder tests must still pass — deleting dead code should
  have zero effect on any chart's rendered output, since by definition nothing
  reachable depended on it.
- **Caller re-verification**: re-run every grep from the Context section
  immediately before and after the deletion commit, diffing the results to
  confirm no unexpected caller was missed.

## Success Metrics

- [ ] At least ~37k LOC removed from `src/` and `gup-macros/src/` (transpiler
      ~18.7k + Mixable ecosystem ~8.4k + MarkRenderer ~6.4k + duplicate pipeline
      cache + error recovery ~3.5k, per the review's estimates — actual numbers
      may differ; report the real `git diff --stat` total in the retrospective).
- [ ] `cargo build` and `cargo test -- --test-threads=1` pass with zero new
      warnings introduced by the deletion.
- [ ] Zero remaining references to any deleted module/type outside
      `git log`/history.

## Risk Assessment

- **Medium**: This is a large, mechanical deletion across many files; the
  primary risk is an incomplete caller check missing a real (if obscure)
  production dependency, causing a build break discovered late. Mitigation:
  delete in the dependency order listed in Technical Tasks (leaf modules first),
  running `cargo build` after each group, not as one giant commit.
- **Low**: Deleting `plot_api`/`FieldAccessor` removes a documented (if broken)
  part of the public API (`x("field")`, `y("field")` style). Per
  [Decision 2](../STRATEGIC_REVIEW_2026-10.md#decisions) ("Breaking API reset:
  approved. No deprecation period"), this is an accepted, intentional breaking
  change, not an oversight — call it out explicitly in release notes once those
  exist.
- **Low**: `src/shader_pipeline.rs` has a real, if currently production-unused,
  reference to `shader_ast`. If investigation at implementation time finds
  `shader_pipeline.rs` itself has production callers (some `mark/*.rs` files
  reference `shader_pipeline::` for basic types), keep `shader_pipeline.rs` but
  delete only `shader_ast` and the parts of `shader_pipeline.rs` that depend on
  it — do not assume the two always travel together without checking.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document, including the actual LOC removed
      (`git diff --stat`) and any item on the proposed delete list that was kept
      because the caller check found a real dependency.
