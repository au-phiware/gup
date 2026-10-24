---
implementer: haiku
implementer-reason:
  a check script with five exact rules, a seeded test in the existing throwaway
  style, and two one-line wirings
touches: [maskfile.md, scripts/, scripts/pre_commit.sh]
---

# GUP-504: Story ID Guard

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 🚧 In Progress
**Created**: 2026-10-24

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

- [ ] Usage: `scripts/check_story_ids.sh [--dir DIR]` (default
      `docs/planning/stories`). It reads the story files `DIR/GUP-*.md` and the
      entry lines (`^- \[GUP-…\](…)`) of `DIR/INDEX.md` and, if present,
      `DIR/INDEX_ARCHIVE.md`.
- [ ] It reports one line per problem and exits 1 if there is any, 0 otherwise
      (printing `story ids: OK (<n> stories)`), for these rules:
  1. every entry's link names a file that exists in `DIR`;
  2. for each ID above the baseline (number > 315): at most one story file;
  3. for each ID above the baseline: exactly one entry across both index files,
     and that entry links that ID's file;
  4. for each story file above the baseline: its first level-1 heading starts
     with `# GUP-NNN:` with the file's number;
  5. no story file has an ID with a letter suffix (`GUP-285B` style) above the
     baseline.
- [ ] On the real repository it passes (after GUP-503).

### AC2: seeded test

- [ ] `scripts/test_check_story_ids.sh` builds a `mktemp -d` stories directory
      (removed on exit) and asserts:
  - a valid set (two stories above the baseline, each with one entry, plus a
    legacy pair `GUP-013_A.md`/`GUP-013_B.md` with two entries) passes;
  - each of these fails, with the rule's message: a second file with an ID above
    the baseline; a second entry for the same ID (one in `INDEX.md`, one in
    `INDEX_ARCHIVE.md`); a story file with no entry; an entry linking a missing
    file; a heading whose number differs from the file name; `GUP-400B_X.md`.
- [ ] It runs in under 5 s.

### AC3: wired in

- [ ] `scripts/pre_commit.sh`: `./scripts/check_story_ids.sh` joins the
      always-run `checks` array, with name `ids` (`names='\s,gallery,ids'`), and
      the header comment's "these always run" sentence names it.
      `./scripts/test_pre_commit.sh` still passes unchanged.
- [ ] `mask all-check`: `./scripts/check_story_ids.sh` joins the `gallery` group
      (`'./scripts/check_gallery_sync.sh && ./scripts/check_story_ids.sh'`) and
      `./scripts/test_check_story_ids.sh` joins the `hook` group; the
      description names both.
- [ ] `scripts/merge_check.sh` gains a first step, `story-ids`
      (`./scripts/check_story_ids.sh`), shown in its table.
- [ ] Seeded proof, recorded and not committed: copying a story file to a new
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

- [ ] Start commit within 30 minutes.
- [ ] Write `scripts/check_story_ids.sh` and its test; run on the real
      repository; commit.
- [ ] Wire into `pre_commit.sh`, `all-check` and `merge_check.sh`.
- [ ] Run the seeded commit by hand; `mask all-check`.

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

- [ ] A duplicate ID above GUP-315 fails the hook, `all-check` and
      `merge-check`.
- [ ] The real repository passes.

## Risk Assessment

- **Low**: a story file named oddly (lower-case, extra suffix) trips a rule.
  _Mitigation_: rules apply only above the baseline, where names are uniform
  (GUP-388 onward).

## Definition of Done

- [ ] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-check` passes; `./scripts/test_pre_commit.sh` passes
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser`: not needed (scripts only); say so
- [ ] WASM size: "unchanged: no library code change"
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
