---
implementer: haiku
implementer-reason:
  a small script running four existing commands in a fixed order with a fixed
  report format; ACs are commands
touches: [maskfile.md, scripts/]
---

# GUP-502: Merge Check Task

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 📋 Planned
**Created**: 2026-10-24

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

- [ ] Runs, in this order, each with its output written to
      `${CARGO_TARGET_DIR:-target}/merge-check/<step>.log` and not to the
      terminal:
  1. `gup-core`: `cargo test -p gup-core --all-features -- --test-threads=1`
  2. `wasm-size`: `./scripts/wasm_size.sh`
  3. `wasm-browser`: `./scripts/wasm_browser.sh`
  4. `dogfood` (only with `--dogfood`): `./dogfood/run_all.sh`
- [ ] Every step runs even if an earlier one failed.
- [ ] At the end it prints one table, a header line and one line per step:
      `step  result  seconds  log`, with `PASS`, `FAIL` or `SKIP` (dogfood
      without `--dogfood`), then `merge-check: PASS` or
      `merge-check: FAIL (<n> of <m>)`. Exit status 0 only if every run step
      passed.
- [ ] For a failed step it also prints the last 20 lines of that step's log
      under the table.
- [ ] Before step 1 it prints `git log -1 --oneline` and whether the working
      tree is clean (`git status --porcelain` empty or not).
- [ ] `-h` prints usage; an unknown option exits 2 with usage.

### AC2: `mask merge-check`

- [ ] `mask merge-check` runs the script, with a `--dogfood` flag option.
- [ ] Its description says when to run it (after every merge of story branches,
      before pushing), that it needs a GPU or lavapipe, Chromium and
      `wasm-bindgen` like its parts, and that it takes as long as its parts (no
      new checks).

### AC3: proof

- [ ] On a clean `main`, `mask merge-check` prints the table with three `PASS`
      lines and one `SKIP`; paste it in the Implementation Summary.
- [ ] Seeded proof, recorded and not committed: with a failing assertion added
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

- [ ] Start commit within 30 minutes.
- [ ] Write `scripts/merge_check.sh`; add `mask merge-check`.
- [ ] Run it on `main` (long: background or a 600000 ms timeout; checkpoint
      first); run the seed.
- [ ] `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-501: Headless Environment Script 📋 — `run_all.sh` sets its own
  environment, so `--dogfood` works from any shell.
- GUP-496: Checkpoint Script 📋 — soft, for the DoD's checkpoint line.

### Enables Stories

- GUP-504: Story ID Guard — adds its check as a step.

## Testing Strategy

- **Manual**: one clean run and one seeded failure (AC3).
- **Lint**: shellcheck through `mask all-check`'s workflow lint does not cover
  scripts; run `shellcheck scripts/merge_check.sh` from the dev shell if
  available, else `bash -n`.

## Success Metrics

- [ ] A clean run: three PASS, one SKIP, exit 0.
- [ ] A seeded run: one FAIL, the rest PASS, exit non-zero.

## Risk Assessment

- **Low**: the browser step flakes under load (review §2, "Flaky timing gates").
  _Mitigation_: the table shows which step failed and the log; rerun that step
  alone.

## Definition of Done

- [ ] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-check` passes
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser`: run as part of AC3 (no wasm code change)
- [ ] WASM size: the scatter gz bytes from AC3's `wasm-size` step (expected
      unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
