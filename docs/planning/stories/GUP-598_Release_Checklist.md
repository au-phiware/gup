---
implementer: sonnet
implementer-reason:
  one script that runs existing gates and a few consistency checks fixed in
  Decisions, plus a checklist document; ends before anything is published
touches: [maskfile.md, scripts/, docs/RELEASING.md, docs/README.md]
---

# GUP-598: Release Checklist

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 📋 Planned
**Created**: 2026-11-02

## Context

By the time this story runs, track N3 has made the workspace publishable
(GUP-588, GUP-589), added Windows and possibly macOS CI (GUP-590, GUP-591),
minimal accessibility (GUP-592, GUP-593), the gallery and hero (GUP-594,
GUP-595), a CHANGELOG with a semver policy (GUP-596), and a current README
(GUP-597). Release day needs one command that proves the tree is ready, and one
document that says what the owner does next. Publishing is irreversible: a
crates.io version can be yanked, not deleted. This story stops at "ready to
publish". The owner runs `cargo publish`.

## Read First

- `maskfile.md` "## all-check" (284–342), "## merge-check" (381–406) and
  GUP-589's `publish-check` and `docs-rs` tasks.
- `scripts/merge_check.sh`: the summary-table pattern to reuse (each check runs
  regardless of earlier failures).
- `CHANGELOG.md` (GUP-596), the root `Cargo.toml` `[workspace.package]` and
  `[workspace.dependencies]` (GUP-588), and the facade's `gup-core` line
  (GUP-589).

## User Story

> "As the maintainer, I want one command that says whether the tree is ready to
> publish, and a checklist for the steps only I can take, so that release 0.1
> goes out without surprises."

## Acceptance Criteria

### AC1: `mask release-check`

- [ ] `scripts/release_check.sh` runs every check in Decisions, each regardless
      of the others' results. It prints one summary table (check, pass/FAIL,
      seconds), as `merge_check.sh` does, and exits non-zero if any check
      failed. `mask release-check` runs it. It never publishes, tags or pushes;
      `grep -n 'cargo publish' scripts/release_check.sh` shows only `--dry-run`.
- [ ] Each consistency check (items 6–9 in Decisions) has a seeded failure in
      `scripts/test_release_check.sh`, run on a scratch copy of the files it
      reads: a `0.1.1` in one place but not the others; a missing CHANGELOG
      heading; a `publish = false` on a published crate; a bad category slug.
      Each must make its check fail. `mask all-check` runs the test script with
      the others.

### AC2: The run

- [ ] Run `mask release-check` on the story's branch, rebased on `main`. Paste
      the summary table into the Implementation Summary. Every check passes, or
      each failure is reported under Escalation Triggers with its output. Fix
      nothing outside this story's files.
- [ ] Check the latest Tests, Lint, Visual regression, Dogfood, Release check
      and Platforms workflow runs on `main` with
      `gh run list --branch main --limit 20`. Paste each workflow's latest
      conclusion.

### AC3: `docs/RELEASING.md`

- [ ] The checklist of Decisions, as a Markdown task list, with the owner steps
      marked "Owner". `docs/README.md` links it in one line.
- [ ] The CHANGELOG's `## [0.1.0] - Unreleased` heading is left as it is;
      RELEASING.md's step 5 says to date it.
- [ ] Re-read `CHANGELOG.md`'s "Added" and "Known limitations" against what
      merged (GUP-596 left accessibility, the gallery and the Windows smoke job
      out because they had not landed): every shipped feature is listed, every
      listed feature exists, and no limitation is stale. Commit the fix.

## Decisions

