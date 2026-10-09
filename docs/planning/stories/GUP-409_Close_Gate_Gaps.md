# GUP-409: Close Gate Gaps: Workflow Lint, Staged-Only Hook and CI Concurrency

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: ✅ Complete (2026-10-10;
two GitHub-side observations pending, see Implementation Summary) **Created**:
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

- [x] `actionlint` is available in the Nix dev shell (added to `flake.nix`'s
      `buildInputs`) and runs as part of `mask all-check` and `mask all-fix`
      (where `--fix`-equivalent behaviour does not exist for actionlint, so
      `all-fix` just runs the check).
- [x] `actionlint` runs in the `lint.yml` CI workflow against every file in
      `.github/workflows/`.
- [x] The 14 existing shellcheck warnings are each either fixed or excluded with
      a documented reason (an actionlint config file or inline
      `# shellcheck disable=` comment, whichever the fix calls for); the check
      reports 0 after this story, not 14.
- [ ] A seeded violation (reintroducing the exact corruption from commit
      `cf03651`, or an equivalent malformed YAML header, in a throwaway workflow
      file not imported by any trigger) is shown failing `actionlint` locally
      and in CI, then reverted (GUP-398's convention). _Local half done: the
      exact `cf03651` file fails `actionlint` (exit 1) and is rejected by the
      hook, in place and from the snapshot (Retrospective). The CI half needs a
      push of the seed, which this story's worker could not do: it runs in
      `mask all-check`, so the Lint job runs the same command._
- [x] `scripts/pre_commit.sh`'s classification treats any change under
      `.github/workflows/` as requiring the actionlint check (full mode is
      acceptable if scoping it is not worth the complexity; document the choice
      either way).

### AC2: The pre-commit hook checks the staged snapshot, not the working tree

- [x] `scripts/pre_commit.sh` (and anything it calls, such as
      `mask     all-check` when invoked from the hook) runs its checks against
      exactly what is being committed — the staged content — not the working
      tree. The implementation may use a temporary worktree, a checkout of the
      index into a scratch directory, `git stash --keep-index` around the check,
      or another mechanism; document the choice and its cost
      (`scripts/pre_commit.sh`'s header comment currently documents the old
      working-tree behaviour and must be corrected).
- [x] Direct invocations of `mask all-check`/`mask pre-commit` outside the git
      hook (for example from CI's `mask ci`, or a developer running it by hand)
      are unaffected or explicitly documented as still checking the working tree
      — be clear about which commands check what.
- [x] **Seeded proof, direction 1 (false pass):** with a staged fix to a clippy
      violation and a _different_, untracked file left dirty with an unrelated
      clippy violation, today's hook fails (wrongly, since the violation is not
      being committed); after the fix, the hook passes.
- [x] **Seeded proof, direction 2 (false fail):** with a staged file that is
      clean on its own, and an unstaged, uncommitted edit to the _same_ file
      that introduces a violation, today's hook can fail on content that will
      not actually be committed (or pass only because of unstaged content that
      happens to compensate); after the fix, the hook's verdict matches what
      `git diff --cached` would produce in isolation. Record the exact
      before/after commands and output, per GUP-398's convention.
- [x] `scripts/test_pre_commit.sh` gains cases covering both directions above
      and passes.

### AC3: CI concurrency control on every workflow

- [x] Every workflow in `.github/workflows/` has a `concurrency:` block.
      `performance.yml` and `wasm.yml` gain one, matching the existing
      convention (`group: <workflow>-${{ github.ref }}`,
      `cancel-in-progress: true`); the other seven are reviewed and left as-is
      unless a specific problem is found.
- [ ] A seeded proof: trigger two overlapping runs of a workflow that lacked
      `concurrency` (for example two quick successive pushes to a scratch
      branch) before the fix, observe both run to completion or queue
      competitively; after the fix, observe the first run cancelled in GitHub's
      UI/API. Record the run URLs or `gh run list` output. _"Before" is in the
      run history (Retrospective): pushes 83143d2 and 7f8ad96 overlapped;
      83143d2's Lint and Visual regression runs were cancelled two seconds after
      7f8ad96 was pushed, while its Performance run (no group) ran on for
      another nine minutes. "After" needs the next overlapping pushes._
- [x] A documented decision on trimming `performance.yml`'s push-triggered jobs
      (currently `perf_check`, `performance`, `axis_performance`,
      `wasm_axis_performance` — four jobs on every push to `main`): either state
      why all four earn their place on every push, or move one or more to
      `pull_request`-only, `schedule`-only, or `workflow_dispatch`-only, with
      the reasoning recorded in this story's Context or a comment in
      `performance.yml`.

## Technical Tasks

- [x] Add `actionlint` (and any shellcheck it bundles/needs) to `flake.nix`'s
      dev shell `buildInputs`.
- [x] Add an `actionlint .github/workflows/*.yml` line to
      `mask lint`/`mask     lint-check`/`mask all-check`/`mask all-fix` in
      `maskfile.md`, and a step in `lint.yml`.
- [x] Fix or explicitly exclude each of the 14 current shellcheck findings
      (mostly `SC2086` unquoted expansions and `SC2015`/`SC2129` style notes in
      `android-ci.yml`, `performance.yml` and `wasm.yml`).
- [x] Add workflow-file handling to `scripts/pre_commit.sh`'s classifier.
- [x] Redesign `scripts/pre_commit.sh` (and `mask all-check`'s invocation from
      the hook) to operate on the staged index rather than the working tree —
      likely via `git worktree add` against a temporary directory checked out
      from a synthetic commit of the index, or an equivalent stash-based
      approach; update the header comments in both `scripts/pre_commit.sh` and
      `maskfile.md`'s `pre-commit`/`all-check` sections that currently document
      the old (working-tree) behaviour.
- [x] Add the two new seeded-violation cases to `scripts/test_pre_commit.sh`.
- [x] Add `concurrency:` blocks to `performance.yml` and `wasm.yml`.
- [x] Review `performance.yml`'s push-triggered job set and either justify or
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

- [x] `nix run nixpkgs#actionlint -- .github/workflows/*.yml` reports 0 issues
      (down from 14) and is enforced in CI.
- [x] The pre-commit hook's verdict matches `git diff --cached` in isolation,
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

- [x] All Acceptance Criteria are satisfied and checked, except the two CI
      halves that need a push (AC1's CI seed, AC3's "after" observation).
- [x] All tests pass: no Rust source changed. `scripts/test_pre_commit.sh` (44
      cases) passes; the full suite runs in the Tests workflow.
- [x] Lint and format clean: `mask all-check` passed in the hook on three full
      commits (`9aae6c7` and `279e9f7` in place, `b154f2f` inside the snapshot).
- [x] All examples compile: covered by `all-check`'s
      `clippy --workspace --all-targets`.
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Commits**: `5c547d8` workflow shellcheck fixes, `concurrency` for Performance
and WASM, Performance push trim; `9aae6c7` actionlint in the dev shell, maskfile
and hook; `279e9f7` staged-snapshot hook; `b154f2f` actionlint names its files
(a bug found by running the hook in full mode from the snapshot).

- **AC1, workflow lint**: `actionlint` is in `flake.nix` (nixpkgs wraps it with
  shellcheck). `mask lint`, `lint-check`, `all-fix` and `all-check` run
  `actionlint .github/workflows/*.y*ml`, so the Lint workflow does too. Of the
  14 findings, 13 are fixed (quoting, grouped redirects, `if` for
  `A && B || C`); android-ci's `adb shell '... $(getprop ...)'` is a deliberate
  single-quoted string for the device's shell, so it has an inline
  `# shellcheck disable=SC2016` with that reason. actionlint reports 0. The hook
  has a new `workflows` class (scoped, not full): actionlint on every workflow
  file, no Rust check.
- **AC2, staged snapshot**: see `scripts/pre_commit.sh`'s header. If the working
  tree matches the index git hands the hook, the checks run in place (one
  `git diff --quiet` and one `git ls-files --others`). Otherwise the index is
  copied, written to a tree and checked out into
  `$(git rev-parse --git-dir)/gup-pre-commit/tree` with `read-tree --reset -u`
  against the snapshot's own index (only changed files are rewritten), `clean`
  removes debris, and the snapshot's copy of the script re-runs there with
  `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE` pointing at it. No stash, and the
  real working tree and index are never written, so it is safe while another
  process edits the checkout. `mask pre-commit` by hand behaves the same;
  `mask all-check` by hand checks the tree it runs in (documented in
  `maskfile.md`).
- **Shared target directory**: the snapshot uses the checkout's target
  directory, and its `.cargo/config.toml` flips `incremental` for workspace
  members only, so members get their own artifacts and dependencies stay warm.
  Without it, seeded below, the hook passed a staged lint.
- **AC3, concurrency**: `performance.yml` (group includes the event, so a push
  never cancels the weekly benchmark) and `wasm.yml` gained `concurrency`.
  `axis_performance` and `wasm_axis_performance` no longer run per push (pull
  requests, weekly, manual); `performance.yml`'s header gives the reason per
  job. Push-to-main jobs: Performance 4 → 2.

**Key files**: `scripts/pre_commit.sh`, `scripts/test_pre_commit.sh`,
`maskfile.md`, `flake.nix`,
`.github/workflows/{performance,wasm,android-ci,lint,README}`,
`.github/agents/story-worker.md`.

**Tests**: `test_pre_commit.sh` now has 44 cases: 30 classifier cases (5 new for
workflows; `tests.yml` moved from full to workflows), 13 snapshot cases in a
throwaway repository and an actionlint invocation guard. Seeded bugs in the
snapshot code (always in place; no clean and full rewrite; ignoring
`GIT_INDEX_FILE`) each fail it.

**Needs GitHub to confirm**: the Lint workflow running actionlint (and failing
on a pushed seed); the first push with two overlapping runs of Performance or
WASM showing the older one cancelled; and no "job was not acquired by Runner" on
the next few pushes.

## Retrospective

**Completed**: 2026-10-10

### Gate audit: seeded violation → observed failure

The clippy seeds are in `crates/gup-culling-lod` (a leaf crate): `seeded_fn`
returning `Vec::<u8>::new().len() == 0` (`clippy::len_zero`) in `src/lib.rs`, or
the same assertion in an untracked `tests/seeded_untracked.rs`. "OLD" is the
hook at `9aae6c7`, before the snapshot; "NEW" is `279e9f7`/`b154f2f`. Each run
is `./scripts/pre_commit.sh` with `CARGO_TARGET_DIR=/tmp/gup-target`, or
`git commit` where marked. Every seed was reverted afterwards.

| Gate / case                                                | Seeded state                                                                                                     | Observed                                                                                                                                      |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `actionlint`                                               | `git show cf03651:…/visual-regression.yml > .github/workflows/zz-seeded.yml`                                     | exit 1: `"jobs" section is missing`, `"on" section is missing`                                                                                |
| hook, `git commit` (in place)                              | that file staged, nothing else dirty                                                                             | `mode=workflows`, `[gha] … exited with code 1`, commit rejected                                                                               |
| hook, `git commit` (snapshot)                              | that file staged, unrelated untracked file                                                                       | snapshot, `mode=workflows`, actionlint exit 1, commit rejected                                                                                |
| OLD hook, direction 1                                      | staged clean `seeded_fn`; untracked test with the lint                                                           | **exit 1 on `tests/seeded_untracked.rs`: false fail**                                                                                         |
| NEW hook, direction 1                                      | same                                                                                                             | snapshot, exit 0 (16.5 s cold, 2.1 s warm); `git commit` succeeded                                                                            |
| OLD hook, direction 2a                                     | staged clean `seeded_fn`; unstaged lint in the same file                                                         | **exit 1 on `src/lib.rs:42`: false fail**                                                                                                     |
| NEW hook, direction 2a                                     | same                                                                                                             | snapshot, exit 0 (2.1 s)                                                                                                                      |
| OLD hook, direction 2b                                     | staged lint; unstaged fix in the same file                                                                       | **exit 0: false pass (the lint would have reached `main`)**                                                                                   |
| NEW hook, direction 2b                                     | same                                                                                                             | snapshot, exit 1 on `src/lib.rs:42` (3.6 s); `git commit` rejected                                                                            |
| NEW hook without the snapshot's `.cargo/config.toml`       | 2b, after a clean `cargo clippy -p gup-culling-lod` in the real tree (artifact newer than the snapshot's source) | **exit 0: false pass**; the snapshot reused the real tree's artifact                                                                          |
| NEW hook with it                                           | same                                                                                                             | exit 1 on `src/lib.rs:42`                                                                                                                     |
| NEW hook, full mode in the snapshot                        | staged `test_pre_commit.sh` comment; untracked file                                                              | `[gha]` exit 1 on the real checkout's old workflows (bug, fixed `b154f2f`)                                                                    |
| same, after the fix                                        | staged maskfile and scripts; untracked file (`git commit` of `b154f2f`)                                          | all eight checks pass in the snapshot                                                                                                         |
| `test_pre_commit.sh`                                       | snapshot step skipped (`worktree_differs` always false)                                                          | 4 direction cases and the config case fail                                                                                                    |
| `test_pre_commit.sh`                                       | `read-tree` then `checkout-index -a -f` (full rewrite), no `clean`                                               | 3 cases fail (mtime, deletion, debris)                                                                                                        |
| `test_pre_commit.sh`                                       | `index=$gitdir/index` (ignores `GIT_INDEX_FILE`)                                                                 | `GIT_INDEX_FILE is honoured` fails                                                                                                            |
| `test_pre_commit.sh`                                       | `index=$(git rev-parse --git-path index)`                                                                        | passes: `--git-path index` already honours `GIT_INDEX_FILE` (not a bug)                                                                       |
| AC3 before (`gh run list`), pushes `83143d2` and `7f8ad96` | 2026-10-05, pushed 19:31:46 and 19:53:26                                                                         | Lint 37363986951 and Visual regression 37363987153 (grouped) **cancelled** at 19:53:28; Performance 37363987034 (no group) ran on to 20:02:56 |

