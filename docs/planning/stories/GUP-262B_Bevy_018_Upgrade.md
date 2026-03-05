# GUP-262B: Bevy 0.18 Upgrade

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-03-04 **Completed**: 2025-07-18

## Overview

Upgrade `gup-bevy` from Bevy 0.17 (wgpu 26) to Bevy 0.18 (wgpu 27) when the main
`gup` crate upgrades its wgpu dependency.

## Context

GUP-262 targets Bevy 0.17 because it is the only release that ships wgpu 26.x,
matching the main `gup` crate's wgpu requirement. Bevy 0.18 ships wgpu 27.x.
This story tracks the version bump required once `gup` upgrades to wgpu 27.

## User Story

As a game developer, I want to use the latest Bevy release so that I have access
to the latest Bevy features and bug fixes alongside Gup charts.

## Acceptance Criteria

- [x] `gup-bevy` depends on `bevy = "0.18"` and `wgpu = "27"`.
- [x] All existing tests pass with the new Bevy version.
- [x] The `bevy_scatter` example compiles and runs.
- [x] Version compatibility table in `docs/BEVY_INTEGRATION.md` is updated.

## Dependencies

- GUP-262 ✅
- Main `gup` crate wgpu 27 upgrade ✅ (performed as part of this story)

## Testing Strategy

- All existing `gup-bevy` tests pass.
- Example visual verification.
- CI matrix includes Bevy 0.18.

## Risk Assessment

- **Low**: Bevy 0.17 → 0.18 API changes are typically minor for plugin authors.
- **Medium**: wgpu 26 → 27 may introduce breaking API changes in the main `gup`
  crate that need separate resolution.

## Definition of Done

- [x] All Acceptance Criteria satisfied
- [x] Tests pass
- [x] Documentation updated

## Implementation Summary

### What was implemented

1. **wgpu 26 → 27 upgrade** across the entire workspace (main `gup` crate,
   `gup-bevy`, examples, tests, benchmarks)
2. **Bevy 0.17 → 0.18 upgrade** for the `gup-bevy` crate
3. **Bevy 0.18 render-world entity model adaptation** — replaced
   `commands.entity(entity).insert(...)` with `commands.spawn(...)` plus a
   per-frame cleanup system for stale extracted entities

### wgpu 26 → 27 API changes addressed

- `PollType::Wait` changed from a unit variant to a struct variant with
  `submission_index: Option<SubmissionIndex>` and `timeout: Option<Duration>`
- `PollType::WaitForSubmissionIndex` removed — replaced by
  `PollType::Wait { submission_index: Some(...), timeout: None }`
- `DeviceDescriptor` gained a required `experimental_features` field

### Bevy 0.17 → 0.18 API changes addressed

- Render-world entity model changed: main-world entity IDs no longer auto-exist
  in the render world. Added `cleanup_extracted_gup_charts` system and switched
  to `commands.spawn(...)` in the extract schedule.

### Key files changed

- `Cargo.toml` — wgpu 26→27
- `gup-bevy/Cargo.toml` — bevy 0.17→0.18, wgpu 26→27
- `gup-bevy/src/render_node.rs` — extract system rewrite for Bevy 0.18
- `gup-bevy/src/plugin.rs` — register cleanup system
- `gup-bevy/src/context.rs`, `gup-bevy/src/plugin.rs` — comment updates
- `docs/BEVY_INTEGRATION.md` — version compatibility table
- `gup-bevy/README.md` — version table
- 50+ source files across `src/`, `tests/`, `benches/`, `examples/` — PollType
  and DeviceDescriptor API adaptations

### Test results

- 13 gup-bevy integration tests pass
- 2 doc-tests pass
- `bevy_scatter` example runs and displays window

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### wgpu 26 → 27 PollType API Change

- **Challenge**: `PollType::Wait` changed from a unit variant to a struct
  variant with `submission_index` and `timeout` fields, and
  `PollType::WaitForSubmissionIndex` was removed entirely. This affected 60+
  call sites across the codebase.
