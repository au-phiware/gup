# GUP-409: Close Gate Gaps: Workflow Lint, Staged-Only Hook and CI Concurrency

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 🚧 In Progress **Created**:
2026-10-09

## Context

[GUP-398](GUP-398_Honest_And_Fast_Quality_Gates.md) established that "a gate
that has never been proven to fail is not a gate" and made every hook/CI check
prove itself against a seeded violation. Three gaps surfaced since then that
GUP-398's own audit did not cover, because they are not `cargo`/`mask` checks at
all: the workflow YAML itself, the hook's choice of what to check, and CI's
runner capacity.

**Workflow YAML is not validated.**
[GUP-400](GUP-400_RFC_001_S2_Extract_Gup_Text.md) (commit `cf03651`) pasted
generated text into the comment header of
`.github/workflows/visual-regression.yml`, corrupting its YAML (confirmed by
`git show cf03651 -- .github/workflows/visual-regression.yml`; fixed two commits
later in `548131a`, "Fix visual-regression workflow corrupted by GUP-400").
Neither the pre-commit hook nor CI's own lint job caught it — both only run
Rust/Nix/Markdown checks — and GitHub could not parse the workflow at all (the
referenced run, 37334077045, never started). `nix run nixpkgs#actionlint`
catches this class of error locally and in CI. `actionlint` currently reports 14
shellcheck warnings across the existing workflows (verified 2026-10-09:
`android-ci.yml` ×2, `performance.yml` ×7, `wasm.yml` ×3, plus duplicates);
these need fixing or an explicit, documented exclusion, so the new check starts
at zero.

