# GUP-389: Delete Unwired Dead Subsystems

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: ✅ Complete (2026-10-05)
**Created**: 2026-10-04

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

- [x] For every module/type deleted, the story (or its retrospective) records
      the grep command used to confirm zero production callers, re-run
      immediately before deletion (the evidence above is a starting point, not a
      substitute — code may have changed).
- [x] If a caller check finds an unexpected production caller for something on
      the delete list, that specific item is **not** deleted in this story; note
      it in the retrospective as a follow-up decision instead of silently
      keeping it half-migrated.

### AC2: The crate builds clean after deletion

- [x] `cargo build` and `cargo check --examples` succeed with the deleted
      modules removed from `src/lib.rs`.
- [x] No `#[allow(dead_code)]` is added to silence a warning about code that
      should instead have been deleted (if a warning appears, trace it to a real
      remaining caller or finish the deletion).

### AC3: Dependent examples and tests are updated, not left broken

- [x] `examples/observable_plot_showcase.rs` (uses `BoundChartBuilder`,
      `plot_api`) is updated to drop the dead API usage or deleted if nothing
      else in it is worth keeping — confirm which at implementation time by
      reading the rest of the file.
- [x] Any test in `src/mixable.rs`, `src/integration.rs`, `src/plugins.rs`,
      `src/chart_builder/plot_api.rs`, `src/mark/renderer.rs`,
      `src/mark/batch_renderer.rs`, `src/error/recovery.rs` and siblings is
      removed along with its module (not left dangling).
- [x] `src/lib.rs`'s `pub use mixable::*;`, `pub use plugins::*;`,
      `pub use integration::*;`, `pub use examples::*;` glob re-exports are
      removed (this overlaps with GUP-391's prelude work — coordinate so GUP-391
      doesn't re-add them).

### AC4: Deleted subsystems are discoverable in history, not silently erased

- [x] The commit message(s) list every top-level module/path deleted, so
      `git log --diff-filter=D -- src/<path>` finds this story's commit for
      anyone looking for the removed code later.

## Technical Tasks

- [x] Re-run all grep caller-checks from the Context section against the current
      tree; update the list of files to delete if anything has changed since
      2026-10-04.
- [x] Delete `src/shader_ast/`, `gup-macros/src/transpile/`, and the
      `#[shader_fn]` macro entry point in `gup-macros` — confirm
      `src/shader_pipeline.rs`'s reference to `shader_ast` (found via grep) is
      either also dead or is updated to not depend on the deleted module.
- [x] Delete `src/mixable.rs`, `src/mixable/`, `src/async_mixable.rs`,
      `src/async_mixable/`, `src/integration.rs`, `src/plugins.rs`, and the
      `RenderLayerManager` struct/impls in `src/chart_builder.rs:3185-3283+`.
- [x] Delete `src/mark/renderer.rs` and `src/mark/batch_renderer.rs` after
      confirming zero chart-builder callers. (`renderer.rs` deleted;
      `batch_renderer.rs` kept: live callers in the culling/LOD code that
      GUP-390 moves, see Retrospective.)
- [x] Delete `src/chart_builder/plot_api.rs`, the `FieldAccessor` type and its
      free functions in `src/chart_builder/accessor.rs`, the
      `From<FieldAccessor>` impl and `AccessorFunction::from_field` in
      `src/chart_builder/builders.rs`, and `BoundChartBuilder` in
      `src/chart_builder.rs`.
- [x] Determine which of `src/pipeline_cache.rs` /
      `src/chart_builder/pipeline_cache.rs` is live (cross-reference GUP-239)
      and delete the other.
- [x] Delete `src/error/recovery.rs`, `cache.rs`, `resource.rs`, `fallback.rs`,
      `reporting.rs`, `error_context.rs`, `lazy_context.rs` (re-verify each is
      only reachable via Mixable before deleting all seven — some may have
      independent callers not caught by the Context-section grep).
- [x] Delete `src/examples.rs` and `src/examples/`.
- [x] Update `src/lib.rs` to remove `pub mod` / `pub use` lines for everything
      deleted.
- [x] Update or delete `examples/observable_plot_showcase.rs`.
- [x] Run `cargo build`, `cargo check --examples`,
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

- [x] At least ~37k LOC removed from `src/` and `gup-macros/src/` (transpiler
      ~18.7k + Mixable ecosystem ~8.4k + MarkRenderer ~6.4k + duplicate pipeline
      cache + error recovery ~3.5k, per the review's estimates — actual numbers
      may differ; report the real `git diff --stat` total in the retrospective).
- [x] `cargo build` and `cargo test -- --test-threads=1` pass with zero new
      warnings introduced by the deletion.
- [x] Zero remaining references to any deleted module/type outside
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

- [x] All Acceptance Criteria are satisfied and checked.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] Lint and format clean: `mask all-fix`.
- [x] All examples compile: `cargo check --examples`.
- [x] Story status updated to ✅ Complete in story file and INDEX.md.
- [x] Retrospective added to story document, including the actual LOC removed
      (`git diff --stat`) and any item on the proposed delete list that was kept
      because the caller check found a real dependency.

