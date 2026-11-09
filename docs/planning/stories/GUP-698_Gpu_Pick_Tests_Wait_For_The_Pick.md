---
implementer: sonnet
implementer-reason:
  one test helper changes from a fixed frame count to a deadline; no library
  code
touches: [crates/gup-core/tests/events.rs]
---

# GUP-698: GPU Pick Tests Wait For The Pick, Not For N Frames

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: ✅
Complete 2026-11-09 **Created**: 2026-11-09

## Context

`events::a_gpu_click_needs_a_press_and_release_within_four_pixels` failed with
"a pick was still in flight after 10 resolves" while other processes loaded the
GPU (5, 4, then 0 failures in three reruns). The shared helper `settle` in
`crates/gup-core/tests/events.rs` resolves at most 10 times with a 10 ms sleep
between, so it assumes the GPU answers within about 100 ms. CI runners are
shared; a timing-dependent test is a flake waiting to happen.

## User Story

> "As a maintainer, I want GPU-pick tests to wait until the pick arrives (or a
> generous deadline passes), so that load on the machine cannot fail them."

## Read First

- `crates/gup-core/tests/events.rs` (`settle`, the "GPU-picked hover and click"
  section).

## Acceptance Criteria

- [x] Every test that waits on a GPU pick waits on a deadline (10 s), not a
      frame count; the failure message says what was waited for and how long.
- [x] No library code changes (if the test reveals a real bug, stop and report).
- [x] Before/after failure counts for 20 iterations under parallel GPU load are
      recorded in the story.

## Decisions

- Poll with a short sleep between resolves; deadline 10 s.

## Escalation Triggers

- The pick never arrives even without load (a library bug).

## Out of Scope

- Library behaviour; non-pick tests.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-fix`, `mask all-check`, `mask merge-check` pass
- [x] Story status updated in story file and INDEX.md

## Implementation Summary

`settle` in `crates/gup-core/tests/events.rs` (the only helper that waits on a
GPU pick; 12 call sites) now resolves until `Chart::needs_frame` is false,
sleeping 2 ms between resolves, and panics after 10 s with "a GPU pick was still
in flight ... after it was submitted". No library code changed. The other
`needs_frame` uses (`append.rs`, `events.rs` assertions) are synchronous checks,
not waits. `events`: 40 passed.

Failure counts, 20 iterations of the `events` binary while 8 to 24 other copies
of it looped in parallel (8 cores): before 0/20 (24 loops, 16 min), after 0/20
(8 loops, 5 min).

## Retrospective

**Completed**: 2026-11-09

- The flake was not reproduced here, before or after. The original failures came
  while other agents' builds and GPU work shared the device; looping copies of
  the same small test binary did not starve the readback enough.
- The deterministic wait is still worth having: the old bound (about 100 ms) was
  a guess about the machine, and shared CI runners are slower and less even than
  this one. A deadline of 10 s turns a slow GPU into a slow test, and a real
  lost pick still fails, with a clear message.
- Follow-up: none.
