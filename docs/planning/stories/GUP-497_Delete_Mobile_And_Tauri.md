---
implementer: sonnet
implementer-reason:
  deletions with an exact path list; one hook-test fixture is specified in
  Decisions
touches:
  [
    Cargo.toml,
    maskfile.md,
    README.md,
    docs/README.md,
    scripts/test_pre_commit.sh,
    .github/workflows/,
  ]
---

# GUP-497: Delete the Mobile and Tauri Integrations

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
(2026-10-24) **Created**: 2026-10-24

## Context

`gup-ios`, `gup-android` and the Tauri example (`examples/gup-tauri`) were
parked outside the workspace in GUP-390 to be re-wired later. They name APIs the
RFC-001 flip deleted (`gup::context::GupContext`, the `ios-shim` and
`android-shim` features), so a re-wire would be a rewrite. The owner chose to
delete them and their workflows; git keeps them
(`docs/planning/STRATEGIC_REVIEW_2026-11.md`, "Owner decisions (2026-10-24)",
item 3). A future mobile or Tauri host is N6 work, built from scratch on
`gup-core` when a user asks.

The code is small (40 tracked files), but references to it are spread over the
root `Cargo.toml`, `.gitignore`, two workflows, two mask tasks, `README.md`,
`docs/README.md`, the architecture docs, `examples/README.md` and one
pre-commit-hook test case. This story removes all of them with every gate green.

## Read First

- Root `Cargo.toml` lines 1–14 (workspace `exclude` and its comment).
- `README.md` "Parked integration crates" (around lines 150–176) and
  "Development Status" (around line 93).
- `docs/README.md` "Mobile Platforms" (around lines 120–134).
- `scripts/test_pre_commit.sh` lines 1–25 (`expect`) and line 81.
- `scripts/pre_commit.sh` lines 228–239 (`foreign_manifest_dir`) and 302–313.
- `maskfile.md` `## ci` (the "Not covered" sentence), `## tauri-example`,
  `## ios-build`.

## User Story

> "As a contributor, I want the repository to contain only code that builds
> against the current core, so that I never spend time on, or get misled by,
> integrations that cannot compile."

## Acceptance Criteria

### AC1: the code and workflows are gone

- [x] Deleted: `gup-ios/`, `gup-android/`, `examples/gup-tauri/`,
      `examples/android/`, `examples/ios_scatter/`, `pkg/` (it holds only the
      Swift and Kotlin packages and a `.gitignore`),
      `docs/TAURI_INTEGRATION.md`, `.github/workflows/ios-ci.yml`,
      `.github/workflows/android-ci.yml`.
- [x] Root `Cargo.toml`: `exclude = ["crates/gup-culling-lod"]`, and its comment
      names only `gup-culling-lod` (parked, strategic review 2026-10 Decision 3)
      and says the mobile and Tauri crates were deleted in GUP-497.
- [x] `.gitignore` loses the six `/gup-ios/…`, `/gup-android/…` and
      `/examples/gup-tauri/…` lines.
- [x] `maskfile.md` loses `## tauri-example` and `## ios-build`; `## ci`'s "Not
      covered" sentence names only the Gallery deploy.
- [x] `cargo metadata --format-version 1 --no-deps | jq -r '.workspace_members | length'`
      prints the same number before and after, and `git diff --stat Cargo.lock`
      is empty.

### AC2: no references remain

- [x] This command prints nothing:

      ```bash
      git grep -n -i -E 'gup-ios|gup-android|gup_ios|gup_android|gup-tauri|ios-shim|android-shim|ios_scatter|examples/android|pkg/(ios|android)|TAURI_INTEGRATION|ios-ci|android-ci' \
        -- . ':!docs/planning' ':!crates/gup-culling-lod'
      ```

- [x] `README.md`: the "Parked integration crates" section is removed (the
      `gup-egui`/`gup-bevy` sentence at its end moves up to the preceding
      section); "Development Status" no longer mentions "the mobile
      integrations".