## Implementation Summary

Deleted in dependency order, one commit per subsystem. Every commit message
records the caller-check grep, re-run immediately before that deletion, and
lists each deleted path, so `git log --diff-filter=D -- <path>` finds it.

| Commit    | Subsystem                                                       | All files (+/−) |
| --------- | --------------------------------------------------------------- | --------------- |
| `da10cf4` | Rust-to-WGSL transpiler, `#[shader_fn]`, `src/shader_ast/`      | +7 / −21,821    |
| `6829d60` | `plot_api`, `FieldAccessor`, `BoundChartBuilder`                | +43 / −2,126    |
| `2b51a3e` | Duplicate `chart_builder::PipelineCache` cluster                | +0 / −1,317     |
| `80485b8` | `MarkRenderer`                                                  | +33 / −2,728    |
| `353e85c` | Fix: `gup-bevy`/`gup-egui` `DynChart` bound (see Retrospective) | +2 / −6         |
| `0033880` | Mixable ecosystem, `src/examples`, `#[derive(Mixable)]`         | +203 / −15,150  |
| `164ddae` | Error recovery/reporting/resource/context/cache modules         | +15 / −4,835    |
| **Total** | `git diff --shortstat main..HEAD`                               | +894 / −62,587  |

Of that, `src/` and `gup-macros/src/` account for +586 / −36,557, close to the
review's ~37k estimate. The rest is tests, benches, examples and docs for the
deleted code.

**Kept, with reasons** (AC1: a caller check found a real dependency):

- `src/mark/batch_renderer.rs`: its `InstanceAttributes` and `Viewport2D` are
  used by `compute_instance_filter`, `occlusion_culler`, `radix_sort`,
  `unified_culling_pipeline`, `lod/` and `renderer/`. GUP-390 moves that code to
  an experimental crate, and `batch_renderer` should go with it.
- `BlendMode` (was in `src/mixable.rs`): used by `render.rs`,
  `mark/advanced_rendering.rs`, `mark/performance_opt.rs`, `batch_renderer` and
  `visual_test_utils.rs`. Moved to `src/render.rs` (still `gup::BlendMode`).
- `GupError::configuration_error()` (was an `impl GupError` block in
  `src/error/recovery.rs`): called by `context.rs` and `debug/web_dashboard.rs`.
  Moved into `src/error.rs`.
- `src/pipeline_cache.rs`: the live `PipelineCache` (Selection, brush,
  linked_selection, ten examples). The `chart_builder` copy was deleted.
- `src/shader_pipeline.rs`: has production users; only its AST-optimiser path
  was removed.

**Also removed** as dead once their producers were gone:
`GupError::{CompositionError, FallbackAlreadyActive, RecoveryFailed, NoFallbackAvailable}`,
`GupError::composition_error()`, `ErrorCategory::Recovery`, and
`GupError::needs_full_context()` / `is_hot_path_error()` (these existed only to
choose lazy vs eager `ErrorContext` creation).

