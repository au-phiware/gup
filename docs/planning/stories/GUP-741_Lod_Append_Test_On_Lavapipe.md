---
implementer: sonnet
implementer-reason:
  a test whose assumption is found and replaced; no library change expected
touches: [crates/gup-core/tests/lod.rs]
---

# GUP-741: The LOD Append Test Passes on Lavapipe

## Story Overview

**Initiative**: Strategic Review 2026-11 (CI) **Status**: 🚧 In Progress
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

- [ ] The reason lavapipe differs is found and written down.
- [ ] The test asserts a property that holds on every device, without loosening
      a bound.
- [ ] It passes on lavapipe and on the default device, as does all of
      `tests/lod.rs`.

## Decisions

- If the cause is deliberate behaviour of the plot, the fix is in the test.

## Escalation Triggers

- Full chunks rebuild or draw differently after an append: report; it is a
  library bug.

## Out of Scope

- The edge solve itself (GUP-567).

## Definition of Done

- [ ] Test passes on both devices; `mask all-check` and `mask merge-check` pass.
