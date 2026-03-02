# GUP-279: Linked View Coordination

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress **Created**:
2025-07-14

## Context

Coordinated multiple views (CMV), also known as linked views, is one of the most
powerful patterns in interactive data visualization. When a user selects or
brushes a subset of data in one chart, all other charts in the dashboard
immediately highlight or dim the corresponding data items. This allows analysts
to explore relationships across dimensions simultaneously — for example,
brushing a time range in a timeline chart to highlight matching items in a
scatter plot and a bar chart.

GUP-075 delivered GPU-accelerated interactive mark selection for a single chart,
and GUP-278 (Brush Mark for Rectangular Selection) provides the rectangular
brush gesture that drives selection in a chart. These stories operate on
per-chart, index-based selection state. However, neither provides a mechanism
for one chart's selection to propagate to another: there is no shared selection
state and no notion of cross-chart item identity.

This story introduces `SharedSelectionState` — an `Arc<Mutex<SelectionState>>`
that multiple charts receive at construction time. When a brush or click in
Chart A updates the shared state, Chart B reads from the same state and
re-renders its marks with visual dimming applied to non-selected items.
Selection identity is based on user-supplied key functions rather than raw
buffer indices, making it robust across charts whose data arrays may be ordered
differently.

GUP-001's `Mixable` trait already enables composition of multiple charts into a
single rendering surface. This story builds on that foundation by adding the
cross-chart communication layer that makes such compositions interactive.

## User Story

> "As a visualization developer, I want to pass a shared selection state to
> multiple charts so that brushing or clicking in one chart automatically
> highlights the same data items across all linked charts."

> "As an end user exploring a multi-chart dashboard, I want unselected data
> items to be visually dimmed so that I can clearly see which items are
> highlighted by my selection, regardless of which chart I interact with."

## Acceptance Criteria

### AC1: SharedSelectionState type

- [ ] A `SharedSelectionState` type is provided, backed by
      `Arc<Mutex<SelectionState>>`
- [ ] `SelectionState` stores selected item keys as a `HashSet<K>` where
      `K:     Hash + Eq + Send + Sync + 'static`
- [ ] `SharedSelectionState` can be cloned cheaply (cloning the `Arc`, not the
      data) and passed to multiple charts
- [ ] `SharedSelectionState` exposes `select(keys)`, `deselect(keys)`,
      `clear()`, and `is_selected(key)` methods without requiring the caller to
      manage the lock directly
- [ ] All public types implement `Debug` and `Clone`

### AC2: Key function API

- [ ] Charts that participate in linked views accept a
      `key_fn: impl Fn(&T) ->     K` parameter that maps each data item to its
      cross-chart identity key
- [ ] The key function is stored in the chart and invoked when the selection
      state changes, to map selected keys back to local indices for GPU upload
- [ ] A data item with no matching key in the current `SelectionState` is
      treated as unselected (not as an error)
- [ ] When `SharedSelectionState` contains no selected keys (empty set), all
      items render at full opacity (no dimming)

### AC3: Visual dimming of unselected items

- [ ] When one or more items are selected, unselected items are rendered at a
      configurable reduced opacity (default: 0.2)
- [ ] Selected items render at full opacity (1.0)
- [ ] The dimming opacity value is configurable per-chart via a
      `selection_dim_opacity(f32)` builder method
- [ ] Dimming is applied via the existing mark shader pipeline (e.g., by
      modifying the alpha channel of the mark's colour uniform or an instance
      attribute), without introducing a separate render pass
- [ ] GPU validation layers produce no errors during dimming transitions

### AC4: Cross-chart propagation

- [ ] Modifying `SharedSelectionState` (via a brush in Chart A or any other
      mechanism) causes Chart B to re-render with updated dimming within the
      same frame or at most one frame later
- [ ] Charts poll or subscribe to `SharedSelectionState` changes without busy-
      waiting (e.g., via a dirty flag, a generation counter, or an async
      notification channel)
- [ ] If two charts hold the same `SharedSelectionState` and Chart A clears the
      selection, Chart B also returns to full-opacity rendering

### AC5: Integration with Brush Mark (GUP-278)

- [ ] A chart configured with a brush mark and a `SharedSelectionState`
      automatically writes the brushed item keys into the shared state on each
      brush update
- [ ] The integration requires no boilerplate beyond passing the
      `SharedSelectionState` to the chart builder; the wiring is handled
      internally

### AC6: Example demonstrating linked scatter plots

- [ ] A runnable example `examples/linked_views.rs` shows two scatter plots
      rendered side by side using the `Mixable` composition trait
- [ ] Both scatter plots share the same `SharedSelectionState` and the same key
      function (item index or a named field)
- [ ] Brushing a rectangular region in the left scatter plot highlights the
      corresponding points in the right scatter plot
- [ ] The example compiles with `cargo check --examples` and runs without panics
      or GPU validation errors

## Technical Tasks

- [ ] Define `SelectionState<K>` struct in `src/interaction/selection_state.rs`
      with `HashSet<K>` for selected keys, and a generation counter (`u64`) for
      change detection
