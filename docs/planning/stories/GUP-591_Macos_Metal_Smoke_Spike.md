---
implementer: sonnet
implementer-reason:
  a spike job beside GUP-590's Windows job, with a decision rule for each
  outcome; no library code
touches: [.github/workflows/]
---

# GUP-591: macOS (Metal) Smoke Spike

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-02

## Context

Track N3 asks for a macOS render job "if a runner GPU works". GitHub's standard
arm64 macOS runners are virtual machines. Reports from other wgpu projects say
they expose an "Apple Paravirtual device" Metal adapter with reduced reported
capabilities, and that GPU tests on it sometimes fail. The `-xlarge` runners
advertise GPU acceleration, but they are billed, which is an owner decision. No
one has tried Gup on Metal.

[GUP-590](GUP-590_Windows_Warp_Render_Smoke_Job.md) adds
`.github/workflows/platforms.yml`: a Windows job that prints the adapter
(`tests/platform_adapter.rs`) and runs `cargo test -p gup --tests`. This story
adds the same job on macOS and settles, by a rule, whether it stays.

## Read First

- `.github/workflows/platforms.yml` (from GUP-590) and
  `tests/platform_adapter.rs`.
- GUP-590's Decisions: golden failures are reported, never tolerated away.

## User Story

> "As a macOS developer, I want CI to draw Gup's charts on Metal if a hosted
> runner can, so that a regression on my platform is caught."

## Acceptance Criteria

### AC1: Spike

- [ ] `platforms.yml` gains a `macos` job on `macos-15`, with the Windows job's
      steps, `WGPU_BACKEND: metal` in place of `dx12`, and
      `tests/platform_adapter.rs` asserting `Metal` when that variable is set.
      Upload artifacts as `macos-golden-diffs`.
- [ ] Push it and record the adapter's name, device type, and the result of the
      smoke step, case by case, in the Implementation Summary.

### AC2: The rule applied

- [ ] Exactly one outcome of the rule in Decisions is applied, and the
      Implementation Summary says which.
- [ ] `actionlint .github/workflows/*.y*ml` passes.

## Decisions

- **Runner**: `macos-15`, standard size (free for public repositories).
  `-xlarge` runners are not used; see Out of Scope.
- **Rule**:
  - **A Metal adapter exists and every test passes**: the job stays as a
    required job, like Windows.
  - **A Metal adapter exists, but some goldens fail beyond tolerance while every
    structural check passes**: the job stays with `continue-on-error: true` and
    a comment naming the failing cases. Write a follow-up story ("macOS goldens
    on the paravirtual device") with the `DiffStats` and diff images, read by
    eye. Do not change tolerances or goldens.
  - **No Metal adapter, device creation fails, or a non-golden test fails**:
    remove the job. Leave a comment in `platforms.yml` with the date, the runner
    image, and the error. Write the follow-up story the same way.
- **The README** is GUP-597's job: it reads this story's outcome.

## Escalation Triggers

- A failure points at library code: a Metal validation error, or naga's MSL
  translation rejecting a shader. Report it with the error. It is a real
  platform bug worth the orchestrator's attention before 0.1.

## Out of Scope

- Billed `-xlarge` runners or a self-hosted Mac: an owner step if the standard
  runner cannot draw.
- Windows: GUP-590.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] The job; push; record (AC1). Commit.
- [ ] Apply the rule; follow-up story if one is needed (AC2). Commit.

## Dependencies

### Prerequisite Stories

- GUP-590: Windows (WARP) render smoke job 📋 — `platforms.yml` and the adapter
  test.

### Enables Stories

- GUP-597: README for release 0.1 — says which platforms CI draws on.

## Testing Strategy

- **CI**: the `macos` job's outcome, under the rule.
- **Visual validation**: diff images read by eye if any golden fails.

## Success Metrics

- [ ] It is known, with evidence, whether hosted macOS runners can test Gup.

## Risk Assessment

- **Medium**: the paravirtual device passes some runs and fails others.
  _Mitigation_: run the job twice before applying the first outcome of the rule.
  Record both runs.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: diff images read if any golden failed
- [ ] `mask wasm-browser`: not needed; no layout, input or wasm code changes
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