AC3 "after", and AC1's CI seed, need a push; see the Implementation Summary.

### Hook timing (warm, this machine, `scripts/pre_commit.sh`)

| Change                                           | Before                          | After                                                   |
| ------------------------------------------------ | ------------------------------- | ------------------------------------------------------- |
| docs-only, clean tree (in place)                 | 1.9 s (`git commit` of 799bf7e) | 1.55–1.79 s (5 runs)                                    |
| docs-only, dirty tree (snapshot)                 | n/a (checked the working tree)  | 1.68–1.85 s (5 runs)                                    |
| gup-culling-lod, snapshot                        | n/a                             | 16.5 s first (members, non-incremental), 2.0–3.6 s warm |
| full mode, snapshot (first full in the snapshot) | 49–62 s in place (GUP-398)      | 77 s                                                    |

The snapshot itself is 25 MB (the tracked files). Creating it took 0.6 s; a
re-sync takes about 0.1–0.3 s.

### Key Technical Learnings

#### Two checkouts that share a target directory share member artifacts

- **Challenge**: The snapshot needs warm dependencies, so it should use the
  checkout's target directory. But cargo hashes a workspace member's package ID
  by its path _relative to the workspace root_, so two checkouts of the same
  workspace produce the same artifact names. Freshness is by mtime, so either
  checkout reuses the other's artifact whenever its own sources are older. A toy
  workspace showed `Fresh a` for a second checkout with different content; the
  seeded 2b run above showed the hook passing a staged lint.