- [ ] Define `SharedSelectionState<K>` as a newtype wrapping
      `Arc<Mutex<SelectionState<K>>>` with ergonomic `select`, `deselect`,
      `clear`, and `is_selected` methods
- [ ] Add `Clone` and `Debug` implementations; derive or manually implement as
      appropriate given the `Arc` wrapper
- [ ] Add
      `with_shared_selection(state: SharedSelectionState<K>, key_fn: impl     Fn(&T) -> K)`
      builder method to the chart/mark builder API
- [ ] Add `selection_dim_opacity(f32)` builder method with a default of `0.2`
- [ ] Implement change detection: store last-seen generation in each chart's
      render state; on each `render()` call, compare against current generation
      in the `SharedSelectionState` and rebuild the per-instance selection
      attribute buffer if changed
- [ ] Add a per-instance `u32` selection flag attribute (0 = unselected, 1 =
      selected or no active selection) to the mark instance buffer, or extend
      the existing colour/alpha instance data
- [ ] Update the mark vertex/fragment shaders to multiply the alpha channel by
      the dim opacity when the selection flag is 0 and at least one key is
      selected globally
- [ ] Wire brush mark (GUP-278) selection callback to write brushed keys into
      `SharedSelectionState` when one is configured
- [ ] Write unit tests for `SelectionState`: `select`, `deselect`, `clear`,
      `is_selected`, and generation counter increment
- [ ] Write integration test: construct two chart instances sharing a
      `SharedSelectionState`, call `select()`, verify both charts' rendered
      instance buffers reflect the expected selection flags
- [ ] Create `examples/linked_views.rs` with two side-by-side scatter plots and
      a brush on the left chart
- [ ] Update `src/interaction/mod.rs` to re-export the new types
- [ ] Document public API with `///` doc-comments including usage examples

## Dependencies

### Prerequisite Stories

- GUP-001: Build Mixable Trait ✅ — `Mixable` composition enables multiple
  charts on a single surface
- GUP-075: Interactive Mark Selection ✅ — GPU selection pipeline and
  `SelectionState` foundations this story extends to shared, cross-chart scope
- GUP-013: Event Handling System 📋 — `.on()` API and `EventManager` that route
  brush and click events into the selection update path
- GUP-278: Brush Mark for Rectangular Selection 📋 — provides the rectangular
  brush gesture that drives selection; this story integrates it with the shared
  state

### Enables Stories

- Future dashboard composition stories — linked views is a prerequisite for any
  story that requires cross-chart data coordination (filters, zoom sync, etc.)

## Testing Strategy

- **Unit tests**: `SelectionState` CRUD operations; generation counter advances
  on every mutation; `SharedSelectionState` clone shares the same underlying
  data
- **Integration tests**: Two mock charts sharing a `SharedSelectionState`;
  assert that after `select(keys)` both charts' `dirty` flag is set and their
  per-instance attribute buffers are rebuilt correctly on the next render tick
- **Visual validation**: Run `examples/linked_views.rs`, brush the left scatter
  plot, screenshot both charts — unselected points must be visibly dimmed in
  both
- **Performance**: Rebuilding the per-instance selection buffer for 100 K points
  after a selection change should complete in under 2 ms on CPU; measure with a
  criterion benchmark if the data path is non-trivial

## Success Metrics

- [ ] `SharedSelectionState` passes all unit and integration tests
- [ ] `examples/linked_views.rs` compiles and demonstrates visible cross-chart
      highlighting without GPU validation errors
- [ ] Dimming transition from no-selection to a selection and back produces no
      visual artefacts (flickering, incorrect opacity)
- [ ] Selecting 10 K items across two charts of 100 K points each causes no
      frame-time regression exceeding 2 ms compared to the unlinked baseline

## Risk Assessment

- **Medium**: Shader changes to support per-instance dimming may interact with
  existing colour/alpha uniform paths in unexpected ways. _Mitigation_: Add the
  selection flag as an independent instance attribute rather than modifying
  existing colour uniforms; keep the shader change minimal and test against the
  existing mark rendering tests.

- **Medium**: The generation-counter change detection approach adds a lock
  acquisition on every render call. For dashboards with many charts this could
  become a hot path. _Mitigation_: Use a non-blocking `try_lock` with a fallback
  to the previous generation; document the trade-off; revisit with a lock-free
  atomic counter if profiling shows contention.

- **Low**: GUP-278 (Brush Mark) is not yet complete, so the brush integration
  path cannot be end-to-end tested until that story lands. _Mitigation_: Design
  the `SharedSelectionState` API so that brush integration is a thin adapter;
  provide a manual `select(keys)` call in the example as a stand-in and note in
  the story that full brush wiring is contingent on GUP-278.

- **Low**: Generic key type `K` requires `Hash + Eq + Send + Sync + 'static`,
  which may complicate type inference for callers who use closures as key
  functions. _Mitigation_: Provide convenience type aliases for common key types
  (e.g., `SharedSelectionState<usize>`) and document the constraints clearly.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
