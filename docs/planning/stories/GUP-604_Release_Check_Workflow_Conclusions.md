---
implementer: sonnet
implementer-reason: a script addition with a seeded test on canned JSON
touches: [scripts/, maskfile.md, docs/RELEASING.md]
---

# GUP-604: Release Check Reads the Workflow Conclusions

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: ✅
Complete (2026-11-04) **Created**: 2026-11-04

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

- [x] A `workflows` check in `scripts/release_check.sh` reads
      `gh run list --branch main` and `gh run view --json jobs`, and fails when
      any workflow, or any job of one (including macOS), is not successful. It
      is SKIP without `gh` or the network.
- [x] A seeded test feeds canned JSON, including a green workflow with a red
      job.
- [x] `docs/RELEASING.md` step 1 drops the manual list.

## Dependencies

### Prerequisite Stories

- GUP-598 ✅

## Testing Strategy

Seeded: `scripts/test_release_check.sh`.

## Risk Assessment

- **Low**: a read-only addition to the release tooling.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-fix` clean
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `scripts/release_check.sh` has a `ci` check (the row is named `ci`, per the
  orchestrator's decisions, not `workflows`). For `origin/main`'s commit it runs
  `gh run list --commit <sha>`, takes the latest run of each workflow (Tests,
  Lint, Visual regression, Dogfood, Gallery, WASM Compilation Check, Release
  check, Platforms) and reads `gh run view <id> --json jobs`. It fails if a
  workflow has no run or its latest run is not a completed success, if a
  required job (the `required_jobs` list: the three Tests jobs including
  feature-matrix, Browser smoke, the publish dry run, docs.rs, Windows, macOS,
  and so on) is missing or not successful, or if any other job is red (skipped
  is fine). It is SKIP without `gh`, when `gh auth status` fails, or when `gh`
  cannot reach GitHub. It only runs `gh auth status`, `gh run list` and
  `gh run view`.
- `docs/RELEASING.md` step 1 drops the manual list and the macOS sub-step in
  favour of the `ci` row (the rule that macOS claims need a green job stays).
- `scripts/test_release_check.sh`: a fake `gh` serving canned JSON: green, a
  green workflow with a red macOS job, a missing job, a failed run, a missing
  run, an older failed run followed by a green one, `gh` absent, `gh` not
  authenticated, and a check that `gh` was asked only the three read commands.
  The scratch-git wrapper, the `GIT_*` unsetting and the bare-repository guard
  are unchanged.
- Evidence (`mask release-check` on this branch): all-check PASS 230 s, tests
  PASS 493 s, merge-check PASS 853 s, publish PASS 469 s, docs-rs PASS 91 s,
  one-version, seven, categories PASS, clean-tree FAIL (this worktree branch is
  not yet on or above `origin/main`; expected before the merge), ci PASS 15 s
  against the real repository.

## Retrospective

**Completed**: 2026-11-04

### Key Technical Learnings

#### Job names versus job ids

- **Challenge**: Some jobs have no `name:` (their name is the id, such as
  `build-gallery`), and the Tests names carry a suffix.
- **Solution**: Required jobs are matched by substring of the job name, listed
  in one `required_jobs` array in the script.
- **Pattern**: The list must follow `.github/workflows`; the test reads it from
  the script, so the fixture follows it, but a renamed job still needs the list
  updated.

#### Latest run per workflow

- **Challenge**: A re-run leaves an older failed run on the same commit.
- **Solution**: Take the latest by `createdAt` then `databaseId`; a seeded case
  covers an old failure followed by a pass.

### Architectural Decisions

#### SKIP, not FAIL, when GitHub is unreachable

- **Decision**: no `gh`, no login or no network is SKIP with a one-line reason.
- **Reasoning**: The orchestrator's decision; the row must not make an offline
  release check red.
- **Trade-off**: An unauthenticated releaser gets a SKIP and must read the runs
  by hand; `docs/RELEASING.md` says so.

### Development Workflow Insights

- A fake `gh` on PATH with a call log tests both the logic and the read-only
  rule. A PATH of symlinks to the needed tools, without `gh`, tests the absent
  case.

### Follow-up Stories

1. **GUP-606: Release check ci row for a worktree branch** — `clean-tree` fails
   in a worktree branch that is behind `origin/main`, and `ci` reads
   `origin/main`, not HEAD. Decide whether `--only` runs on a branch should read
   HEAD's runs. Only if worktree runs of `release-check` matter.