**The pre-commit hook checks the working tree, not the staged snapshot.**
`maskfile.md`'s `pre-commit` and `all-check` tasks both say so directly: "Like
`all-check`, the hook checks the working tree, not the index"
(`scripts/pre_commit.sh`'s own header comment repeats this). In practice this
caused a real failure during the orchestrator/worker split: committing a
one-file fix while a worker had in-progress, uncommitted changes to other files
in the same tree made clippy fail on the worker's files, which were never going
to be part of the commit. The opposite failure is also possible and more
dangerous: a commit can pass the hook only because of unstaged changes that
happen to fix something the staged diff alone would break — so `main` gets a
broken commit. [GUP-398](GUP-398_Honest_And_Fast_Quality_Gates.md)'s convention
applies here too: this needs a seeded-violation proof in both directions (a
staged-only violation that today's hook misses because of clean-but-irrelevant
untracked state, and a staged-only fix that today's hook wrongly rejects because
of dirty working-tree state), not just a description of the bug.

**CI has no concurrency control on two workflows, and Performance runs more jobs
per push than it needs to.** Most workflows already have a `concurrency:` block
(`lint.yml`, `tests.yml`, `visual-regression.yml`, `dogfood.yml`, `gallery.yml`,
`android-ci.yml`, `ios-ci.yml` — all `cancel-in-progress: true`), added
piecemeal across earlier stories. `performance.yml` and `wasm.yml` do not. Runs
on `83143d2` and `7f8ad96` failed with "job was not acquired by Runner … after
multiple attempts" — a GitHub-hosted-runner capacity error that happens when too
many queued jobs pile up faster than runners are assigned. A push to `main`
currently triggers about eleven jobs across seven workflows (`lint`, `tests`,
`visual-regression`, `wasm`, `dogfood`, `gallery`'s two jobs, and four of
`performance`'s jobs — `perf_check`, `performance`, `axis_performance`,
`wasm_axis_performance`; `cross_platform_comparison`, `pattern_benchmarks_pr`
and the weekly `benchmark` job do not run on push). Without `concurrency` groups
on `performance.yml` and `wasm.yml`, a second push before the first finishes
adds its jobs to the queue instead of cancelling the superseded run, compounding
exactly the pile-up that produced the "not acquired" failures.

## User Story

> "As a maintainer pushing to `main`, I want a corrupted workflow file, an
> unscoped-but-wrong local hook result, and CI run failures from runner
> exhaustion to all be caught before they cost a debugging session, so that the
> gates GUP-398 made honest stay honest as new kinds of gaps appear."

## Acceptance Criteria

### AC1: Workflow YAML is linted

- [ ] `actionlint` is available in the Nix dev shell (added to `flake.nix`'s
      `buildInputs`) and runs as part of `mask all-check` and `mask all-fix`
      (where `--fix`-equivalent behaviour does not exist for actionlint, so
      `all-fix` just runs the check).
- [ ] `actionlint` runs in the `lint.yml` CI workflow against every file in
      `.github/workflows/`.
- [ ] The 14 existing shellcheck warnings are each either fixed or excluded with
      a documented reason (an actionlint config file or inline
      `# shellcheck disable=` comment, whichever the fix calls for); the check
      reports 0 after this story, not 14.
- [ ] A seeded violation (reintroducing the exact corruption from commit
      `cf03651`, or an equivalent malformed YAML header, in a throwaway workflow
      file not imported by any trigger) is shown failing `actionlint` locally
      and in CI, then reverted (GUP-398's convention).
- [ ] `scripts/pre_commit.sh`'s classification treats any change under
      `.github/workflows/` as requiring the actionlint check (full mode is
      acceptable if scoping it is not worth the complexity; document the choice
      either way).

### AC2: The pre-commit hook checks the staged snapshot, not the working tree

- [ ] `scripts/pre_commit.sh` (and anything it calls, such as
      `mask     all-check` when invoked from the hook) runs its checks against
      exactly what is being committed — the staged content — not the working
      tree. The implementation may use a temporary worktree, a checkout of the
      index into a scratch directory, `git stash --keep-index` around the check,
      or another mechanism; document the choice and its cost
      (`scripts/pre_commit.sh`'s header comment currently documents the old
      working-tree behaviour and must be corrected).
- [ ] Direct invocations of `mask all-check`/`mask pre-commit` outside the git
      hook (for example from CI's `mask ci`, or a developer running it by hand)
      are unaffected or explicitly documented as still checking the working tree
      — be clear about which commands check what.
- [ ] **Seeded proof, direction 1 (false pass):** with a staged fix to a clippy
      violation and a _different_, untracked file left dirty with an unrelated
      clippy violation, today's hook fails (wrongly, since the violation is not
      being committed); after the fix, the hook passes.
- [ ] **Seeded proof, direction 2 (false fail):** with a staged file that is
      clean on its own, and an unstaged, uncommitted edit to the _same_ file
      that introduces a violation, today's hook can fail on content that will
      not actually be committed (or pass only because of unstaged content that
      happens to compensate); after the fix, the hook's verdict matches what
      `git diff --cached` would produce in isolation. Record the exact
      before/after commands and output, per GUP-398's convention.
- [ ] `scripts/test_pre_commit.sh` gains cases covering both directions above
      and passes.

### AC3: CI concurrency control on every workflow

- [ ] Every workflow in `.github/workflows/` has a `concurrency:` block.
      `performance.yml` and `wasm.yml` gain one, matching the existing
      convention (`group: <workflow>-${{ github.ref }}`,
      `cancel-in-progress: true`); the other seven are reviewed and left as-is
      unless a specific problem is found.
- [ ] A seeded proof: trigger two overlapping runs of a workflow that lacked
      `concurrency` (for example two quick successive pushes to a scratch
      branch) before the fix, observe both run to completion or queue
      competitively; after the fix, observe the first run cancelled in GitHub's
      UI/API. Record the run URLs or `gh run list` output.
- [ ] A documented decision on trimming `performance.yml`'s push-triggered jobs
      (currently `perf_check`, `performance`, `axis_performance`,
      `wasm_axis_performance` — four jobs on every push to `main`): either state
      why all four earn their place on every push, or move one or more to
      `pull_request`-only, `schedule`-only, or `workflow_dispatch`-only, with
      the reasoning recorded in this story's Context or a comment in
      `performance.yml`.

## Technical Tasks

- [ ] Add `actionlint` (and any shellcheck it bundles/needs) to `flake.nix`'s
      dev shell `buildInputs`.
- [ ] Add an `actionlint .github/workflows/*.yml` line to
      `mask lint`/`mask     lint-check`/`mask all-check`/`mask all-fix` in
      `maskfile.md`, and a step in `lint.yml`.
- [ ] Fix or explicitly exclude each of the 14 current shellcheck findings
      (mostly `SC2086` unquoted expansions and `SC2015`/`SC2129` style notes in
      `android-ci.yml`, `performance.yml` and `wasm.yml`).
- [ ] Add workflow-file handling to `scripts/pre_commit.sh`'s classifier.
- [ ] Redesign `scripts/pre_commit.sh` (and `mask all-check`'s invocation from
      the hook) to operate on the staged index rather than the working tree —
      likely via `git worktree add` against a temporary directory checked out
      from a synthetic commit of the index, or an equivalent stash-based
      approach; update the header comments in both `scripts/pre_commit.sh` and
      `maskfile.md`'s `pre-commit`/`all-check` sections that currently document
      the old (working-tree) behaviour.
- [ ] Add the two new seeded-violation cases to `scripts/test_pre_commit.sh`.
- [ ] Add `concurrency:` blocks to `performance.yml` and `wasm.yml`.
- [ ] Review `performance.yml`'s push-triggered job set and either justify or
      trim it; update the workflow's header comment with the decision.

## Dependencies

### Prerequisite Stories

- GUP-398 ✅ — established the seeded-violation convention and the hook's
  proportional-scoping design this story extends.
- GUP-403 ✅ — made all CI workflows green and added `mask ci`; this story keeps
  them green under the new checks.

### Enables Stories

(none known)

## Testing Strategy

- **Unit/script tests**: `scripts/test_pre_commit.sh`'s existing 24 cases plus
  the new staged-vs-working-tree cases from AC2.
- **Integration tests**: a seeded, deliberately malformed workflow file run
  through `actionlint` locally and in CI (AC1); a seeded concurrent-run
  observation via `gh run list`/the Actions UI (AC3).
- **CI**: `lint.yml` picks up the new `actionlint` step; `performance.yml` and
  `wasm.yml` are pushed with the new `concurrency` blocks and observed
  cancelling a superseded run.

## Success Metrics

- [ ] `nix run nixpkgs#actionlint -- .github/workflows/*.yml` reports 0 issues
      (down from 14) and is enforced in CI.
- [ ] The pre-commit hook's verdict matches `git diff --cached` in isolation,
      proven in both directions by seeded tests.
- [ ] Every workflow in `.github/workflows/` has a `concurrency` block; no "job
      was not acquired by Runner" failures are observed on the next several
      pushes to `main` (best-effort; capacity errors depend on GitHub's
      infrastructure too, so this is evidence, not a guarantee).

## Risk Assessment

- **Medium**: rewriting the hook to check a staged snapshot (not the working
  tree) could be slow (an extra `git worktree`/checkout per commit) or subtly
  wrong (missing untracked files the staged change depends on, like a new module
  not yet `git add`-ed). Mitigation: measure the added time and report it next
  to GUP-398's existing before/after hook timing; the seeded tests in AC2 are
  the correctness check.
- **Low**: trimming `performance.yml`'s push-triggered jobs could hide a
  regression that only a push-triggered run would have caught before a PR
  merges. Mitigation: only trim with an explicit, recorded reason (AC3), and
  prefer moving jobs to `pull_request` (which still gates merges) over removing
  them outright.
- **Low**: `actionlint`'s shellcheck integration may flag style issues
  (`SC2129`, `SC2015`) that are harmless in practice. Mitigation: AC1 allows
  explicit, documented exclusions rather than forcing every warning to be
  "fixed" if the fix would reduce clarity.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
