# GUP-262: Bevy Integration

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-01-31 **Completed**: 2025-03-04

## Context

Bevy is a rapidly growing open-source Rust game engine built on an
Entity-Component-System (ECS) architecture. Its Rust-native design and active
community make it one of the most natural targets for a Gup integration: game
developers and simulation authors who already use Bevy want to embed live data
visualizations — scatter plots of entity positions, time-series of simulation
metrics, spatial histograms — directly inside their 3D scenes without spinning
up a second GPU context or writing adapter glue by hand.

The core challenge is resource sharing. Bevy owns the `wgpu` `Device` and
`Queue` for a running application; Gup independently creates its own
`GupContext` (GUP-004) that also wraps an `Arc<wgpu::Device>` and
`Arc<wgpu::Queue>`. Running both in the same process without coordination would
create two independent GPU contexts, wasting VRAM and defeating the purpose of a
tight integration. This story solves that problem by providing a `GupPlugin`
that extracts Bevy's wgpu resources and injects them into a shared `GupContext`
that is registered as a Bevy `Resource`.

GUP-004 gave us a `GupContext` backed by `Arc`-wrapped wgpu handles, making it
straightforward to construct one from externally-supplied device/queue arcs.
GUP-018 produced the chart builder API (`Chart` and friends) that Bevy
components will wrap. GUP-039 extended `GupContext` with multi-surface
management, which is the natural abstraction for rendering a chart into a
Bevy-managed texture target.

The implementation lives in a new `gup-bevy` crate (or behind a `bevy` feature
flag), keeping Bevy's dependency tree completely optional for users who do not
need it. The Bevy integration strategy shown in
`docs/IMPLEMENTATION_STRATEGY.md` — a `GupChart` component and a
`gup_chart_render_system` — serves as the direct specification for what is
delivered here.

## User Story

> "As a Bevy game developer, I want to add Gup data-visualization charts as ECS
> components so that I can render GPU-accelerated scatter plots and other charts
> inside my Bevy application without managing a second GPU context."

## Acceptance Criteria

### AC1: Shared wgpu Context

- [x] `GupPlugin` extracts Bevy's `wgpu::Device` and `wgpu::Queue` via
      `bevy::render::renderer::RenderDevice` / `RenderQueue` resources.
- [x] A `GupContext` is constructed from those extracted handles (no second GPU
      adapter is created).
- [x] The constructed `GupContext` is inserted as a Bevy `Resource` named
      `GupRenderContext`.
- [x] Running an example with Bevy's GPU validation layer active produces no
      validation errors related to cross-context resource usage.

### AC2: GupChart ECS Component

- [x] `GupChart` implements `bevy::ecs::component::Component`.
- [x] `GupChart` holds a `Chart` (from GUP-018) and an `auto_update: bool` flag.
- [x] `GupChart::new(chart)` and `GupChart::with_auto_update(chart, bool)`
      constructors are provided.
- [x] `GupChart` can be inserted on any Bevy `Entity` alongside standard Bevy
      components (e.g. `Transform`, `Visibility`).

### AC3: Render System Integration

- [x] A `gup_render_system` Bevy system is provided that queries all `GupChart`
      components and calls `Chart::render` for each one using the shared
      `GupRenderContext`.
- [x] The system is added to Bevy's `RenderApp` (or `PostUpdate` schedule as
      appropriate) by `GupPlugin::build`.
- [x] Charts with `auto_update: false` skip re-rendering unless explicitly
      marked dirty.
- [x] No `panic` or GPU error occurs when zero `GupChart` entities exist in the
      world.

### AC4: Bevy Scatter-Plot Example

- [x] A runnable example at `examples/bevy_scatter.rs` (or
      `gup-bevy/examples/scatter.rs`) exists and compiles with
      `cargo run -p gup-bevy --example bevy_scatter`.
- [x] The example creates a Bevy `App`, adds `GupPlugin`, spawns a `GupChart`
      entity containing a scatter plot built with the GUP-018 chart-builder API,
      and renders it in a Bevy window.
- [x] The scatter plot data updates each frame (e.g. animated sine wave) to
      demonstrate `auto_update: true` behaviour.
- [x] The example runs without GPU validation errors on at least one platform
      (Linux/macOS or Windows).

### AC5: Integration Guide Documentation

- [x] `docs/BEVY_INTEGRATION.md` (or equivalent) exists and covers: - Adding the
      `gup-bevy` crate / `bevy` feature to `Cargo.toml`. - Adding `GupPlugin` to
      a Bevy `App`. - Creating and spawning a `GupChart` component. - Updating
      chart data at runtime. - Known limitations and caveats (e.g. Bevy version
      compatibility).
- [x] All public items in `gup-bevy` have rustdoc comments.

### AC6: Feature-Gated Build

