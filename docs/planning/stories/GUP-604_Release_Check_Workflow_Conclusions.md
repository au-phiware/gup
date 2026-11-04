---
implementer: sonnet
implementer-reason: a script addition with a seeded test on canned JSON
touches: [scripts/, maskfile.md, docs/RELEASING.md]
---

# GUP-604: Release Check Reads the Workflow Conclusions

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-04

## Context

GUP-598's checklist asks the releaser to read the latest Tests, Lint, Visual
regression, Dogfood, Release check and Platforms runs by hand. A workflow can
report success while one of its jobs is red (the macOS job was advisory with
`continue-on-error`), so a check must read jobs, not only runs.

## User Story

> "As the maintainer, I want `release-check` to read the latest workflow job
> conclusions on `main`, so that release day has no surprises."

## Decisions

- A `ci` row: for `git rev-parse origin/main`,
  `gh run list --commit <sha> --json ...`, then `gh run view --json jobs` per
  run. PASS only if every workflow's latest run on that commit concluded success
  and the required jobs exist (Tests including feature-matrix, Lint, Visual
  regression, Dogfood, Gallery, WASM, Release check, Platforms windows and
  macos).
- Without `gh`, or unauthenticated, the row is SKIP with a one-line reason,
  never FAIL. Read-only: it never triggers, re-runs or cancels runs.
- `docs/RELEASING.md` replaces its hand-written "check CI is green" steps with
  "`mask release-check`'s `ci` row".
- Seeded cases in `scripts/test_release_check.sh` with a fake `gh` on PATH:
  success, one failed job, a missing job, `gh` absent.
- Safety (GUP-598's incident): tests unset `GIT_*`, use the scratch-directory
  git wrapper and keep the bare-repository guard. Never `cargo publish` without
  `--dry-run`.

## Acceptance Criteria

- [ ] A `workflows` check in `scripts/release_check.sh` reads
      `gh run list --branch main` and `gh run view --json jobs`, and fails when
      any workflow, or any job of one (including macOS), is not successful. It
      is SKIP without `gh` or the network.
- [ ] A seeded test feeds canned JSON, including a green workflow with a red
      job.
- [ ] `docs/RELEASING.md` step 1 drops the manual list.

## Dependencies

### Prerequisite Stories

- GUP-598 ✅

## Testing Strategy

Seeded: `scripts/test_release_check.sh`.

## Risk Assessment

- **Low**: a read-only addition to the release tooling.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` clean
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
