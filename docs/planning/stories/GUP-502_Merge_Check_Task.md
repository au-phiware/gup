---
implementer: haiku
implementer-reason:
  a small script running four existing commands in a fixed order with a fixed
  report format; ACs are commands
touches: [maskfile.md, scripts/]
---

# GUP-502: Merge Check Task

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
(2026-10-24) **Created**: 2026-10-24

## Context

Parallel stories collided semantically several times in October: GUP-426's
default legends broke GUP-431's tests and doubled the WASM scenes' hand-placed
legends, so CI went red after a clean textual merge; GUP-494 and GUP-479 merged
2- and 3-wide axis arrays (STRATEGIC_REVIEW_2026-11 §2, "Semantic merge
conflicts"). The orchestrator now runs three checks by hand after every merge:
`gup-core`'s tests with all features, `mask wasm-browser` and `mask wasm-size`.
The review recommends one command for it (recommended change 3).

This story adds `mask merge-check`: those three checks, plus the dogfood suite
on request, each run even when an earlier one fails, with one summary table at
the end. It is deliberately small.

## Read First

- `maskfile.md` `## wasm-size`, `## wasm-browser`, `## dogfood` and
  `## prune-build` (a task with a flag option).
- `scripts/perf_budget.sh` lines 1–40 (how an existing script prints a pass/fail
  table).

## User Story

> "As the orchestrator, after merging one or more story branches I want one
> command that runs the checks textual merges cannot catch, so that a semantic
> conflict is found before I push."

## Acceptance Criteria

### AC1: `scripts/merge_check.sh`

- [x] Runs, in this order, each with its output written to
      `${CARGO_TARGET_DIR:-target}/merge-check/<step>.log` and not to the
      terminal:
  1. `gup-core`: `cargo test -p gup-core --all-features -- --test-threads=1`
  2. `wasm-size`: `./scripts/wasm_size.sh`
  3. `wasm-browser`: `./scripts/wasm_browser.sh`
  4. `dogfood` (only with `--dogfood`): `./dogfood/run_all.sh`
- [x] Every step runs even if an earlier one failed.
- [x] At the end it prints one table, a header line and one line per step:
      `step  result  seconds  log`, with `PASS`, `FAIL` or `SKIP` (dogfood
      without `--dogfood`), then `merge-check: PASS` or
      `merge-check: FAIL (<n> of <m>)`. Exit status 0 only if every run step
      passed.
- [x] For a failed step it also prints the last 20 lines of that step's log
      under the table.
- [x] Before step 1 it prints `git log -1 --oneline` and whether the working
      tree is clean (`git status --porcelain` empty or not).
- [x] `-h` prints usage; an unknown option exits 2 with usage.

### AC2: `mask merge-check`

- [x] `mask merge-check` runs the script, with a `--dogfood` flag option.
- [x] Its description says when to run it (after every merge of story branches,
      before pushing), that it needs a GPU or lavapipe, Chromium and
      `wasm-bindgen` like its parts, and that it takes as long as its parts (no
      new checks).

### AC3: proof

- [x] On a clean `main`, `mask merge-check` prints the table with three `PASS`
      lines and one `SKIP`; paste it in the Implementation Summary. _Run on the
      story branch instead_: `main` plus this story's commits, which touch no
      Rust and no gup-core code. The orchestrator merges, so `main` itself was
      not run.
- [x] Seeded proof, recorded and not committed: with a failing assertion added
      to any `gup-core` test, `mask merge-check` prints `FAIL` for `gup-core`,
      still runs and passes the other steps, prints the 20 log lines, and exits
      non-zero.

## Decisions

- **Exactly the orchestrator's three checks, dogfood opt-in**: these are the
  checks that caught semantic conflicts; dogfood is slower and its inputs change
  rarely. The full `mask all-check` and CI still run on push.
- **No parallelism**: the steps share the build directory lock and the GPU.
- **Logs under the target directory**, not `/tmp`, so they stay with the
  checkout.
- **Bash, `set -uo pipefail` (no `-e`)**, timing with `SECONDS`; short copyright
  header.
- **Dogfood runs through `run_all.sh`**, which sets its own headless environment
  after GUP-501.

## Escalation Triggers

- Any step fails on a clean `main` for a reason unrelated to this story.
- The total run time on a warm build directory exceeds 15 minutes without
  `--dogfood` (record the per-step seconds).

## Out of Scope

- Running `mask all-check`, `mask perf-budget` or CI workflows from the script.
- Detecting which steps a merge needs (always runs all).
- The story-ID check: GUP-504 adds itself to this script.

## Technical Tasks

- [x] Start commit within 30 minutes.
- [x] Write `scripts/merge_check.sh`; add `mask merge-check`.
- [x] Run it on the story branch (long: background or a 600000 ms timeout;
      checkpoint first); run the seed. Not run on `main` (see AC3).
- [x] `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-501: Headless Environment Script ✅ — `run_all.sh` sets its own
  environment, so `--dogfood` works from any shell.
- GUP-496: Checkpoint Script 📋 — soft, for the DoD's checkpoint line.

### Enables Stories

- GUP-504: Story ID Guard — adds its check as a step.

## Testing Strategy

- **Manual**: one clean run and one seeded failure (AC3).
- **Lint**: shellcheck through `mask all-check`'s workflow lint does not cover
  scripts; `shellcheck scripts/merge_check.sh` was run and is clean. Gap tracked
  in GUP-517.

## Success Metrics

- [x] A clean run: three PASS, one SKIP, exit 0.
- [x] A seeded run: one FAIL, the rest PASS, exit non-zero.

## Risk Assessment

- **Low**: the browser step flakes under load (review §2, "Flaky timing gates").
  _Mitigation_: the table shows which step failed and the log; rerun that step
  alone.

## Definition of Done

- [x] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-check` passes
- [x] Lint and format clean: `mask all-fix`
- [x] `mask wasm-browser`: run as part of AC3 (no wasm code change)
- [x] WASM size: the scatter gz bytes from AC3's `wasm-size` step (expected
      unchanged)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2026-10-24

