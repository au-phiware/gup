---
implementer: haiku
implementer-reason:
  add one shellcheck invocation to an existing mask task and fix what it finds
touches: [maskfile.md, scripts/]
---

# GUP-517: Shellcheck the Scripts in the Gate

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 🚧 In Progress
**Created**: 2026-10-24

## Context

`mask all-check` lints the workflow `run:` blocks through actionlint, which runs
shellcheck on them, but nothing in the gate runs shellcheck on `scripts/*.sh`.
GUP-502 wrote `scripts/merge_check.sh` and had to run shellcheck by hand. The
hook's bash scripts (`pre_commit.sh`, `checkpoint.sh`, `prune_build_dir.sh`,
`headless_env.sh`, and the rest) have the same gap.

## User Story

> "As a maintainer, I want every shell script in `scripts/` linted by the gate,
> so that a script bug is caught by `mask all-check` and not by a run."

## Acceptance Criteria

### AC1: The gate lints every script

- [ ] `mask all-check` runs `shellcheck` over `scripts/*.sh` and
      `dogfood/run_all.sh`, and fails on any finding.
- [ ] Each finding is fixed, or suppressed with a `# shellcheck disable=` line
      that gives the reason on the same line. No file-wide disables.
- [ ] The shellcheck version is the one pinned by the flake (run it from the dev
      shell, not from the host).

### AC2: Proof

- [ ] A seeded violation in one script fails `mask all-check`, then is reverted
      (recorded, not committed).

## Decisions

- **Same gate, no new tool**: shellcheck from the flake, invoked by `all-check`
  like the other lint steps. No CI workflow change.
- **Fix the findings, do not disable them**: a disable needs a reason on its
  line.

## Escalation Triggers

- A script finding needs a behaviour change, not a quoting or syntax fix.

## Out of Scope

- Linting bash inside Markdown or workflow files beyond what actionlint covers.
- Formatting shell scripts (shfmt).

## Technical Tasks

- [ ] Add the shellcheck step to `mask all-check`.
- [ ] Fix or justify each finding.
- [ ] Seeded proof (AC2).

## Dependencies

### Prerequisite Stories

- GUP-502: Merge Check Task ✅ (its script is the first one linted here).

## Testing Strategy

- **Manual**: the seeded violation (AC2).
- **Gate**: `mask all-check` passes on the clean tree.

## Success Metrics

- [ ] `mask all-check` fails on a seeded script violation and passes clean.

## Risk Assessment

- **Low**: shellcheck finds a real bug in an old script. _Mitigation_: fix it in
  this story if the fix is small, otherwise log a follow-up.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-check` passes
- [ ] Lint and format clean: `mask all-fix`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