- **Solution**: A `.cargo/config.toml` in the snapshot's parent directory (cargo
  reads config from every ancestor of the working directory) sets
  `[profile.dev] incremental = false` and
  `[profile.dev.package."*"] incremental = true`. The profile is part of the
  artifact hash, so members get their own artifacts and dependencies keep the
  checkout's. `codegen-units` and `debug` did not work: setting them on
  `package."*"` also changed the dependencies' build scripts and proc macros
  (whose defaults come from `build-override`), so everything rebuilt.
- **Pattern**: The same reuse happens between any two checkouts that share a
  `CARGO_TARGET_DIR`, which is how agent worktrees run today. In this story the
  real tree reported `Fresh` for gup and gup-text using the snapshot's builds.
  That became GUP-411.

#### Tools that find their project by walking up to `.git`

- **Challenge**: The snapshot lives under the git directory. `actionlint` with
  no arguments walks up to a `.git` and lints the workflows beside it, so from
  the snapshot it linted the real checkout's older workflows. Only a full-mode
  run inside the snapshot showed it.
- **Solution**: Name the files: `actionlint .github/workflows/*.y*ml`.
  `test_pre_commit.sh` rejects a bare `actionlint`. Git itself,
  `mdl --git-recurse`, cargo, rustfmt and prettier behave (environment
  variables, or tracked config files that are found first).
