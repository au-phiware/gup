---
implementer: opus
implementer-reason:
  changes how a frame's draws are submitted (one pass versus several bounded
  submits) without changing the picture; needs measurement on the real GPU and a
  design for blending, MSAA resolve and the pick pass across submits
touches:
  [
    crates/gup-core/src/render.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/examples/zoom_bench.rs,
  ]
---

# GUP-738: A Very Long Draw Is Split So the Driver Does Not Reset the GPU

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-16

## Context

GUP-734 reproduced "Acquiring a texture failed" on the development machine
(Intel HD 630): with `Lod::Off`, a 10M-row full draw at 1080p with 4× MSAA is
one GPU pass of about 1.1 s, and the driver resets the device (device lost,
reason `Unknown`). Passes of 0.64 s (6M rows) and 0.86 s (8M) survive. GUP-734
made the error say the device was lost and why; it cannot keep the device. A
user who turns level of detail off on a large layer, or whose layer is not
LOD-eligible (size-ordered, polylines), on a slow GPU, loses the device instead
of getting a slow frame.

## User Story

> "As a user drawing every row of a huge layer on a slow GPU, I want a slow
> frame, not a lost device."

## Acceptance Criteria (draft; refine before Ready)

- [ ] Measure: the per-pass time at which this machine's driver resets the
      device, and whether splitting one pass into several submits of bounded
      work (e.g. by chunk, with `queue.submit` between) avoids it.
- [ ] If it does: draws past a measured per-submit budget are split across
      submits, the image is byte-identical to the single-pass image (blending,
      MSAA resolve, size order and the pick pass included), and the 10M
      `Lod::Off` run of GUP-734 presents.
- [ ] No cost when a frame fits in one submit (the common case): existing perf
      budgets hold.

## Open Questions

- Does MSAA force one render pass (resolve at the end), and can a multisampled
  target be continued across passes with `LoadOp::Load` without a cost that
  defeats the purpose?
- Is the budget per pass, per submit or per command buffer on Vulkan, Metal and
  DX12?

## Out of Scope

- Making the full draw itself faster (that is level of detail).

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask merge-check`, `mask all-check`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