- [x] When the `bevy` feature (or `gup-bevy` crate) is not enabled, the main
      `gup` crate compiles without any Bevy dependency pulled in.
- [x] `cargo check` on the workspace root (without `--features bevy`) succeeds.
- [x] `cargo check -p gup-bevy` succeeds.

## Technical Tasks

- [ ] Decide on delivery mechanism: a workspace sub-crate `gup-bevy` vs. a
      `bevy` feature flag on the root `gup` crate. A sub-crate is preferred to
      keep the dependency graph clean.
- [ ] Add `gup-bevy/Cargo.toml` with
      `bevy = { version = "...", default-features = false,     features = ["..."] }`
      and `gup` as a path dependency.
- [ ] Implement `GupRenderContext` newtype resource wrapping `GupContext`.
- [ ] Implement `GupPlugin` using `bevy::app::Plugin`:
  - In `build()`, add `gup_render_system` to the appropriate Bevy schedule.
  - In a startup system or `finish()`, extract `RenderDevice`/`RenderQueue` from
    Bevy and construct the shared `GupContext`.
- [ ] Implement `GupChart` component struct with `Chart` and `auto_update`
      fields.
- [ ] Implement
      `gup_render_system(mut charts: Query<&mut GupChart>, context: ResMut<GupRenderContext>)`.
- [ ] Handle the `auto_update: false` path by introducing a `dirty` flag or
      relying on an explicit `GupChartDirty` marker component.
- [ ] Write the `bevy_scatter` example demonstrating per-frame data updates.
- [ ] Write `docs/BEVY_INTEGRATION.md`.
- [ ] Add rustdoc to all public items in `gup-bevy`.
- [ ] Extend CI to run `cargo check -p gup-bevy` (or `--features bevy`) on at
      least one platform.

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides `GupContext` with
  `Arc<wgpu::Device>` and `Arc<wgpu::Queue>`, enabling construction from
  externally supplied handles.
- GUP-018: Observable Plot Chart Builders ✅ — provides the `Chart` type that
  `GupChart` wraps and the chart-builder API used in the example.
- GUP-039: Context Window Integration ✅ — multi-surface `GupContext` management
  needed to render into Bevy-owned texture targets.

### Enables Stories

- A future `gup-egui` integration story would follow the same pattern (shared
  context, widget wrapper, plugin/system registration) established here.

## Testing Strategy

- **Unit tests**: Test that `GupChart::new` and `with_auto_update` set fields
  correctly. Test that a `GupContext` constructed from given `Arc<Device>` /
  `Arc<Queue>` references the same underlying objects (pointer equality).
- **Integration tests**: In a headless Bevy world (no window), instantiate
  `GupPlugin`, spawn a `GupChart`, run one app tick, and assert no panic occurs.
  Use Bevy's `App::update()` for headless operation.
- **Visual validation**: Run `bevy_scatter` example and observe animated scatter
  plot embedded in a Bevy window; capture a screenshot to confirm the chart is
  visible.
- **Compile-time gating**: CI step that verifies `cargo check` without `bevy`
  feature succeeds (no accidental Bevy dependency bleed).

## Success Metrics

- [ ] `cargo run --example bevy_scatter` (with the Bevy feature/crate enabled)
      renders an animated scatter plot in a Bevy window with no GPU validation
      errors.
- [ ] `cargo check` on the workspace without the `bevy` feature produces zero
      errors and zero warnings related to `gup-bevy` symbols.
- [ ] All integration tests pass: `cargo test -p gup-bevy -- --test-threads=1`.
- [ ] The integration guide covers the full add-to-project workflow in fewer
      than 30 lines of user-written code.

## Risk Assessment

- **Medium**: Bevy's render graph and wgpu resource extraction API changes
  frequently between minor versions. The integration must pin a specific Bevy
  version and document that constraint clearly. _Mitigation_: Pin to the latest
  stable Bevy release at story start; add a `BEVY_VERSION` note to
  `docs/BEVY_INTEGRATION.md` and the crate README. Schedule a follow-up story
  for version bumps.

- **Medium**: Bevy's `RenderDevice` abstraction may not expose the raw
  `Arc<wgpu::Device>` directly in all versions — it may require
  `bevy::render::renderer::RenderDevice::wgpu_device()`. Verify the extraction
  API before committing to an approach. _Mitigation_: Prototype the resource
  extraction in a throwaway binary before writing full implementation; fall back
  to constructing a second `GupContext` with a texture-copy path if sharing
  proves infeasible, and document the trade-off.

- **Low**: Bevy's `RenderApp` runs in a separate thread from the main world.
  Cross-thread `Resource` access requires `Send + Sync`, which `GupContext`
  already satisfies (GUP-238 audited `Send + Sync` bounds), but chart data
  mutations must go through proper Bevy change-detection channels. _Mitigation_:
  Use Bevy's standard `Commands` / `EventWriter` patterns for data updates;
  document this in the integration guide.