- [x] `docs/README.md`: the "Mobile Platforms" section is removed.
- [x] `docs/TECHNICAL_APPROACH.md` (around line 404) and
      `docs/IMPLEMENTATION_STRATEGY.md` (around line 111): the sentence about
      the iOS, Android and Tauri shims says they were deleted (strategic review
      2026-11, Decision 3) and that a mobile or Tauri host would be built on
      `gup-core` on demand (track N6). `cargo test --doc -p gup` passes (both
      files are doctests).
- [x] `examples/README.md`: the parked-directories sentence is removed.
- [x] `docs/MISSION_AND_GOALS.md` and everything under `docs/planning/` are not
      edited (vision and history).

### AC3: the hook's foreign-crate case keeps a test

- [x] `scripts/test_pre_commit.sh` line 81 (`examples/gup-tauri/...`) is
      replaced by the fixture in Decisions, and the comment in
      `scripts/pre_commit.sh` (around line 306) no longer mentions Tauri.
- [x] `./scripts/test_pre_commit.sh` passes, and no other `expect` line in it
      changes.
- [x] Seeded proof, recorded in the Implementation Summary: with the
      `foreign_manifest_dir` branch in a scratch copy of `pre_commit.sh` made to
      always take the `else` path, the new case fails. Not committed.

### AC4: all gates green

- [x] `mask all-check` passes (it includes `actionlint` on the remaining
      workflows and `test_pre_commit.sh`).
- [x] `cargo test --doc -p gup` passes.

## Decisions

- **Delete, do not archive**: git history keeps the code; the review's Decision
  3 says delete.
