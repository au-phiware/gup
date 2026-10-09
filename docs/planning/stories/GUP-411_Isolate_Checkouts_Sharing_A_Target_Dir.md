# GUP-411: Keep Checkouts That Share a Target Directory from Reusing Each Other's Artifacts

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: 📋 Planned
**Created**: 2026-10-10

## Context

Agents run in git worktrees of this repository and share one `CARGO_TARGET_DIR`
(for example `/tmp/gup-target`) to save disk.
[GUP-409](GUP-409_Close_Gate_Gaps.md) found that this is not safe. Cargo hashes
a workspace member's package ID by its path _relative to the workspace root_, so
every checkout of the workspace produces the same artifact file names for `gup`,
`gup-core` and the other members. Cargo decides freshness by mtime. So a
checkout reuses an artifact that another checkout built from different source
whenever its own sources are older than that artifact:

- GUP-409's toy workspace: a second checkout with different content reported
  `Fresh a` and produced no build.
- GUP-409's seeded hook run: with the separation removed, the hook passed a
  staged clippy lint because the real tree had just built clean content into the
  same artifact.
- In the same run the real tree reported `Fresh` for `gup` and `gup-text` using
  artifacts built in the hook's snapshot (identical content that time).

A worktree is created with all its files written at once. If another agent then
builds `gup-core` from its own edits, this worktree's `gup-core` sources are
older than that artifact, so `cargo clippy`, `cargo test` or the pre-commit hook
can accept or reject code that is not in this checkout. The failure is silent
and depends on timing, the kind that costs a debugging session.

GUP-409 protected only the hook's snapshot, by giving workspace members a
different `incremental` profile setting in the snapshot. That trick cannot
separate N arbitrary checkouts: there are only two values.

## User Story

> "As an agent (or a developer with several worktrees) sharing a target
> directory to save disk, I want every build and lint to use artifacts built
> from my checkout's sources, so that a green or red result means something
> about my code."

## Acceptance Criteria

- [ ] A seeded reproduction in two worktrees that share a target directory:
      worktree B (older sources) reports `Fresh` for a member that worktree A
      rebuilt from different content, and `cargo clippy` in B gives A's verdict.
      Recorded with commands and output (GUP-398's convention).
- [ ] A fix, chosen and documented, after which the same reproduction makes B
      rebuild the member. Candidates: per-checkout target directories with
      shared dependency artifacts (cargo's `build.build-dir`, or `sccache`), a
      per-checkout `CARGO_TARGET_DIR` set by the dev shell, or another
      mechanism. Measure the disk and cold-build cost of the choice.
- [ ] The orchestrator and story-worker guidance (`.github/agents/`) say how to
      share build output safely, and the dev shell's default (strategic review
      T0 item 5, "Set a default `CARGO_TARGET_DIR` in the dev shell") follows
      it.
- [ ] The pre-commit hook's snapshot separation (GUP-409) is kept, replaced or
      removed to match, with its seeded test still passing.

## Technical Tasks

- [ ] Reproduce with two `git worktree add` checkouts and one target directory;
      capture `cargo clippy -v` showing `Fresh`.
- [ ] Prototype per-checkout isolation that keeps dependencies shared, and
      measure disk use and cold and warm `mask all-check` times.
- [ ] Update `flake.nix` (if the dev shell sets the default), the agent docs and
      `scripts/pre_commit.sh`'s header comment.

## Dependencies

### Prerequisite Stories

- GUP-409 ✅: found the problem and the member-only profile separation.

### Enables Stories

(none known)

## Testing Strategy

- **Seeded reproduction**: two worktrees, one target directory, before and after
  the fix.
- **Script tests**: `scripts/test_pre_commit.sh`'s snapshot cases still pass.
- **Measurement**: disk use of the target directory and `mask all-check` times
  for one and for three concurrent worktrees.

## Success Metrics

- [ ] No checkout reports `Fresh` for a member whose sources differ from the
      artifact's, in the seeded reproduction.
- [ ] Disk use for three concurrent worktrees is measured and stays within what
      /tmp can hold (about 20 GB today).

## Risk Assessment

- **Medium**: per-checkout target directories multiply disk use, which has
  already exhausted /tmp in earlier stories. Mitigation: share dependency
  artifacts and isolate only workspace members.
- **Low**: changing the dev shell's environment affects every developer.
  Mitigation: keep it overridable and document it in `CLAUDE.md`.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Lint and format clean: `mask all-check`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