- **Low**: Build times. Bevy has a large dependency tree. The sub-crate approach
  isolates this cost to users who opt in. _Mitigation_: Use
  `default-features = false` on the Bevy dependency and enable only the features
  strictly required (windowing, rendering, ecs).

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-bevy -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples` (with appropriate feature
      flags)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

### What Was Implemented

- **`gup-bevy` workspace sub-crate** — A standalone crate that depends on `gup`
  and `bevy 0.17` (wgpu 26.x) without polluting the main crate's dependency
  graph.
- **`GupPlugin`** — Bevy `Plugin` that extracts `RenderDevice`/`RenderQueue`/
  `RenderAdapter`/`RenderInstance` from Bevy's render sub-app and constructs a
  shared `GupRenderContext` in `finish()`.
- **`GupRenderContext`** — Bevy `Resource` wrapping both a `GupContext` and a
  `RenderContext`, constructed from Bevy's own wgpu handles.
- **`GupChart`** — Bevy `Component` with type-erased chart storage (`DynChart`
  trait), `auto_update` flag, and `dirty` flag.
- **`gup_render_system`** — `PostUpdate` system that renders dirty charts to PNG
  and updates the backing Bevy `Image` asset on each frame.
- **`GupContext::from_wgpu` / `RenderContext::from_wgpu`** — New constructors on
  the main `gup` crate that allow creating contexts from external wgpu handles.
- **`bevy_scatter` example** — Animated sine-wave scatter plot in a Bevy window.
- **`docs/BEVY_INTEGRATION.md`** — Full integration guide with version table,
  minimal example, architecture description, and known limitations.

### Key Files Changed

| File                                | Change                                    |
| ----------------------------------- | ----------------------------------------- |
| `Cargo.toml`                        | Added `gup-bevy` to workspace members     |
| `src/context.rs`                    | Added `GupContext::from_wgpu`             |
| `src/render.rs`                     | Added `RenderContext::from_wgpu`          |
| `gup-bevy/Cargo.toml`               | New crate manifest                        |
| `gup-bevy/src/lib.rs`               | Crate root with prelude                   |
| `gup-bevy/src/plugin.rs`            | `GupPlugin` implementation                |
| `gup-bevy/src/context.rs`           | `GupRenderContext` resource               |
| `gup-bevy/src/chart.rs`             | `GupChart` component + `DynChart` trait   |
| `gup-bevy/src/render_system.rs`     | `gup_render_system` + `blank_chart_image` |
| `gup-bevy/examples/bevy_scatter.rs` | Animated scatter plot example             |
| `gup-bevy/tests/integration.rs`     | 8 integration tests                       |
| `docs/BEVY_INTEGRATION.md`          | Full integration guide                    |

### Test Counts

- **8 integration tests** — GupChart constructors, dirty lifecycle,
  render_to_png, GupRenderContext creation, headless Bevy tick.
- **2 doc tests** — Module-level and plugin doc examples compile.
- **2665 main gup tests** — All pass (no regressions).

## Retrospective

**Completed**: 2025-03-04

### Key Technical Learnings

#### Bevy Version × wgpu Version Matrix

- **Challenge**: The story required sharing wgpu `Device`/`Queue` between Bevy
  and Gup. This only works if both use the exact same wgpu version, since
  `wgpu::Device` from version 26 is a different Rust type than from version 27.
- **Solution**: Tested Bevy 0.15 (wgpu 23), 0.16 (wgpu 24), 0.17 (wgpu 26), and
  0.18 (wgpu 27). Bevy **0.17** was the only release matching Gup's wgpu 26
  requirement.
- **Pattern**: Always check the entire transitive dependency graph for version
  alignment before choosing a framework integration target. A simple
  `cargo metadata` query resolves this quickly.

#### Type Erasure for Generic Charts

- **Challenge**: `ComposedChart<T, M>` is generic over data type and mark type,
  but Bevy `Component`s must be concrete `'static` types. The `Mixable` trait is
  not object-safe (has `Sized` constraints and generic methods).
- **Solution**: Introduced `DynChart`, a minimal object-safe trait with just
  `render` and `render_to_png`. Blanket-implemented for all
  `ComposedChart<T, M>` where `T: Clone + Send + Sync + Debug + 'static` and
  `M: Mark`. `GupChart` stores `Box<dyn DynChart>`.
- **Pattern**: When a framework's trait isn't object-safe, create a narrow
  object-safe "rendering" trait and implement it generically.

#### Bevy's `finish()` Hook for Render Resource Access

- **Challenge**: Bevy's `RenderDevice` / `RenderQueue` are only available after
  `RenderPlugin` finishes initialization. Calling `app.world().resource()` in
  `Plugin::build()` panics.