- **Solution**: Sed-based mass replacement: `PollType::Wait` →
  `PollType::Wait { submission_index: None, timeout: None }`,
  `PollType::WaitForSubmissionIndex(idx)` →
  `PollType::Wait { submission_index: Some(idx), timeout: None }`.
- **Pattern**: wgpu major version bumps tend to consolidate enum variants. Check
  release notes for such collapses before upgrading.

#### Bevy 0.18 Render-World Entity Model

- **Challenge**: In Bevy 0.17, `commands.entity(main_world_entity).insert(...)`
  worked in the Extract schedule because entities were implicitly mirrored. In
  Bevy 0.18, main-world entity IDs do not exist in the render world by default,
  causing a panic: "Entity not yet spawned".
- **Solution**: Changed to `commands.spawn(component)` to create fresh
  render-world entities, and added a `cleanup_extracted_gup_charts` system that
  runs before extraction to despawn stale entities from the prior frame.
- **Pattern**: For custom extract systems in Bevy 0.18+, always spawn new
  entities in the render world. Use `SyncToRenderWorld` + `RenderEntity` for
  persistent entity mapping if needed.

#### DeviceDescriptor Field Addition

- **Challenge**: wgpu 27 added `experimental_features` and `trace` as required
  fields on `DeviceDescriptor`. Using `..Default::default()` hides this, but
  explicit struct literals needed updating.
- **Solution**: Added `experimental_features: Default::default()` to all
  explicit `DeviceDescriptor` initializers. The `trace` field was already
  present from a prior wgpu update.
- **Pattern**: Prefer `..Default::default()` for `DeviceDescriptor` to be
  resilient to new required fields.

### Architectural Decisions

#### Upgrade wgpu in Main Crate as Part of This Story

- **Decision**: Upgraded the main `gup` crate from wgpu 26 to 27 alongside the
  Bevy upgrade, even though it was listed as a separate unplanned prerequisite.
- **Reasoning**: `gup-bevy` shares wgpu `Device`/`Queue` between Bevy and Gup.
  If the two crates use different wgpu major versions, their types are
  incompatible and the integration cannot work.
- **Trade-off**: Larger scope than originally planned for a Bevy-only upgrade.
- **Future**: The wgpu 27 upgrade is now done for the whole workspace; any
  future Bevy upgrades that bump wgpu will follow the same pattern.

#### Spawn + Cleanup vs SyncToRenderWorld

- **Decision**: Used spawn-per-frame + cleanup pattern instead of Bevy 0.18's
  `SyncToRenderWorld` component for render-world entity management.
- **Reasoning**: `SyncToRenderWorld` requires adding it as a required component
  and restructuring the entity model. The spawn + cleanup pattern is simpler,
  maintains backward compatibility, and sufficient for the small number of chart
  entities (typically < 10).
- **Trade-off**: Slightly less efficient (despawn + respawn each frame) but
  avoids coupling to Bevy's sync infrastructure.
- **Future**: If Gup supports hundreds of chart entities, migrating to
  `SyncToRenderWorld` would eliminate per-frame entity churn.

### Development Workflow Insights

- The wgpu upgrade was mechanical but wide-reaching (60+ files). Sed-based mass
  replacement worked well but required a second pass to fix double-application.
  Future mass replacements should use more precise patterns or `cargo fix`.
- Running the bevy_scatter example required a detached process since the
  compositor window manager needed time to register the window.
- Pre-existing clippy errors (`erasing_op`, `approx_constant`) in test files are
  not related to this change and should be addressed in a separate cleanup.

### Follow-up Stories

1. **GUP-262E: Migrate gup-bevy to SyncToRenderWorld** — Replace the
   spawn-per-frame + cleanup extraction pattern with Bevy 0.18's
   `SyncToRenderWorld` component for persistent render-world entity mapping.
   This would eliminate per-frame entity churn and align with Bevy's recommended
   extraction pattern for long-lived entities.