- **Pattern**: When checks move to another directory, run every check there
  once, not just the ones the story is about.

#### `git read-tree --reset -u` with a private index is an incremental checkout

- **Challenge**: Checking out the index for every commit must not rewrite
  unchanged files, or cargo rebuilds everything.
- **Solution**: The snapshot keeps its own index file.
  `read-tree --reset -u <tree>` compares stat data and rewrites only files whose
  blob changed (or that were modified in the snapshot), and deletes files
  dropped from the index. `git write-tree` runs on a copy of the hook's index,
  so its cache-tree update cannot touch git's locked index.

### Architectural Decisions

#### In place when the trees match, snapshot otherwise

- **Decision**: Check in place when `git diff --quiet` and
  `git ls-files --others --exclude-standard` show no difference; use the
  snapshot only otherwise.
- **Reasoning**: In place, the hook reuses the warm, incremental artifacts of
  the developer's own `mask all-fix`. The snapshot's members are non-incremental
  and separate.
- **Trade-off**: Two code paths. Ignored files are assumed not to affect the
  checks.

#### No stash, no writes to the real tree

- **Decision**: A checkout of the index under the git directory, re-synced per
  run, rather than `git stash --keep-index` or a reverse-applied patch.
- **Reasoning**: The stash stack is shared across worktrees, and the failure
  this story fixes happened while another agent was editing the same checkout.
  Anything that rewrites the working tree mid-check races with that agent.
