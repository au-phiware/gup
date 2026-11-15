---
implementer: sonnet
implementer-reason:
  reproduce first; likely a surface-acquire timeout on frames slower than the
  presentation timeout; the fix is in the window host's frame loop
touches:
  [
    crates/gup-core/src/app.rs,
    crates/gup-core/examples/zoom_bench.rs,
    crates/gup-core/tests/,
  ]
---

# GUP-734: A Slow Frame Does Not Fail With "Acquiring a texture failed"

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-16

## Context

GUP-717 ran `zoom_bench` with `--lod off` at 10M rows on the development machine
(Intel HD 630) and saw "Acquiring a texture failed" in six runs of six. A full
draw of 10M rows takes about a second per frame there, so the most likely cause
is the surface's `get_current_texture` timing out while the previous frame is
still on the GPU. A user who opts out of level of detail
(`Selection::lod(Lod::Off)`) on a big layer, or whose GPU is slow, would see the
same error instead of a slow chart. Not investigated yet.

## User Story

> "As a user with a big layer and `Lod::Off`, I want a slow chart, not an
> error."

## Read First

- GUP-717's story (the observation) and GUP-716's (`Lod::Off`).
- The window host's frame loop in `crates/gup-core/src/app.rs` (surface acquire,
  `SurfaceError::Timeout`/`Outdated`/`Lost` handling) and `zoom_bench`'s own
  loop.

## Acceptance Criteria

- [ ] Reproduce it and record the exact error, the wgpu `SurfaceError` variant
      and where it is raised; if it does not reproduce, record the attempts and
      close the story.
- [ ] If it is a timeout on a slow frame: the window host retries or skips the
      frame on `SurfaceError::Timeout` (and reconfigures on `Outdated`/`Lost`)
      instead of failing, and logs it once rather than per frame. A test or a
      scripted run shows a deliberately slow frame (or 10M rows with `Lod::Off`)
      presenting without the error.
- [ ] If the cause is in `zoom_bench` only (not the library host), fix the bench
      and say so.
- [ ] CHANGELOG `[0.1.0]` Fixed line if the library changes.

## Decisions

- Never hide a real device loss: only `Timeout` is retried; `Lost` and
  `OutOfMemory` keep their current handling unless the investigation shows it is
  wrong.

## Escalation Triggers

- The error comes from GPU memory exhaustion rather than a timeout.

## Out of Scope

- Making a 10M-row full draw faster (that is what LOD is for).

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask merge-check`, `mask all-check`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