- **Solution**: Used `Plugin::finish()` instead of `build()` for context
  extraction. In `finish()`, the render sub-app is fully initialized and
  resources are safe to access.
- **Pattern**: In Bevy plugins, use `build()` for system registration and
  `finish()` for late resource extraction from the render world.

#### wgpu 26 Internal Reference Counting

- **Challenge**: Gup's `GupContext` stores `Arc<Device>` while Bevy's
  `RenderDevice` wraps `WgpuWrapper<Device>`. Sharing the same underlying GPU
  device requires either a single `Arc` or some other reference mechanism.
- **Solution**: In wgpu 26, `Device`, `Queue`, `Adapter`, and `Instance` are
  internally reference-counted (they contain an `Arc` inside). Calling
  `.clone()` is a cheap Arc bump, not a new GPU allocation. This means wrapping
  a cloned `Device` in a separate `Arc<Device>` is safe — both Arcs point to the
  same inner GPU handle.
- **Pattern**: wgpu types in v26+ are Clone-cheap. Don't fight the borrow
  checker — just clone them.

### Architectural Decisions

#### Separate `gup-bevy` Crate (Not a Feature Flag)

- **Decision**: Implemented as a standalone workspace member crate rather than a
  `bevy` feature flag on the main `gup` crate.
- **Reasoning**: Bevy 0.17 pulls in ~120 transitive dependencies including
  `winit`, ECS, asset loading, etc. Feature-gating this behind a flag would
  still require all downstream users to see the dependency in `Cargo.toml` and
  risk accidental activation. A separate crate keeps the dependency boundary
  clean.
- **Trade-off**: Users add two dependencies (`gup` + `gup-bevy`) instead of one
  with a feature flag.
- **Future**: If Bevy becomes a more common integration target, a feature flag
  on the main crate could be reconsidered.

#### Render-to-PNG Round-Trip

- **Decision**: Charts are rendered to PNG bytes, decoded, and loaded as Bevy
  `Image` assets rather than rendering directly to GPU textures.
- **Reasoning**: The simplest correct implementation. `ComposedChart` already
  has `render_to_png`; Bevy's `Image::from_buffer` can load PNGs. This avoids
  the complexity of integrating with Bevy's render graph or manually managing
  texture views between Gup and Bevy.
- **Trade-off**: GPU → CPU → GPU round-trip per frame per chart. Not suitable
  for large charts or many charts at 60 fps.
- **Future**: A dedicated story should add direct texture sharing (render to a
  shared `wgpu::Texture`, wrap as Bevy `Image` with zero-copy).

#### `PostUpdate` Schedule Placement

- **Decision**: `gup_render_system` runs in `PostUpdate`, not inside Bevy's
  render graph.
- **Reasoning**: Avoids deep coupling to Bevy's render internals. The system
  only needs `Query<(&mut GupChart, &mut Sprite)>` and `ResMut<Assets<Image>>`,
  which are main-world resources. Running in `PostUpdate` ensures all user
  `Update` systems have had a chance to modify chart data.
- **Trade-off**: Chart rendering is synchronous on the main thread.
- **Future**: Moving rendering to the render world (via `ExtractComponent`)
  would enable parallel rendering with Bevy's renderer.

### Development Workflow Insights

- **Bevy compilation time**: First build of `bevy 0.17` takes ~45 seconds even
  with minimal features. Incremental rebuilds of `gup-bevy` alone take <1s.
- **Disk space**: Building both `gup` and `bevy` from scratch consumed ~69 GB in
  the target directory. Using a shared target dir on a large filesystem is
  essential; `/tmp` ran out of space.
- **Testing Bevy systems**: `MinimalPlugins` doesn't include `Assets<Image>`, so
  systems that access `ResMut<Assets<Image>>` need the resource to be optional
  (`Option<ResMut<...>>`). This also makes the system more robust in production.
- **WgpuWrapper access**: Bevy wraps all wgpu types in `WgpuWrapper<T>` for WASM
  Send/Sync safety. Accessing the inner type requires understanding the deref
  chain: `RenderQueue` → `Arc<WgpuWrapper<Queue>>` → `WgpuWrapper<Queue>` →
  `Queue`. The `.0` field is `pub`, and `WgpuWrapper::into_inner()` provides
  clean access.

### Follow-up Stories

1. **GUP-262A: Direct Texture Sharing for Bevy** — Eliminate the render-to-PNG
   round-trip by rendering charts directly to a `wgpu::Texture` that is wrapped
   as a Bevy `Image` handle with zero-copy. Would dramatically improve
   performance for animated charts.

2. **GUP-262B: Bevy 0.18 Upgrade** — When the main gup crate upgrades to wgpu
   27, update gup-bevy to target Bevy 0.18. This requires verifying API
   compatibility and updating any changed Bevy APIs.
