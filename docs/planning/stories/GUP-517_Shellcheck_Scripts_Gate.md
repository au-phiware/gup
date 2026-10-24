---
implementer: haiku
implementer-reason:
  add one shellcheck invocation to an existing mask task and fix what it finds
touches: [maskfile.md, flake.nix, scripts/]
---

# GUP-517: Shellcheck the Scripts in the Gate

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
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

- [x] `mask all-check` runs `shellcheck` over `scripts/*.sh` and
      `dogfood/run_all.sh`, and fails on any finding.
- [x] Each finding is fixed, or suppressed with a `# shellcheck disable=` line
      that gives the reason on the same line. No file-wide disables.
- [x] The shellcheck version is the one pinned by the flake (run it from the dev
      shell, not from the host).

### AC2: Proof

- [x] A seeded violation in one script fails `mask all-check`, then is reverted
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

## Resolved (2026-10-24)

Orchestrator approved the flake.nix pin: `shellcheck` added to the dev shell
beside `actionlint` (one line; `touches:` extended to `flake.nix`). The dev
shell now resolves it to
`/nix/store/i5kmqg3q40a7n5v0ax06rh22l9qp3xhm-shellcheck-0.11.0-bin/bin/shellcheck`,
version 0.11.0. `mask all-check` inside `nix develop` exits 0, which covers CI's
Lint workflow.

## Implementation Summary

- `maskfile.md`: new `sh` branch in `all-check` running
  `shellcheck scripts/*.sh dogfood/run_all.sh`, plus a paragraph describing it.
- `flake.nix`: `shellcheck` in the dev shell.
- Nine findings fixed in four scripts (see retrospective); two targeted disables
  with reasons.
- AC2 proof: a seeded `echo $GUP517_SEED` in `scripts/check_story_ids.sh` made
  `mask all-check` exit 1 (`sh` branch, SC2086/SC2317). Reverted.
- Tests: `scripts/test_checkpoint.sh` passes after the quoting change; the other
  gate tests pass in `mask all-check`.

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Shellcheck in the dev shell

- **Challenge**: nixpkgs' actionlint wraps shellcheck by store path, so the dev
  shell had no `shellcheck` on PATH and `command -v shellcheck` found the host's
  copy.
- **Solution**: list `shellcheck` in the dev shell's `buildInputs` beside
  `actionlint`.
- **Pattern**: check `command -v` inside `nix develop`, not the host, before
  claiming a tool is pinned.

#### Fixing findings without changing behaviour

- **Fixed by rewrite**: SC2001 `sed 's|.*blob/main/||'` became
  `${link##*blob/main/}` (greedy, same match; checked on four sample inputs).
  SC2016 single-quoted `bash -c` strings became double-quoted with escaped `$`
  and backticks, passing the same string to the child shell.
- **Suppressed**: SC2207 (`IFS=$'\n' arr=($(sort ...))`). A `mapfile` rewrite
  would turn an empty list into one empty entry. SC2001 on a per-line
  `sed 's/^/  - /'`, which `${var//}` cannot express.
- **Pattern**: a rewrite is allowed only when a sample test shows identical
  output, including the empty case.

### Architectural Decisions

#### Targeted disables over rewrites when the rewrite changes edge cases

- **Decision**: two `# shellcheck disable=` lines, each with its reason on the
  same line.
- **Reasoning**: rewrites changed the empty-list case or per-line anchoring.
- **Trade-off**: two suppressions remain in the scripts.
- **Future**: revisit SC2207 if the gallery's example list ever needs empty
  handling.

### Development Workflow Insights

- A `Blocked` escalation for a one-line scope question worked: the orchestrator
  approved it, and the approval was recorded in the story.
- The seeded violation proved the gate fails on a real finding before the
  revert.

### Follow-up Stories

None. GUP-521 and GUP-522 were reserved but not needed.
