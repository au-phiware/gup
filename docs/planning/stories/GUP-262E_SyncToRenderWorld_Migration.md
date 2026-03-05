# GUP-262E: Migrate gup-bevy to SyncToRenderWorld

**Initiative**: Ecosystem Integration **Status**: 💡 New **Created**: 2025-07-18

## Overview

Replace the spawn-per-frame + cleanup extraction pattern in `gup-bevy` with
Bevy 0.18's `SyncToRenderWorld` component for persistent render-world entity
mapping.

## Context

GUP-262B upgraded gup-bevy to Bevy 0.18 and worked around the new render-world
entity model by spawning fresh `ExtractedGupChart` entities each frame and
cleaning up stale ones. This works but is not the idiomatic Bevy 0.18 pattern.
Bevy 0.18 introduced `SyncToRenderWorld`, `RenderEntity`, and `MainEntity` to
provide automatic, persistent mapping between main-world and render-world
entities.

## User Story

As a Bevy plugin author, I want gup-bevy to follow Bevy's recommended
extraction pattern so that the integration is more efficient and maintainable as
Bevy evolves.

## Acceptance Criteria

- [ ] `ChartTextureTarget` or `GupChart` uses `SyncToRenderWorld` as a required
  component.
- [ ] The extract system uses `RenderEntity` to look up the render-world entity
  and inserts `ExtractedGupChart` on it.
- [ ] The `cleanup_extracted_gup_charts` system is removed.
- [ ] All existing tests pass.
- [ ] The `bevy_scatter` example runs without entity accumulation.

## Technical Tasks

1. Add `SyncToRenderWorld` as a required component on `GupChart`.
2. Rewrite `extract_gup_charts` to query `RenderEntity` and insert on the
   mapped render-world entity.
3. Remove `cleanup_extracted_gup_charts`.
4. Update plugin system registration.
5. Verify no entity leaks with a multi-frame test.

## Dependencies

- GUP-262B ✅

## Testing Strategy

- All existing `gup-bevy` tests pass.
- Add test verifying render-world entity count stays constant across frames.
- Example visual verification.

## Risk Assessment

- **Low**: `SyncToRenderWorld` is a well-documented Bevy 0.18 feature.
- **Low**: Changes are confined to `gup-bevy` crate.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] Tests pass
- [ ] No entity accumulation in render world
