# GUP-383: Duplicate Story ID Guard

## Story Overview

**Initiative**: Project Maintenance **Status**: 📋 Planned **Created**:
2026-10-04

## Context

An audit of `docs/planning/stories/` (see
[GUP-374](GUP-374_Duplicate_Story_ID_Cleanup.md)) found 34 story IDs reused
across 41 files, including one ad-hoc workaround (`GUP-285B` in INDEX.md) that
papered over a collision instead of resolving it. Nothing currently prevents a
new story from claiming an ID that's already in use — a contributor drafting a
new story from a stale checkout of INDEX.md, or copy-pasting an existing story
as a template without updating its number, can silently reintroduce the same
problem that GUP-374 cleans up.

This story adds a scripted guard that fails fast when two story files share an
ID, or when INDEX.md references the same ID twice. The project already has
precedent for this kind of hygiene check: `scripts/check_gallery_sync.sh` and
`scripts/check_gallery_links.sh` validate cross-file consistency for the
examples gallery and are wired into `mask` tasks and a CI workflow
(`.github/workflows/gallery.yml`). This story follows the same pattern for story
IDs.

This story is independent of GUP-374's renumbering work and can be implemented
first. Until GUP-374's renumbering lands, the guard is expected to report
failures — that's correct behavior, not a bug, and documents the outstanding
cleanup. The guard should not be made to pass by weakening it; the existing
duplicates should be tracked as a known-failing baseline (or the check added to
CI only after GUP-374 lands, if a maintainer prefers no interim CI failures —
see AC3 for both options).

## User Story

> "As a project maintainer, I want CI to fail when a new story file accidentally
> reuses an existing GUP ID, so duplicate IDs cannot be reintroduced after
> GUP-374 cleans up the current ones."

## Acceptance Criteria

### AC1: Script detects duplicate IDs

- [ ] A script (e.g. `scripts/check_story_ids.sh`) identifies "story files" as
      files directly under `docs/planning/stories/` whose first line matches
      `^# GUP-[0-9]+:`. This correctly excludes non-story artifacts like
      `GUP-096_performance_report.md` (whose first line is a prose heading, not
      a `# GUP-NNN:` story header) while still catching real duplicates like the
      two `GUP-065_*.md` files.
- [ ] For each story file, the script extracts the ID from (a) the filename
      prefix and (b) the `# GUP-NNN:` heading, and fails with a clear message if
      they disagree for a given file.
- [ ] The script fails with a clear message (listing every offending ID and the
      files involved) if any ID is claimed by more than one story file.
- [ ] The script fails if `docs/planning/stories/INDEX.md` contains the same
      `[GUP-NNN]` bracketed reference pointing at two different target files.
- [ ] The script exits 0 with no output when there are no duplicates.

### AC2: Wired into `mask`

- [ ] A new `mask` task (e.g. `mask check-story-ids`) runs the script.
- [ ] The task is documented in `maskfile.md` following the existing task doc
      format (see `validate-marks` for a minimal example).

### AC3: CI integration

- [ ] A CI workflow step runs `mask check-story-ids` (or the script directly).
      Given the current 34 known duplicates (tracked by GUP-374), choose one of:
  - (a) add the CI step now and mark it `continue-on-error: true` /
    allow-failure until GUP-374 lands, then remove that flag in GUP-374's
    Definition of Done, or
  - (b) land this story's script and mask task without enabling it in CI, and
    add the CI step as part of GUP-374's Definition of Done once the repository
    is actually duplicate-free. Document which option was chosen and why in the
    Implementation Summary.

### AC4: Guard is self-validating

- [ ] Running the script against the current (pre-GUP-374)
      `docs/planning/stories/` directory reports all 34 duplicate groups.
- [ ] A unit/integration test (e.g. a small fixture directory, or a shell test
      in `scripts/test_perf_scripts.sh`-style harness) proves the script
      correctly distinguishes a real duplicate from a look-alike non-story file
      (regression test for the GUP-096 exclusion rule in AC1).

## Technical Tasks

- [ ] Write `scripts/check_story_ids.sh` (bash, following the header/comment
      style of `scripts/check_gallery_sync.sh`: `set -euo pipefail`, a usage
      comment block, `SCRIPT_DIR`/`REPO_ROOT` resolution).
- [ ] Implement story-file detection via first-line `^# GUP-[0-9]+:` match.
- [ ] Implement filename-prefix vs. heading-ID consistency check.
- [ ] Implement duplicate detection across story files (by filename-prefix ID).
- [ ] Implement duplicate detection within INDEX.md (by bracketed `[GUP-NNN]`
      reference).
- [ ] Add a `check-story-ids` task to `maskfile.md`.
- [ ] Add a CI workflow step per the AC3 decision.
- [ ] Add a small regression test fixture or inline test demonstrating the
      GUP-096-style exclusion and a genuine duplicate both behave correctly.

## Dependencies

### Prerequisite Stories

None — the script can be written and tested against the current (duplicate-
containing) backlog immediately.

### Enables Stories

- GUP-374 📋 (Duplicate Story ID Cleanup) — once GUP-374's renumbering is
  complete, this guard's CI step (if deferred per AC3 option (b), or its
  allow-failure flag if option (a)) should be finalized so duplicates are
  rejected going forward.

## Testing Strategy

- **Script test**: run `scripts/check_story_ids.sh` against the real
  `docs/planning/stories/` directory before GUP-374 lands and confirm it reports
  exactly the 34 groups documented in GUP-374's inventory table (no more, no
  fewer — a mismatch indicates a bug in the detection logic).
- **Regression fixture**: a temporary directory with (a) two files sharing a GUP
  ID and (b) one file with a GUP-prefixed filename but a non-story first line,
  asserting the script flags (a) and ignores (b).
- **Idempotence**: run the script twice in a row; output must be identical.

## Success Metrics

- [ ] `mask check-story-ids` runs in under a few seconds (pure text processing,
      no build required).
- [ ] The script correctly reports zero duplicates once GUP-374 lands (verify by
      re-running after that story is complete).

## Risk Assessment

- **Low**: This is a small, self-contained shell script with clear existing
  precedent (`scripts/check_gallery_sync.sh`). The main risk is over- or
  under-matching "story files" — mitigated by the explicit `^# GUP-[0-9]+:`
  heading rule and its regression test (AC4).
- **Low**: Choosing between AC3 options (a) and (b) is a judgment call left to
  whoever implements this story; either is acceptable, but should be recorded so
  GUP-374 knows what remains to be finalized.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