- **`pkg/` goes entirely**: it contains only `ios/`, `android/` and a
  `.gitignore` of `*`. `mask pack` (wasm-pack's default output) recreates it
  when used.
- **Foreign-crate fixture** (replaces line 81 of `test_pre_commit.sh`): the hook
  still escalates to `mode=full` when a staged path is inside a crate outside
  the workspace under the root package's directories; keep that branch tested
  with a crate made on the fly:

  ```bash
  # A crate outside the workspace inside the root package's directories
  # (none exists today) is checked in full, not scoped to `gup`.
  foreign=$(mktemp -d examples/foreign-crate.XXXXXX)
  trap 'rm -rf "$foreign"' EXIT
  printf '[package]\nname = "foreign"\nversion = "0.0.0"\n' >"$foreign/Cargo.toml"
  expect 'mode=full' "$foreign/src/main.rs"
  rm -rf "$foreign"
  trap - EXIT
  ```

  If `test_pre_commit.sh` already sets an `EXIT` trap, chain into it instead of
  replacing it.

- **Wording for the architecture docs**: one sentence each, for example "The
  iOS, Android and Tauri shims were deleted (strategic review 2026-11, Decision
  3); a mobile or Tauri host would be built on `gup-core` when a user asks
  (track N6)."
- **`gup-culling-lod` stays** parked and excluded; it is not part of this
  decision.

## Escalation Triggers

- Any `expect` line in `test_pre_commit.sh` other than line 81 needs to change.
- `Cargo.lock` changes, or the workspace member count changes.
- A file outside the AC1 list turns out to depend on the deleted code (a build,
  test, script or workflow fails because of the deletion).
- `scripts/pre_commit.sh` needs a change other than the comment.

## Out of Scope

- `crates/gup-culling-lod` (parked; LOD returns in N5).
- Stale feature guides, accessibility docs and the docs index: GUP-499.
- Closing the mobile stories (GUP-274, GUP-354) in INDEX.md: GUP-503.
- The generic `mask pack/serve/start` tasks and `CLAUDE.md`'s WebAssembly
  section: GUP-498.

## Technical Tasks

- [x] Start commit within 30 minutes.
- [x] `git rm -r` the AC1 paths; edit `Cargo.toml`, `.gitignore`, `maskfile.md`;
      checkpoint.
- [x] Edit `README.md`, `docs/README.md`, `docs/TECHNICAL_APPROACH.md`,
      `docs/IMPLEMENTATION_STRATEGY.md`, `examples/README.md`.
- [x] Replace the `test_pre_commit.sh` case; edit the `pre_commit.sh` comment;
      run the seed by hand.
- [x] Run the AC2 grep, `cargo test --doc -p gup`, `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-496: Checkpoint Script 📋 — `scripts/checkpoint.sh` for the DoD's
  checkpoint line (soft: commit by hand if it has not landed).

### Enables Stories

- GUP-499: Archive Stale Docs — edits `docs/README.md` after this story's
  removal of its Mobile section.

## Testing Strategy

- **Integration**: `mask all-check` (actionlint, `test_pre_commit.sh`, clippy
  over the workspace), `cargo test --doc -p gup`.
- **Seeded proof**: the foreign-crate case fails when the hook's branch is
  disabled (AC3).
- **Search**: the AC2 grep prints nothing.

## Success Metrics

- [x] 40 tracked files deleted (37 under the crate, example and `pkg/`
      directories, plus `docs/TAURI_INTEGRATION.md` and the two workflows).
- [x] The AC2 grep prints nothing.
- [x] `mask all-check` passes.

## Risk Assessment

- **Low**: an unnoticed reference breaks a CI workflow. _Mitigation_: the AC2
  grep covers workflows; actionlint runs in `all-check`; the orchestrator's
  post-merge CI poll catches the rest.
- **Low**: the temporary fixture directory survives an interrupted test.
  _Mitigation_: the `EXIT` trap; `examples/foreign-crate.*` is untracked and
  visible in `git status`.

## Definition of Done

- [x] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-check` passes; `cargo test --doc -p gup` passes
- [x] Lint and format clean: `mask all-fix`
- [x] `mask wasm-browser`: not needed (no layout, input or wasm change); say so
      in the Implementation Summary
- [x] WASM size: "unchanged: no library code change"
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Deleted 40 tracked files (`gup-ios/`, `gup-android/`, `examples/gup-tauri/`,
`examples/android/`, `examples/ios_scatter/`, `pkg/`,
`docs/TAURI_INTEGRATION.md` and the two mobile workflows). Edited `Cargo.toml`
(`exclude` and comment), the ignore file, `maskfile.md` (two tasks and the "Not
covered" sentence), `README.md`, `docs/README.md`, `docs/TECHNICAL_APPROACH.md`,
`docs/IMPLEMENTATION_STRATEGY.md`, `examples/README.md`, the comment in
`scripts/pre_commit.sh`, and replaced the `test_pre_commit.sh` case with the
on-the-fly foreign-crate fixture.

- Workspace members: 8 before and after; `Cargo.lock` unchanged.
- The AC2 grep prints nothing. `cargo test --doc -p gup` and `mask all-check`
  pass.
- Seeded proof: with the `foreign_manifest_dir` test in a scratch edit of
  `pre_commit.sh` replaced by `if false`, the new case failed
  (`expected 'mode=full'`, got `mode=scoped`); the edit was reverted and not
  committed.
- `mask wasm-browser`: not needed (no layout, input or wasm change). WASM size:
  unchanged, no library code change.

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Hook test fixtures

- **Challenge**: the hook's foreign-crate branch lost its only real example.
- **Solution**: a temporary crate made under `examples/` with an `EXIT` trap.
- **Pattern**: test a classifier branch with a synthetic input, and prove it
  with a seeded break.

### Architectural Decisions

#### Delete rather than park

- **Decision**: remove the mobile and Tauri code.
- **Reasoning**: it named deleted APIs, so re-wiring would be a rewrite.
- **Trade-off**: none for users; version control keeps the code.
- **Future**: N6 builds a host from scratch on `gup-core`.

### Development Workflow Insights

- The worktree sandbox refused some compound commands; keep commands plain and
  separate.

### Follow-up Stories

None.
