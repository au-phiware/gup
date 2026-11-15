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

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-16) **Created**: 2026-11-16

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

- [x] Reproduce it and record the exact error, the wgpu `SurfaceError` variant
      and where it is raised; if it does not reproduce, record the attempts and
      close the story.
- [x] If it is a timeout on a slow frame: the window host retries or skips the
      frame on `SurfaceError::Timeout` (and reconfigures on `Outdated`/`Lost`)
      instead of failing, and logs it once rather than per frame. A test or a
      scripted run shows a deliberately slow frame (or 10M rows with `Lod::Off`)
      presenting without the error.
- [x] If the cause is in `zoom_bench` only (not the library host), fix the bench
      and say so.
- [x] CHANGELOG `[0.1.0]` Fixed line if the library changes.

## Decisions

- Never hide a real device loss: only `Timeout` is retried; `Lost` and
  `OutOfMemory` keep their current handling unless the investigation shows it is
  wrong.

## Escalation Triggers

- The error comes from GPU memory exhaustion rather than a timeout.

## Out of Scope

- Making a 10M-row full draw faster (that is what LOD is for).

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask merge-check`, `mask all-check`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Investigation (2026-11-16)

Reproduced on the Intel HD 630 (Mesa 26.0.0, Vulkan) with
`zoom_bench --columns --lod off --points 10000000 --size 1920x1080 --samples 4 --present mailbox --uncapped --static`:
acquire #0 works, acquire #1 fails, every time. 10M rows with Fifo at 800x600 or
1080p (paced to 1 fps by the compositor) did not fail.

- **Error**: `wgpu::SurfaceError::Other` ("Acquiring a texture failed with a
  generic error"), the library's
  `Error::Configuration("window target", "no frame: ...")`.
- **Where**: wgpu-core's `Surface::get_current_texture` returns an `Err` when
  `present.device.check_is_valid()` fails; the wgpu front end maps any such
  error to `SurfaceStatus::Unknown`, which is `SurfaceError::Other`. It is not a
  `Timeout` (that is `Ok(None)` from the hal acquire).
- **Cause**: the device was lost. A `set_device_lost_callback` reports reason
  `Unknown`, "Unexpected error variant (driver implementation is at fault)": the
  first frame's single GPU pass (about 1.1 s) is reset by the driver. Same
  flags, GPU pass medians: 6M rows 0.64 s, 8M 0.86 s (both run), 10M fails.
- Not memory exhaustion (escalation trigger) and not a timeout.

## Implementation Summary

The premise (a timeout) was wrong, and the Decision says never to hide a device
loss, so nothing retries it. Changes:

- `Context` records the device-lost reason (devices Gup created) and
  `WindowTarget::acquire` reports `Error::Gpu` "the GPU device was lost (...); a
  GPU batch that runs for seconds is killed by the driver ... (see `Lod`)".
- The acquire became `WindowTarget::acquire_texture`: reconfigure once on
  `Outdated`/`Lost`, retry `Timeout` up to 4 times (logged once), else error.
- Test `scope::tests::a_lost_device_records_its_reason`; CHANGELOG Fixed;
  PERF_BUDGETS note. `zoom_bench` itself is unchanged: it is not at fault.
- The retry is not exercised by a test (a surface cannot be made to time out
  headlessly); the lost-device path was shown on the real GPU (output above).

## Retrospective

**Completed**: 2026-11-16

### Key Technical Learnings

#### wgpu reports a lost device as `SurfaceError::Other`

- **Challenge**: "Acquiring a texture failed with a generic error" names
  nothing; no scope or uncaptured handler saw it.
- **Solution**: read wgpu-core's `get_current_texture` (device validity check
  first), then a `set_device_lost_callback` showed the reason.
- **Pattern**: when a wgpu call fails generically, install the device-lost
  callback before guessing at timeouts.

### Architectural Decisions

#### No retry for a lost device

- **Decision**: only `Timeout` retries; a lost device is reported with its
  reason.
- **Trade-off**: `Lod::Off` at 10M rows on this GPU still fails; it fails
  clearly.
- **Future**: a pass over about 1 s is reset by the driver here. Splitting one
  long pass is a design change (out of scope: "making a 10M-row full draw
  faster").

### Development Workflow Insights

- The repro needed GUP-717's exact flags (Mailbox, uncapped, 1080p, MSAA 4); the
  default Fifo run is paced to 1 fps and passes.

### Follow-up Stories

None: the GPU-batch limit is what level of detail is for.
