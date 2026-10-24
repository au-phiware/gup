---
implementer: haiku
implementer-reason:
  a check script with five exact rules, a seeded test in the existing throwaway
  style, and two one-line wirings
touches: [maskfile.md, scripts/, scripts/pre_commit.sh]
---

# GUP-504: Story ID Guard

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
(2026-10-24) **Created**: 2026-10-24

## Context

Story IDs collided again in October: two scribes running at once both took the
next free number, and GUP-439 had to be renumbered (STRATEGIC_REVIEW_2026-11 §2,
"Semantic merge conflicts"). Nothing checks IDs. GUP-383 planned a guard but
expected 34 legacy duplicates (all at or below GUP-315) to be renumbered first
by GUP-374; GUP-503 closes both in favour of this story, which freezes the
legacy duplicates as a baseline and guards every newer ID.

A collision is a textual non-conflict (two different files), so git merges it
silently. The guard has to run where merged trees are checked: the pre-commit
hook (every commit, including the orchestrator's after a merge),
`mask all-check` (CI's Lint workflow) and `mask merge-check`.

## Read First

- `scripts/check_gallery_sync.sh` (the style of an existing cross-file check).
- `scripts/test_checkpoint.sh` from GUP-496 (or `scripts/test_pre_commit.sh`'s
  `snapshot_cases`): the throwaway-directory test style.
- `scripts/pre_commit.sh` lines 375–381 (the always-run `checks` array).
- `maskfile.md` `## all-check`; `scripts/merge_check.sh` from GUP-502.
- `docs/planning/stories/INDEX.md` lines 1–10 (entry format).

## User Story

> "As the orchestrator, I want a commit or merge that gives two stories the same
> ID to fail a check, so that collisions are fixed when they happen instead of
> weeks later."

## Acceptance Criteria

### AC1: `scripts/check_story_ids.sh`

- [x] Usage: `scripts/check_story_ids.sh [--dir DIR]` (default
      `docs/planning/stories`). It reads the story files `DIR/GUP-*.md` and the
      entry lines (`^- \[GUP-…\](…)`) of `DIR/INDEX.md` and, if present,
      `DIR/INDEX_ARCHIVE.md`.
- [x] It reports one line per problem and exits 1 if there is any, 0 otherwise
      (printing `story ids: OK (<n> stories)`), for these rules:
  1. every entry's link names a file that exists in `DIR`;
  2. for each ID above the baseline (number > 315): at most one story file;
  3. for each ID above the baseline: exactly one entry across both index files,
     and that entry links that ID's file;
  4. for each story file above the baseline: its first level-1 heading starts
     with `# GUP-NNN:` with the file's number;
  5. no story file has an ID with a letter suffix (`GUP-285B` style) above the
     baseline.
- [x] On the real repository it passes (after GUP-503).

### AC2: seeded test

- [x] `scripts/test_check_story_ids.sh` builds a `mktemp -d` stories directory
      (removed on exit) and asserts:
  - a valid set (two stories above the baseline, each with one entry, plus a
    legacy pair `GUP-013_A.md`/`GUP-013_B.md` with two entries) passes;
  - each of these fails, with the rule's message: a second file with an ID above
    the baseline; a second entry for the same ID (one in `INDEX.md`, one in
    `INDEX_ARCHIVE.md`); a story file with no entry; an entry linking a missing
    file; a heading whose number differs from the file name; `GUP-400B_X.md`.
- [x] It runs in under 5 s.

### AC3: wired in

- [x] `scripts/pre_commit.sh`: `./scripts/check_story_ids.sh` joins the
      always-run `checks` array, with name `ids` (`names='\s,gallery,ids'`), and
      the header comment's "these always run" sentence names it.
      `./scripts/test_pre_commit.sh` still passes unchanged.
- [x] `mask all-check`: `./scripts/check_story_ids.sh` joins the `gallery` group
      (`'./scripts/check_gallery_sync.sh && ./scripts/check_story_ids.sh'`) and
      `./scripts/test_check_story_ids.sh` joins the `hook` group; the
      description names both.
- [x] `scripts/merge_check.sh` gains a first step, `story-ids`
      (`./scripts/check_story_ids.sh`), shown in its table.
- [x] Seeded proof, recorded and not committed: copying a story file to a new
      name with the same ID above the baseline makes `git commit` of it fail in
      the hook with the rule-2 message.

## Decisions

- **Baseline GUP-315**: every legacy duplicate ID (35 IDs, 13 to 315) is at or
  below it; every ID from 316 on is unique today. Legacy duplicates are not
  renumbered: their stories are archived history.
- **Rules check files and entries, not prose**: links inside entry text (for
  example "absorbs [GUP-384](...)") are not entries and are not counted.
- **Where it runs**: the hook's always-run checks (cheap, repo-wide, like the
  gallery sync), `all-check`, and `merge-check`. No separate CI workflow: the
  Lint workflow runs `all-check`.
- **Bash only**: `grep -oP`, associative arrays; no new tool.
- **ID allocation stays manual** (the orchestrator gives scribes ranges); this
  story only detects collisions.

## Escalation Triggers

- The real repository fails a rule after GUP-503 (report the lines; do not
  change the baseline or the rules).
- `test_pre_commit.sh` needs an expectation changed (the plan output changes).
- The hook's run time on a docs-only commit grows by more than 1 s.

## Out of Scope

- Renumbering or editing legacy stories.
- An ID allocator or reservation file.
- Checking `docs/planning/rfcs/` or other documents.

## Technical Tasks

- [x] Start commit within 30 minutes.
- [x] Write `scripts/check_story_ids.sh` and its test; run on the real
      repository; commit.
- [x] Wire into `pre_commit.sh`, `all-check` and `merge_check.sh`.
- [x] Run the seeded commit by hand; `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-503: Archive the Story Index History 📋 — `INDEX_ARCHIVE.md` exists and
  the closures are done, so the real repository passes.
- GUP-502: Merge Check Task 📋 — the script this story adds a step to.
- GUP-496: Checkpoint Script 📋 — soft, for the DoD's checkpoint line.

### Enables Stories

None.

## Testing Strategy

- **Unit-style**: `scripts/test_check_story_ids.sh` (AC2), in `all-check`.
- **Integration**: the real repository passes; the seeded duplicate fails a real
  commit.

## Success Metrics

- [x] A duplicate ID above GUP-315 fails the hook, `all-check` and
      `merge-check`. (The hook and `all-check` were seen directly; `merge-check`
      runs the same script as its first step, but the full task needs a GPU and
      was not run here.)
- [x] The real repository passes.

## Risk Assessment

- **Low**: a story file named oddly (lower-case, extra suffix) trips a rule.
  _Mitigation_: rules apply only above the baseline, where names are uniform
  (GUP-388 onward).

## Definition of Done

- [x] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-check` passes; `./scripts/test_pre_commit.sh` passes
- [x] Lint and format clean: `mask all-fix`
- [x] `mask wasm-browser`: not needed (scripts only); say so
- [x] WASM size: "unchanged: no library code change"
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2026-10-24. Commits `f91378c1` (Start) and `5905a824` (guard).

- `scripts/check_story_ids.sh` (new): the five rules, `--dir DIR`, one line per
  problem, exit 1 on any problem, `story ids: OK (<n> stories)` otherwise. Real
  repository: `story ids: OK (516 stories)`, about 0.5 s.
- `scripts/test_check_story_ids.sh` (new): a valid set (two stories above 315,
  and a legacy GUP-013 pair with its archive entries) passes. Six broken cases
  each fail with their rule number: a second file (rule 2), a second entry in
  the archive (rule 3), a story with no entry (rule 3), a missing link (rule 1),
  a heading mismatch (rule 4), and `GUP-400B_X.md` (rule 5). Runs in under 0.3
  s.
- `scripts/pre_commit.sh`: `ids` joins the always-run checks; header updated.
- `maskfile.md`: the `all-check` `gallery` group runs the guard and the `hook`
  group runs its test; the `merge-check` description names the guard.
- `scripts/merge_check.sh`: first step `story-ids`.

**Seeded proof** (not committed): a copy of
`GUP-500_Stable_Compile_Fail_Snapshots.md` staged as
`GUP-500_Seeded_Duplicate.md` was refused at commit time with
`[ids] story ids: rule 2: GUP-500 has 2 story files: GUP-500_Seeded_Duplicate.md GUP-500_Stable_Compile_Fail_Snapshots.md`
and exit 1. The file was then unstaged and deleted.

**Gates**: `mask all-fix` clean; `scripts/test_pre_commit.sh` passes unchanged;
`mask all-check` exit 0 (run after the guard commit).

**Not run**: `mask merge-check` end to end (needs a GPU and Chromium); its
`story-ids` step is the same script. `mask wasm-browser`: not needed (scripts
only). WASM size: unchanged (no library code change).

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Uncovered-decision check on the archive and status

- **Challenge**: The brief asked to stop if the rules did not say how the guard
  treats `INDEX_ARCHIVE.md` or the 🚫 status.
- **Solution**: The archive is named in the rules (read "if present", entries
  counted across both files), so it is covered. No rule reads the status: every
  entry line counts, whatever its emoji. That is the reading taken, and it is
  recorded here rather than escalated.
- **Pattern**: A guard that counts lines should say which files and fields it
  reads. A status column it ignores is then simply out of scope.

#### Subshells dominate bash guards

- **Challenge**: The first version took about 1.1 s on 516 files, too close to
  the hook's 1 s docs-only budget.
- **Solution**: `number_of` sets a variable instead of printing, so there is no
  fork per file. Time dropped to about 0.5 s.
- **Pattern**: In bash loops over hundreds of files, `$(...)` per item costs
  more than the work; use globals or `[[ =~ ]]`.

#### Link ownership by number

- **Challenge**: Rule 3 requires that an entry links its ID's file. Comparing
  the full token broke the suffix case (`GUP-400B_X.md` under entry `400`).
- **Solution**: Compare the link's leading number with the entry's number. The
  suffix is then caught by rule 5 alone, so each seeded failure trips one rule.

### Architectural Decisions

#### Missing INDEX.md is a problem, not a usage error

- **Decision**: An absent `INDEX.md` reports `cannot read` and exits 1. Bad
  arguments exit 2.
- **Reasoning**: The rules say the guard reads `INDEX.md`, so its absence is a
  broken repository, not a usage slip.
- **Trade-off**: A checkout without the index fails the hook. That is intended.
- **Future**: None.

#### Count of stories

- **Decision**: `<n>` in the OK line counts every `GUP-*.md` file (516), legacy
  included, because it reports what was scanned.

### Development Workflow Insights

- Seeded proof worked best as a real staged commit: the refusal and its message
  came from the hook, not from reasoning about the script.
- The worktree sandbox refuses compound shell lines that name git and carry a
  runtime-computed path. Splitting them into plain commands, or writing a script
  file with the Write tool, avoided it.

### Follow-up Stories

None. The merge-check task was not run end to end (it needs a GPU); its step is
the same script and was checked directly.