**Docs**: Tutorial 5 and `examples/tutorials/tutorial05_streaming.rs` were
rewritten from the deleted `StreamingDataSource`/`StreamingScatterPlot` to the
live `DataStream` API. `docs/mark-system/` now describes `Selection` as the
rendering path. Docs that described only deleted code were removed:
`BLEND_MODES.md`, `ERROR_HANDLING_OPTIMIZATION.md`,
`GPU_TIMESTAMP_INTEGRATION.md`, `MIGRATION_FROM_OBSERVABLE_PLOT.md`,
`SHADER_FN_MIGRATION_GUIDE.md` and `transpilation-validation-report.md`.
Gallery, example-index and perf-threshold entries for deleted targets were
removed too.

**Validation**:

- `cargo test -- --test-threads=1`: 4,137 passed, 0 failed, 163 ignored.
- `cargo check -p gup --all-targets --all-features` and `cargo check` of
  `gup-macros`, `gup-bevy`, `gup-egui`, `gup-ios` and `gup-android` build with
  no new warnings. No `#[allow(dead_code)]` was added.
- `03_line_chart` rendered headless and inspected: line, axes and ticks are
  correct. The missing text is the known GUP-394 "PNG export has no text" gap,
  not a regression.
- `visual_blend_demo` (the main `BlendMode` consumer) was run windowed and
  screenshotted: the alpha-blended overlap of the two quads renders correctly.
- `tutorial05_streaming` was run: the sliding window caps at 1,000 of 1,500
  pushed points.
- `pattern_pipeline_demo` was run: sections 1–2 pass, and section 3 hits a
  pre-existing panic (see Retrospective).

## Retrospective

**Completed**: 2026-10-05

### Key Technical Learnings

#### Type-name caller checks miss inherent impls

- **Challenge**: The error-module check grepped every top-level `pub` item name
  and found no production caller. Deleting the modules then broke the build,
  because `src/error/recovery.rs` held an `impl GupError` block whose
  `configuration_error()` is called from `context.rs` and `web_dashboard.rs`.
- **Solution**: Moved the helper into `src/error.rs`.
  `git grep -n "^impl " HEAD -- <paths to delete>` lists every impl a module
  adds to a type that outlives the deletion.
- **Pattern**: A caller check needs two greps: one for the items a module
  defines, and one for the impls it adds to types defined elsewhere (inherent
  impls, trait impls, `From` conversions). The compiler is the final check, so
  run `cargo check` straight after `git rm`, before any cleanup.

#### Deleting a producer orphans the consumer's vocabulary

- **Challenge**: Once Mixable and the recovery framework were gone, four
  `GupError` variants, an `ErrorCategory` and two `GupError` helpers had no
  producer left. Nothing warns about this: public enum variants are never
  "unused".
- **Solution**: Counted constructors outside `error.rs` for each variant, then
  checked which helper constructors build them. Variants built through a
  still-used helper (e.g. `validation_error()`) are live, so only the ones tied
  to the deleted subsystems were removed.
- **Pattern**: After deleting a subsystem, grep for the error variants, enum
  cases and config flags it produced or consumed.

#### Debug info dominates disk on a full test run

- **Challenge**: About 111 test binaries at roughly 230 MB each (mostly
  dependency debug info) would have needed ~25 GB on a nearly full pool.
- **Solution**: `CARGO_PROFILE_DEV_STRIP=debuginfo` brings test binaries to ~25
  MB. It triggers one rebuild of every dependency, because the profile is part
  of the fingerprint.
- **Pattern**: Use `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_STRIP=debuginfo` for
  full-suite runs on constrained disks.

### Architectural Decisions

#### Move survivors instead of keeping their host module

- **Decision**: `BlendMode` moved to `render.rs` and `configuration_error()`
  moved to `error.rs`, instead of keeping thin `mixable.rs`/`recovery.rs` stubs.
- **Reasoning**: A stub module named after a deleted subsystem invites new code
  to build on it. `pub use render::*` keeps `gup::BlendMode` stable.