Added `scripts/merge_check.sh` (new) and the `mask merge-check` task (in
`maskfile.md`, after `## dogfood`). The script runs gup-core's tests with all
features, `wasm-size` and `wasm-browser`, each even when an earlier step failed,
then `dogfood/run_all.sh` with `--dogfood`. Each step's output goes to
`$CARGO_TARGET_DIR/merge-check/<step>.log`. It prints one table, then
`merge-check: PASS` or `merge-check: FAIL (<failed> of <run>)`, and exits 0 only
when every run step passed. Failed steps get their last 20 log lines under the
table. Story GUP-517 (new) records the gap that no script is shellcheck-linted
by the gate.

Clean run on the story branch (`mask merge-check`, 2026-10-24, log paths shown
as `<merge-check>`):

```text
merge-check: HEAD: ab8fe1d4 GUP-502: scripts/merge_check.sh and mask merge-check
merge-check: working tree is clean

step          result  seconds  log
gup-core      PASS        367  <merge-check>/gup-core.log
wasm-size     PASS         40  <merge-check>/wasm-size.log
wasm-browser  PASS          8  <merge-check>/wasm-browser.log
dogfood       SKIP          -  -

merge-check: PASS
```

Seeded failure (a temporary `assert_eq!(1 + 1, 3)` test appended to
`crates/gup-core/src/lib.rs`, reverted with `git checkout`, not committed):

```text
step          result  seconds  log
gup-core      FAIL         89  <merge-check>/gup-core.log
wasm-size     PASS         39  <merge-check>/wasm-size.log
wasm-browser  PASS          4  <merge-check>/wasm-browser.log
dogfood       SKIP          -  -

--- gup-core: last 20 lines of <merge-check>/gup-core.log
(20 lines: the panic and backtrace of seeded_failure)

merge-check: FAIL (1 of 3)
```

