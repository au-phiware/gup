# GUP-262B: Bevy 0.18 Upgrade

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
2025-03-04

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

- [ ] `gup-bevy` depends on `bevy = "0.18"` and `wgpu = "27"`.
- [ ] All existing tests pass with the new Bevy version.
- [ ] The `bevy_scatter` example compiles and runs.
- [ ] Version compatibility table in `docs/BEVY_INTEGRATION.md` is updated.

## Dependencies

- GUP-262 ✅
- Main `gup` crate wgpu 27 upgrade (prerequisite, not yet planned)

## Testing Strategy

- All existing `gup-bevy` tests pass.
- Example visual verification.
- CI matrix includes Bevy 0.18.

## Risk Assessment

- **Low**: Bevy 0.17 → 0.18 API changes are typically minor for plugin authors.
- **Medium**: wgpu 26 → 27 may introduce breaking API changes in the main `gup`
  crate that need separate resolution.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] Tests pass
- [ ] Documentation updated