- **Trade-off**: The `gup::mixable::BlendMode` path is gone (breaking changes
  are allowed by Decision 2).
- **Future**: blend configuration is still split between `render.rs` and
  `mark/advanced_rendering.rs`. RFC-001's new core should define blending once.

#### `batch_renderer.rs` travels with GUP-390, not this story

- **Decision**: Kept `batch_renderer.rs` although the story listed it.
- **Reasoning**: `InstanceAttributes`/`Viewport2D` have live callers in the
  culling and LOD code that GUP-390 quarantines. Deleting it here would mean
  rewriting code that is about to move anyway.
- **Trade-off**: About 1.4k LOC from the review's MarkRenderer estimate stays
  until GUP-390.

#### Port Tutorial 5 rather than delete it

- **Decision**: Rewrote the tutorial and its example on `DataStream` instead of
  deleting them with `async_mixable::streaming`.
- **Reasoning**: The tutorial track needs a streaming chapter, and `DataStream`
  plus `Selection::stream()` is the live path. The old text also had factual
  errors: it used a non-existent `update.count` field, and it claimed `Block`
  backpressure blocks a sync push (it drops the item).

### Development Workflow Insights

- **`git commit` commits the whole index.** I had `git rm`'d the Mixable files
  before committing an unrelated three-file fix (`git add` of just those files
  doesn't unstage the rest). That commit swallowed 14k lines of Mixable
  deletions under the wrong message and did not build on its own. I rebuilt the
  history on the unpushed branch with `git reset --soft`, then
  `git read-tree <sha>` + `git commit -C <sha>` per commit, and confirmed the
  result with an empty `git diff <backup> HEAD`. Run `git diff --cached --stat`
  before every commit while a large deletion is staged.
- **The pre-commit hook reads the index.** `mdl --git-recurse` walks tracked
  files, so committing an intermediate tree failed on a Markdown file that was
  still tracked but already deleted on disk. Restore such files with
  `git checkout -- <file>` until the commit that deletes them.
- **Prettier preserves prose wrapping** in this repo, so hand-edited paragraphs
  must be wrapped at 80 columns by hand. The first MarkRenderer commit attempt
  failed `prettier --check` on two edited docs.
- **Pre-existing panic (recorded, not fixed: outside the deleted code)**:
  `cargo run --example pattern_pipeline_demo` panics in section 3,
  "Accessibility Integration", with "Cannot start a runtime from within a
  runtime" at `src/accessibility/platform.rs:212`.
  `LinuxAccessibility::initialize()` builds a current-thread tokio runtime and
  calls `block_on`, so it panics whenever `AccessibilitySystem::new()` runs
  inside an existing runtime (here `#[tokio::main]`). Reproduce on Linux with
  `cargo run --example pattern_pipeline_demo`. `pattern_rendering_demo` and
  `web_accessibility_demo` call the same constructor and are probably affected.
  Fix direction: use `tokio::runtime::Handle::try_current()` and fall back to an
  owned runtime only when none exists, or make `initialize` async.
- **`gup-egui` and `gup-bevy` did not compile on `main`**:
  `ComposedChart::render` requires `M: MarkInstanceBuilder`, but both `DynChart`
  impls bounded only `M: Mark`. This was a one-line fix in each crate
  (`353e85c`). GUP-390 currently plans to _exclude_ these crates because they
  don't compile, so that premise should be revisited.

### Follow-up Stories

No new story files. Notes for existing work:

1. **GUP-390**: also move `src/mark/batch_renderer.rs` into the experimental
   crate, and reconsider excluding `gup-egui`/`gup-bevy` now that they build.
2. **Accessibility runtime panic**: the fix is a few lines but touches platform
   accessibility code outside this story's scope. Track it with the
   accessibility work, or as a small bug story if that track is active.
3. `.github/agents/story-worker.md` still uses `Mixable` and
   `GupError::CompositionError(...)` in illustrative snippets. Both are now
   deleted, so the agent prompt needs updating by its owner.