- **Trade-off**: 25 MB per worktree (removed with it) and a second copy of the
  workspace members' check artifacts in the target directory.

#### Workflow files are scoped, not full

- **Decision**: A `workflows` class that runs only actionlint.
- **Reasoning**: A workflow cannot change what the Rust checks see, and CI runs
  the workflow itself. Full mode cost a minute per workflow edit.

#### Performance: two jobs per push, not four

- **Decision**: `axis_performance` and `wasm_axis_performance` run on pull
  requests, weekly and on manual dispatch.
- **Reasoning**: One times frozen old-path axis code; the other's gating steps
  repeat WASM's and Tests'. The concurrency group includes the event name so a
  push never cancels the three-hour weekly benchmark.

### Development Workflow Insights

- The worktree sandbox refuses `git` commands it cannot parse (pipes, env
  prefixes, `sed -f`, `.github` paths in loops). Helpers in `/tmp` that only
  edit files, plus plain `git` commands, kept the seeds moving.
- The seed matrix looked complete until full mode ran inside the snapshot.
  Running the most expensive mode once in the new environment found the one real
  bug.
- One `test_pre_commit.sh` seed ("ignore `GIT_INDEX_FILE`") was not a bug at
  all: `git rev-parse --git-path index` already honours it. When a test does not
  catch a seed, explain the seed before assuming the test is weak.

### Follow-up Stories

1. **GUP-411: Keep checkouts that share a target directory from reusing each
   other's artifacts**. Agent worktrees share `CARGO_TARGET_DIR`, so cargo
   reuses a member artifact built from another checkout's content whenever the
   mtimes allow, and a hook, clippy or test run can pass or fail on code that is
   not in the checkout.

Noted without stories:

- AC1's CI seed and AC3's "after" observation need a push (orchestrator).
- A `build.incremental` set in the user's cargo config overrides the snapshot's
  profile setting, and the member separation is lost. `CARGO_INCREMENTAL` is
  handled. The hook's header documents this.
