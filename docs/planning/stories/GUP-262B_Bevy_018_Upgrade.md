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

- Render-world entity model changed: main-world entity IDs no longer
  auto-exist in the render world. Added `cleanup_extracted_gup_charts` system
  and switched to `commands.spawn(...)` in the extract schedule.

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