- **The checks**, in order:
  1. `mask all-check`.
  2. `cargo test -- --test-threads=1` (root) and
     `cargo test -p gup-core -- --test-threads=1`.
  3. `mask merge-check --dogfood` (gup-core tests, WASM size, browser smoke,
     dogfood).
  4. `mask publish-check` (GUP-589).
  5. `mask docs-rs` (GUP-589).
  6. **One version**: `[workspace.package].version`, every `version = "=X"` in
     `[workspace.dependencies]`, the root `gup-core` line's `=X`, and the first
     `## [X]` heading in `CHANGELOG.md` all agree. Read them with
     `cargo metadata` and `jq` where possible, and with `grep` for the
     CHANGELOG.
  7. **Seven published**: `cargo metadata` lists exactly `gup`, `gup-core`,
     `gup-core-macros`, `gup-wgsl`, `gup-text`, `gup-egui` and `gup-bevy` with
     `publish == null`.
  8. **Categories**: every category of the seven is in
     `https://crates.io/category_slugs` (fetched with `curl -sf`; the check is
     reported as skipped, not passed, when offline).
  9. **Clean tree**: `git status --porcelain` is empty, and `HEAD` is on, or a
     descendant of, `origin/main`.
- **Time**: the script prints the expected time at the start, which is the sum
  of the last measured run, recorded in the script's header. It takes tens of
  minutes. It is meant for release day, not for every commit.
- **`docs/RELEASING.md`'s checklist**:
  1. `mask release-check` passes on `main`. (Agent or owner.)
  2. **Owner**: `cargo login` with a crates.io API token scoped to `publish-new`
     and `publish-update`.
  3. **Owner**: confirm the seven names are still free
     (`https://crates.io/api/v1/crates/<name>` returns 404). They were free on
     2026-11-02.
  4. **Owner**: enable GitHub Pages (source: GitHub Actions), and set the
     repository variable `DEPLOY_GALLERY=true`, so the gallery deploys.
  5. **Owner**: date the CHANGELOG heading (`## [0.1.0] - YYYY-MM-DD`) and
     commit.
  6. **Owner**: `cargo publish --workspace`. Cargo publishes in dependency order
     and waits for each to appear in the index.
  7. **Owner**: tag `v0.1.0`, push the tag, and create a GitHub release with the
     CHANGELOG section as its notes.
  8. GUP-599: switch the docs to the crates.io dependency.
  9. Check docs.rs built all seven (`https://docs.rs/crate/<name>/0.1.0`). If
     one failed, read its build log, then fix it in a 0.1.1.
- **If a publish fails midway** (RELEASING.md says this under step 6): the
  crates already published stay. Fix the cause in a commit that does not change
  the published crates. If re-running `cargo publish --workspace` stops at a
  crate that is already on crates.io, publish the rest by name
  (`cargo publish -p <crate>`), in the order of step 6's output.

## Escalation Triggers

- Any check in AC2 fails. Report the check, its output, and the story that owns
  the cause. Do not fix other stories' files here.
- A workflow on `main` is red.

## Out of Scope

- Every owner step in the checklist.
- Switching the docs to crates.io: GUP-599.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] `scripts/release_check.sh`, `mask release-check` (AC1). Commit.
- [ ] `scripts/test_release_check.sh` and its all-check entry (AC1). Commit.
- [ ] `docs/RELEASING.md` (AC3). Commit.
- [ ] The run and the workflow conclusions (AC2). DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-588 📋, GUP-589 📋, GUP-590 📋, GUP-591 📋, GUP-592 📋, GUP-593 📋,
  GUP-594 📋, GUP-595 📋, GUP-596 📋, GUP-597 📋 — everything the release
  contains.
- GUP-536 📋 — tutorial 7.

### Enables Stories

- Owner step: publishing 0.1.0.
- GUP-599: Switch the docs to crates.io.

## Testing Strategy

- **Seeded**: `scripts/test_release_check.sh`.
- **The run itself**: AC2.

## Success Metrics

- [ ] On release day the owner runs one check and follows one list.

## Risk Assessment

- **Medium**: a timing-sensitive gate fails under load during the long run.
  _Mitigation_: `merge-check` already treats timings as advisory in worker runs.
  Re-run a failed timing check once before reporting it.
- **Low**: the category slug list URL changes. _Mitigation_: offline or
  unreachable means "skipped", reported as such.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `mask release-check`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable; the gates' goldens run
- [ ] `mask wasm-browser`: run inside `merge-check`
- [ ] WASM size: run inside `merge-check`; unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