The exit status was 1. The tree was clean again afterwards.

`--dogfood` run (the path the clean and seeded runs did not cover): all four
steps PASS (gup-core 256 s, wasm-size 37 s, wasm-browser 5 s, dogfood 46 s),
`merge-check: PASS`.

Evidence from the logs: gup-core lib tests 195 passed, 0 failed, 3 ignored
(pre-existing). `wasm-size`: scatter 334155 B gz against 334149 B recorded in
PERF_BUDGETS.md (+6 B, within the 2% gate; treated as unchanged). Bare gup-core
over wgpu is 301795 B against the 400000 B budget. `wasm-browser`: Chrome/155 on
SwiftShader, `GUP PASS`.

Gates: `mask all-fix` and `mask all-check` both exit 0 on the branch.
`shellcheck scripts/merge_check.sh` is clean. The `mask merge-check --help`
output is the one-line blockquote; the rest of the description is the body.

Commits: `f1886b52` (Start), `ab8fe1d4` (script and task), `4ddec33f` (the
description fix), and this completion commit.

Tests: no new Rust tests (the change is a shell script and a mask task). The
seeded run is the proof for AC3.

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### mask descriptions and Markdown wrapping

- **Challenge**: The task description is the blockquote. Prettier wraps it at 80
  columns, and mask joins the wrapped lines with no separator, so the help text
  read `textualmerges`. A trailing space on the first line does not survive
  prettier. MD013 (mdl's line-length rule) forbids the one-line alternative.
- **Solution**: One blockquote line of at most 80 columns, with the when, needs
  and duration in the first body paragraph.
- **Pattern**: Keep a mask description to a single short line. Put detail in the
  body, and check `mask <task> --help` after editing the text.

#### Formatting before commit

- **Challenge**: The hook refused the first commit: `maskfile.md` was not
  prettier-formatted.
- **Solution**: Run the pinned prettier (`prettier --write maskfile.md`) before
  staging any Markdown edit, then commit.

### Architectural Decisions

#### Each step's output goes to a log under the target directory

- **Decision**: Logs go to `${CARGO_TARGET_DIR:-target}/merge-check/`, not
  stdout. Only the progress line and the table reach the terminal.
- **Reasoning**: The story required it. The logs stay with the checkout and
  survive for the 20-line failure excerpt.
- **Trade-off**: A long run is quiet until the table, apart from the progress
  lines.
- **Future**: GUP-504 adds a step by appending one `run_step` line.

### Development Workflow Insights

- **Decisions I made (not in the story)**: `FAIL (<n> of <m>)` counts failed
  steps against steps run (SKIP excluded). A progress line
  `merge-check: running <step>` goes to the terminal, which the story did not
  forbid. The script `cd`s to the repository root, so relative
  `CARGO_TARGET_DIR` values resolve there.
- **Clean-main proof**: AC3 asks for a run on `main`. This worktree branch has
  `main` plus story-only commits (no Rust, no gup-core code), so the clean run
  is on the branch. The orchestrator should confirm the table on `main` after
  the merge.
- **Sandbox guard**: the worktree guard refused git commands that were combined
  with other commands (a `git commit` after `with_cargo_env.sh`, and a heredoc
  written into `lib.rs`). Plain, separate commands passed.
- **Timing**: the gup-core step took 367 s on the clean run and 256 s on the
  `--dogfood` run, with the build directory warm for the second. Each whole run
  was under 8 minutes, well inside the 15-minute escalation trigger.

### Follow-up Stories

1. **GUP-517: Shellcheck the Scripts in the Gate** — `mask all-check` lints only
   the workflow `run:` blocks, so `scripts/*.sh` and `dogfood/run_all.sh` have
   no shellcheck gate. Written as `📋 Planned`.
