---
implementer: sonnet
implementer-reason:
  one test helper changes from a fixed frame count to a deadline; no library
  code
touches: [crates/gup-core/tests/events.rs]
---

# GUP-698: GPU Pick Tests Wait For The Pick, Not For N Frames

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-09

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

- [ ] Every test that waits on a GPU pick waits on a deadline (10 s), not a
      frame count; the failure message says what was waited for and how long.
- [ ] No library code changes (if the test reveals a real bug, stop and report).
- [ ] Before/after failure counts for 20 iterations under parallel GPU load are
      recorded in the story.

## Decisions

- Poll with a short sleep between resolves; deadline 10 s.

## Escalation Triggers

- The pick never arrives even without load (a library bug).

## Out of Scope

- Library behaviour; non-pick tests.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix`, `mask all-check`, `mask merge-check` pass
- [ ] Story status updated in story file and INDEX.md
