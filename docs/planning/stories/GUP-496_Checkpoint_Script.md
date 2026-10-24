---
implementer: sonnet
implementer-reason:
  git plumbing fully specified in Decisions; seeded throwaway-repo test like
  test_pre_commit.sh
touches:
  [
    maskfile.md,
    scripts/,
    .github/agents/story-worker.md,
    .github/agents/story-scribe.md,
  ]
---

# GUP-496: Checkpoint Script for Work in Progress

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 🚧 In Progress
**Created**: 2026-10-24

## Context

The weekly usage limit cut workers off mid-story several times in October
(STRATEGIC_REVIEW_2026-11 §2, "Skipped checkpoint commits"): GUP-446 and GUP-454
had no commits when the limit hit and GUP-481's work was uncommitted. Workers
cannot always commit, because the pre-commit hook rightly refuses code that does
not build or lint, so work between two green states is at risk.

The owner adopted a checkpoint script (review Decision 5,
`docs/planning/STRATEGIC_REVIEW_2026-11.md` "Owner decisions (2026-10-24)"): a
work-in-progress snapshot of the whole working tree to a private ref,
`refs/checkpoints/<branch>`, made with git plumbing so that no hook runs, HEAD
and the branch never move, and nothing is pushed. Real commits keep going
through the full hook. This story writes that script, its mask tasks and a
seeded test, and puts it into the worker's loop.

It is the first N0 story so that every later story can use it. It also adds the
owner's other process decision, hot-file serialisation, to the scribe's template
as a `touches:` frontmatter line (review §2 "Recommended process changes", item
2).

## Read First

- `scripts/test_pre_commit.sh` lines 1–25 (the `expect` style) and the
  `snapshot_cases` function (from `# ── The checks see the staged snapshot` to
  its end): the throwaway-repository pattern this story's test copies.
- `maskfile.md` sections `## all-check` and `## prune-build` (a task with an
  option).
- `.github/agents/story-worker.md` lines 99–175 ("How to stop" and "2c. Commit",
  including the "Checkpoint rule").
- `.github/agents/story-scribe.md` lines 85–205 (the story template) and
  "Choosing the implementer".

## User Story

> "As a story worker that can be cut off by a usage limit at any moment, I want
> to snapshot my working tree, untracked files included, without the pre-commit
> hook and without touching my branch, so that an interrupted session loses at
> most half an hour of work."
>
> "As the orchestrator, I want to restore a worker's last snapshot into its
> checkout after an interruption, so that I can resume or hand over the story."

## Acceptance Criteria

### AC1: `scripts/checkpoint.sh save` snapshots the working tree

- [ ] `scripts/checkpoint.sh` (default subcommand `save`) writes a commit whose
      tree is the working tree as `git add -A` sees it: tracked edits, staged
      changes, deletions and untracked files not ignored by `.gitignore`.
      Ignored files are not in it.
- [ ] It points `refs/checkpoints/<branch>` at that commit and prints one line:
      `checkpoint <short sha> -> refs/checkpoints/<branch>`.
- [ ] HEAD, `refs/heads/<branch>`, the real index file (byte for byte) and
      `git stash list` are unchanged afterwards, and no hook runs.
