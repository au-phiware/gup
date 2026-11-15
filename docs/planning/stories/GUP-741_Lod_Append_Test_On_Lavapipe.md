---
implementer: sonnet
implementer-reason:
  a test whose assumption is found and replaced; no library change expected
touches: [crates/gup-core/tests/lod.rs]
---

# GUP-741: The LOD Append Test Passes on Lavapipe

## Story Overview

**Initiative**: Strategic Review 2026-11 (CI) **Status**: ✅ Complete 2026-11-16
**Created**: 2026-11-16

## Context

`appends_that_grow_the_extent_leave_full_chunks_alone` (GUP-719) passes on the
Intel HD 630 and fails on lavapipe, on GitHub CI and locally: "668 of 970 pixels
changed" in the old corner. GUP-737 recorded the same test differing by one
pixel row on the HD 630.

## User Story

> "As a maintainer, I want the LOD append test to pass on every device CI uses,
> so that main stays green."

## Acceptance Criteria

- [x] The reason lavapipe differs is found and written down.
- [x] The test asserts a property that holds on every device, without loosening
      a bound.
- [x] It passes on lavapipe and on the default device, as does all of
      `tests/lod.rs`.

## Decisions

- If the cause is deliberate behaviour of the plot, the fix is in the test.

## Escalation Triggers

- Full chunks rebuild or draw differently after an append: report; it is a
  library bug.

## Out of Scope

- The edge solve itself (GUP-567).

## Definition of Done

- [x] Test passes on both devices; `mask all-check` and `mask merge-check` pass.

## Implementation Summary

Instrumented both devices: instance counts (1,600 then 11,600), chunks (2, 3),
draw calls (4, 5), tier builds (2) and the plot rect are identical on both. The
difference is in the scene: after the append the x axis rule ends at 376.5
instead of 378 and the y axis starts at 17.49 instead of 16. The new rows reach
the domain ends (x 5,000, y 1,000), so the edge solve (GUP-567) insets the scale
ranges to keep those marks inside, and every old mark and every tick label moves
by a sub-pixel amount (labels change by up to 204 per channel on both devices).
The HD 630 shows it only on the antialiased cloud edge (98 pixels in the
corner); lavapipe's rasteriser changes 697 through the dense cloud. It is
deliberate behaviour, not an LOD bug, and the cause of GUP-737.

The test now ends its new rows at x 4,500 and y 900, clear of the domain ends,
and asserts the old corner is byte for byte unchanged (0 differences; it was "at
most two pixel rows"). With that, the whole picture outside the new cloud,
labels included, is identical on both devices.

## Retrospective

**Completed**: 2026-11-16

### Key Technical Learnings

#### A pixel test that moves the data extent moves the scales

- **Challenge**: Same instances, tiers and builds, different pixels, mostly on
  one device.
- **Solution**: Diffing the non-mark scene items between frames showed the axis
  rules, and so the scale ranges, changed.
- **Pattern**: When pictures differ, compare the scene before the pixels.

### Architectural Decisions

#### Test the unchanged corner with the edge solve idle

- **Decision**: Keep new rows off the domain ends; assert zero differences.
- **Reasoning**: It tests the property (old chunks draw the same) exactly.
- **Trade-off**: The edge-solve shift under appends is not exercised here;
  GUP-737 decides whether a streaming chart whose marks reach a domain end may
  shimmer by a sub-pixel.

### Follow-up Stories

None; GUP-737 now has its cause.