- [ ] A second `save` with no change in between prints
      `unchanged: refs/checkpoints/<branch> <short sha>` and leaves the ref
      where it was. A `save` on a clean tree (tree equals HEAD's) with no
      existing checkpoint prints `clean: nothing to checkpoint` and creates no
      ref.
- [ ] On a detached HEAD it exits 1 with
      `checkpoint: needs a branch (git switch -c <name>)`.
- [ ] `grep -n push scripts/checkpoint.sh` finds nothing, and the test (AC4)
      shows a configured remote receives no `refs/checkpoints/*`.

### AC2: `restore`, `list` and `drop`

- [ ] `scripts/checkpoint.sh restore [--force] [BRANCH]` makes the working tree
      match `refs/checkpoints/<BRANCH>` (default: the current branch): every
      file in the checkpoint is written, and every tracked or untracked
      non-ignored file not in the checkpoint is deleted. Ignored files, HEAD,
      the branch and the real index are untouched.
- [ ] Without `--force`, `restore` refuses (exit 1, message naming both commits)
      when the checkpoint's first parent is not HEAD, and refuses (exit 1) when
      the working tree has changes that differ from the checkpoint. With
      `--force` it first saves the current state to
      `refs/checkpoints-backup/<branch>` (same commit form as `save`), prints
      that ref, then restores.
- [ ] After a restore the script re-snapshots the working tree into a temporary
      index and exits 1 if that tree differs from the checkpoint's tree.
- [ ] `scripts/checkpoint.sh list` prints one line per ref under
      `refs/checkpoints/`: name, short sha, committer date, base (first parent,
      short sha).
- [ ] `scripts/checkpoint.sh drop [BRANCH]` deletes `refs/checkpoints/<BRANCH>`
      and `refs/checkpoints-backup/<BRANCH>` if present.

### AC3: mask tasks

- [ ] `mask checkpoint` runs `scripts/checkpoint.sh save`, with option
      `-m/--message` appended to the commit message.
- [ ] `mask checkpoint-restore` runs `scripts/checkpoint.sh restore`, with
      options `--force` and `--branch BRANCH`.
- [ ] Each task's description says what is and is not snapshotted and that it
      never replaces a real commit.

### AC4: seeded test

- [ ] `scripts/test_checkpoint.sh` builds a throwaway repository under
      `mktemp -d` (removed on exit), copies `scripts/checkpoint.sh` into it, and
      checks, with a message per failure and a non-zero exit on any:
  - a modified tracked file, a staged new file, a deleted tracked file and an
    untracked file are in the checkpoint's tree with the right contents; an
    ignored file is not;
  - HEAD, the branch, the index file's sha256 and `git stash list` are
    unchanged; `pre-commit` and `post-commit` hooks installed in the repository
    (each writes a marker file) did not run; `git ls-remote` on a bare remote
    shows no `refs/checkpoints`;
  - the unchanged and clean cases of AC1;
  - after changing a checkpointed file, deleting the untracked one and adding a
    new file, `restore --force` brings back the exact checkpointed file list
    (`git ls-files -co --exclude-standard`) and contents (`sha256sum` of each),
    leaves the ignored file alone, and writes
    `refs/checkpoints-backup/<branch>`;
  - `restore` without `--force` refuses after HEAD moves (the test moves the
    throwaway repository's branch with `git commit-tree` and `git update-ref`;
    it commits its initial state before installing the marker hooks, and never
    uses `--no-verify`);
  - the detached-HEAD refusal.
- [ ] Seeded proof, recorded in the Implementation Summary: replacing `add -A`
      with `add -u` in the copied script makes the test fail on the untracked
      file, and removing the deletion step from `restore` makes it fail on the
      file added after the checkpoint. Neither seed is committed.
- [ ] `mask all-check`'s `hook` group runs `./scripts/test_checkpoint.sh` after
      `test_prune_build_dir.sh`, and the `all-check` description names it.

### AC5: the worker loop uses it

- [ ] `.github/agents/story-worker.md` "Checkpoint rule" says: run
      `scripts/checkpoint.sh` every 20–30 minutes (check `date`) and before any
      command that may run for more than 5 minutes, whether or not the code
      builds; it never replaces a commit, so commit whenever the code builds as
      before.
- [ ] "Phase 1" gains a step: if `scripts/checkpoint.sh list` shows a checkpoint
      for your branch whose tree differs from your working tree, you are
      resuming after an interruption: inspect it
      (`git diff --stat refs/checkpoints/<branch>`) and restore it with
      `scripts/checkpoint.sh restore` (with `--force` only if the working tree
      is behind it).
- [ ] "How to stop" step 1 starts with `scripts/checkpoint.sh`.
- [ ] `.github/agents/story-scribe.md`'s template: the frontmatter gains a
      `touches:` line (a YAML list of hot files and directories the story
      edits), with a paragraph under "Choosing the implementer" defining hot
      files (`crates/gup-core/src/plot.rs`, `selection.rs`, `encoding.rs`,
      `lib.rs` and the prelude, `crates/gup-core/PERF_BUDGETS.md`,
      `maskfile.md`, the WASM scenes under `crates/gup-core/wasm-size/`,
      `docs/planning/stories/INDEX.md` beyond status lines) and saying two
      stories that share one never run in parallel; and the Definition of Done
      gains the standing lines: "First commit within 30 minutes, then
      `scripts/checkpoint.sh` every 20–30 minutes between commits",
      "`mask wasm-browser` if the story changes layout, input or wasm code,
      otherwise say why not", and "WASM size: `mask wasm-size` scatter gz bytes,
      or 'unchanged: no library code change'".

## Decisions

- **Location and form**: one Bash script, `scripts/checkpoint.sh`, with
  subcommands `save` (default), `restore`, `list`, `drop`. `set -euo pipefail`,
  short copyright header, usage text on `-h`. It `cd`s to
  `git rev-parse --show-toplevel` first.
- **Save algorithm** (plumbing only, no porcelain that runs hooks):
  1. `branch=$(git symbolic-ref --quiet --short HEAD)` or the detached-HEAD
     error.
  2. `tmp=$(mktemp)`; trap removes it. Copy the real index
     (`git rev-parse --git-path index`, made absolute) to `$tmp` if it exists,
     so stat data is reused.
  3. `GIT_INDEX_FILE=$tmp git add -A`;
     `tree=$(GIT_INDEX_FILE=$tmp git write-tree)`.
  4. Unchanged/clean checks against `refs/checkpoints/$branch^{tree}` and
     `HEAD^{tree}`.
  5. `git commit-tree --no-gpg-sign -p HEAD [-p <old>] -m <subject> [-m <message>] "$tree"`,
     where `<subject>` is `checkpoint $branch $(date -Iseconds)` and `<old>` is
     the previous checkpoint. The previous checkpoint is the second parent, so
     `git log` keeps the chain. `--no-gpg-sign` because `commit-tree` honours
     `commit.gpgSign` and an agent cannot answer a passphrase prompt.
  6. `git update-ref refs/checkpoints/$branch $commit [<old>]` (compare-and-swap
     on the old value).
- **Restore algorithm**: resolve the checkpoint commit first; base check
  (`git rev-parse <commit>^1` vs `HEAD`); dirty check (snapshot the working tree
  into a temp index as in save, compare trees; equal means nothing to do and it
  prints `already at <short sha>`); with `--force`, save the current state to
  `refs/checkpoints-backup/$branch`; then
  `GIT_INDEX_FILE=$tmp git read-tree <commit>` and
  `GIT_INDEX_FILE=$tmp git checkout-index -a -f`; delete every path in
  `git ls-files -co --exclude-standard -z` (real index) that is not in
  `git ls-tree -r -z --name-only <commit>`; verify as in AC2. The real index is
  never written: restore recreates files, not the staging area (say so in the
  usage text).
- **Ref namespace**: `refs/checkpoints/<branch>` and
  `refs/checkpoints-backup/<branch>` (a separate namespace, so a branch name can
  never collide with a `backup/` sub-ref). Refs are shared between worktrees, so
  the orchestrator sees every worker's checkpoints from the main checkout.
- **No detached-HEAD support**: workers always have a branch; a fallback name
  would break the chain whenever HEAD moves.
- **Test style**: `scripts/test_checkpoint.sh`, Bash, a throwaway repository,
  `git -c user.name=t -c user.email=t@example.com` (CI has no identity), one
  `check` helper that counts failures; it never touches the real repository.
- **Mask tasks** sit after `## prune-build`. Mask options: `checkpoint` has
  `message` (`-m --message`, type string); `checkpoint-restore` has `force`
  (`--force`) and `branch` (`--branch`, type string).

## Escalation Triggers

- Anything that would bypass or weaken the pre-commit gate for real commits:
  changing `scripts/pre_commit.sh`, the hook installation in `flake.nix`, or
  telling workers to use `--no-verify` or to treat a checkpoint as a commit.
- `git add -A` with a temporary index turns out to write the real index or take
  its lock (`index.lock`) in any git version available in the dev shell or CI.
- `git commit-tree` or `git update-ref` runs any hook in the test (the marker
  file appears).
- The `all-check` `hook` group's run time grows by more than 10 s.

## Out of Scope

- Automatic checkpoints (a timer, a cron, a hook): the worker runs it by hand.
- Pruning old checkpoint refs after merges: the orchestrator runs `drop`.
- Hot-file detection or enforcement tooling: `touches:` is declarative for now.
- The orchestrator's own memory and briefs (not in the repository).

## Technical Tasks

- [ ] Commit the story's start ("Start GUP-496: ...") within 30 minutes.
- [ ] Write `scripts/checkpoint.sh` (save, list, drop first; commit; then
      restore).
- [ ] Write `scripts/test_checkpoint.sh`; run the two seeds by hand in a scratch
      copy and record the failures.
- [ ] Add `mask checkpoint` and `mask checkpoint-restore`; add the test to
      `all-check`'s `hook` group and description.
- [ ] Edit `story-worker.md` (Checkpoint rule, Phase 1 resume step, How to stop)
      and `story-scribe.md` (`touches:`, hot files, standing DoD lines).
- [ ] Run `mask all-check`; run the script for real in this checkout once
      (`scripts/checkpoint.sh`, `list`, `drop`) and paste the output in the
      Implementation Summary.

## Dependencies

### Prerequisite Stories

None.

### Enables Stories

- GUP-497 to GUP-504: every later N0 story's Definition of Done uses
  `scripts/checkpoint.sh`.

## Testing Strategy

- **Unit/integration**: `scripts/test_checkpoint.sh` in a throwaway repository
  (AC4), run by `mask all-check` locally and in the Lint workflow.
- **Seeded proof**: the two seeds of AC4, by hand, recorded.
- **Manual**: one real save/list/drop in the worker's own checkout.

## Success Metrics

- [ ] `scripts/test_checkpoint.sh` passes in under 10 s.
- [ ] Both seeds make it fail.
- [ ] `git log -1 --oneline` on the branch and `git status --porcelain` are
      identical before and after a real `save` in the worker's checkout.

## Risk Assessment

- **Medium**: `restore` deletes files. A wrong path list could delete work.
  _Mitigation_: it refuses on a dirty tree without `--force`, `--force` backs up
  first, and the test covers the deletion list exactly.
- **Low**: `git add -A` on a tree with large untracked outputs stores them as
  blobs. _Mitigation_: build output lives outside the checkout
  (`CARGO_TARGET_DIR`); `.gitignore` covers the rest.
- **Low**: checkpoint commits keep blobs alive until `drop`. _Mitigation_:
  `drop` after merge; blobs are deduplicated with real commits.

## Definition of Done

- [ ] First commit within 30 minutes of starting; then commit at each checkpoint
      (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `scripts/test_checkpoint.sh` passes; `mask all-check` passes
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser`: not needed (scripts and agent docs only; no layout,
      input or wasm change); say so in the Implementation Summary
- [ ] WASM size: "unchanged: no library code change"
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
