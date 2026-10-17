---
implementer: sonnet
implementer-reason:
  design settled in Decisions; edits follow existing harness, script and
  workflow patterns
---

# GUP-438: Trustworthy Goldens and Size Gates

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: ✅ Complete (2026-10-17)
**Created**: 2026-10-17

## Context

Three gates have been found untrustworthy while landing GUP-421, GUP-426 and
GUP-431.

1. **Blessing can leave stale goldens.** `GoldenStore::check`
   (`crates/gup-visual-regression/src/golden.rs`, lines 105-152) returns
   `Matched` as soon as the render is within tolerance, before it reaches the
   `if self.bless` branch. With `GUP_BLESS=1`, a change within tolerance
   therefore keeps the old PNG, and a reviewer reading committed PNGs sees the
   old image. GUP-426 and GUP-431 each had to delete goldens to force a rewrite.
2. **`mask wasm-browser` breaks locally.** The dev shell's `LD_LIBRARY_PATH`
   makes Chromium load a nix libm that lacks GLIBC_2.43, so Chromium dies at
   startup. GUP-421 (story line 375) worked around it by hand with
   `env -u LD_LIBRARY_PATH`; GUP-426 hit it again. `scripts/browser_smoke.mjs`
   already reports "exited before the page reported" (lines 117-123), but the
   message does not say why.
3. **WASM size drift is not caught by CI.** The Visual regression workflow runs
   `./scripts/wasm_size.sh` (`.github/workflows/visual-regression.yml`, lines
   165-168), which enforces only the 400,000 B over-wgpu ceiling. The
   `wasm.scatter.gz_bytes` row (+2%, `crates/gup-core/PERF_BUDGETS.md` line 78)
   is checked only by local `mask perf-budget`. Stories merged since GUP-421
   drifted +1.9% unnoticed, and parallel stories each re-recorded the budget
   against their own base. The size depends only on the tree and the pinned
   toolchain, so CI can enforce it.

Note: the bless logic lives in `golden.rs`, not `harness.rs`; the harness only
reports the outcome (`harness.rs` line 195).

## Read First

- `crates/gup-visual-regression/src/golden.rs` lines 20-60 (`BlessReason`,
  `GoldenOutcome`), 105-152 (`check`), 250-330 (existing bless tests)
- `crates/gup-visual-regression/src/harness.rs` lines 180-200 (the `BLESS`
  report line) and 210-255 (harness bless test)
- `tests/golden/README.md` (bless workflow text) and
  `.github/agents/story-worker.md` lines 195-207 (visual-test guidance)
- `scripts/wasm_browser.sh` (whole file) and `scripts/browser_smoke.mjs` lines
  84-123
- `scripts/wasm_size.sh` lines 60-77 and `scripts/perf_budget.sh` lines 10-50
  and 100-165 (table parse and compare)
- `crates/gup-core/PERF_BUDGETS.md` lines 49-80 (table), 145-196 (WASM size)
- `.github/workflows/visual-regression.yml` lines 100-175 (browser job)

## User Story

> "As a story author, I want `GUP_BLESS=1` to leave the committed goldens equal
> to the current render, `mask wasm-browser` to run from the dev shell, and CI
> to fail on WASM size drift, so that I can trust what reviewers see and what
> the gates say."

## Acceptance Criteria

### AC1: Bless always writes the current render

- [x] In bless mode `GoldenStore::check` writes the render to the golden path in
      every case: missing, size change, out of tolerance, and within tolerance
- [x] A within-tolerance render in bless mode returns
      `GoldenOutcome::Blessed { reason: BlessReason::Refreshed(stats), .. }`
- [x] Unit test in `golden.rs`: bless a golden, then check with a render that
      differs within tolerance (use `image(15.0)` as the existing test does);
      with bless on, the golden file's decoded pixels equal the new render
      exactly (and differ from the original); with bless off, the outcome is
      `Matched` and the file is untouched
- [x] Bless-off behaviour is unchanged: the existing tests pass without edits to
      their assertions

### AC2: Every blessed golden is verified as re-rendered

- [x] After writing in bless mode, `check` reloads the golden and compares it
      pixel-for-pixel with the render; a mismatch returns a `Check::Golden`
      failure `golden <path> was not refreshed by this bless run`
- [x] Unit test the helper `verify_refreshed(path, actual) -> Result` directly
      with a matching image (Ok) and a different one (Err containing "not
      refreshed")
- [x] Running `GUP_BLESS=1 mask visual-regression` and the gup-core golden tests
      with `GUP_BLESS=1` twice in a row on an unchanged tree leaves
      `git status --porcelain tests/golden` identical after the second run as
      after the first (bless is idempotent on this machine; no churn from
      nondeterminism). Read each golden that the first run changed and say why
      it changed

### AC3: Behaviour is documented

- [x] The `check` doc comment (lines 105-113) no longer says an in-tolerance
      golden is left alone; it states bless always writes
- [x] `tests/golden/README.md` and the "Intentional visual change" bullet in
      `.github/agents/story-worker.md` (line 204) state: `GUP_BLESS=1` rewrites
      every golden the run renders, so `git diff --stat tests/golden` lists the
      goldens whose bytes changed; read each one; never delete goldens to force
      a rewrite
- [x] The `BLESS` report line in `harness.rs` prints the `Refreshed` reason

### AC4: `mask wasm-browser` works with `LD_LIBRARY_PATH` set

- [x] `scripts/browser_smoke.mjs` spawns the browser with `LD_LIBRARY_PATH`
      removed from its environment (`env` option: a copy of `process.env`
      without the key); `scripts/wasm_browser.sh` also runs node under
      `env -u LD_LIBRARY_PATH` so the launcher does not depend on it
- [x] When the browser exits before the page reports, the error includes the
      first 20 lines of the browser's stderr and the hint
      `check LD_LIBRARY_PATH and GUP_CHROMIUM`
- [x] From a dev shell with a non-empty `LD_LIBRARY_PATH` (show
      `echo "$LD_LIBRARY_PATH"`), `mask wasm-browser` exits 0 and prints
      `GUP PASS`. Read
      `$CARGO_TARGET_DIR/visual-regression/gup_core/browser_scatter.png` and
      state what it shows (the scatter with axes, as in
      `tests/golden/gup_core/`)
- [x] Seeded proof: `GUP_CHROMIUM=/bin/false mask wasm-browser` exits non-zero
      with the "exited before the page reported" message and the hint

### AC5: CI enforces the WASM size budget

- [x] `scripts/perf_budget.sh` gains `--wasm-only`: it checks only budget rows
      whose metric starts with `wasm.` (other rows are skipped, not "not
      measured"); with `--metrics FILE` it measures nothing
- [x] `scripts/wasm_size.sh`, after printing its metric lines, runs
      `perf_budget.sh --wasm-only --metrics <those lines>` and exits non-zero on
      failure, unless `GUP_WASM_SIZE_NO_BUDGET=1` (set by `perf_budget.sh` when
      it calls `wasm_size.sh`, to avoid recursion). `mask wasm-size` therefore
      fails when `wasm.scatter.gz_bytes` is over +2% of the recorded value
- [x] Seeded proof in both directions, using a copy of `PERF_BUDGETS.md` passed
      via `--budgets`: the recorded scatter value lowered by 3% gives `FAIL`;
      the real value passes
- [x] The Visual regression browser job's step (lines 165-168) is renamed
      `Check gup-core's WASM size budget (mask wasm-size)`, still runs
      `./scripts/wasm_size.sh`, and its comment names both checks (ceiling and
      scatter budget)
- [x] `PERF_BUDGETS.md` "WASM size" says CI enforces the scatter budget and the
      ceiling, and that a story changing the size re-records the row in the same
      commit against current `main`
- [x] After the first CI run on the branch, compare CI's `wasm.scatter` figure
      with the local one. If CI differs, record CI's figure in the table and say
      in PERF_BUDGETS.md that the reference is CI's (local figure alongside). If
      they differ by more than 1%, stop (see triggers)

## Decisions

- **Bless semantics**: bless always writes; no tolerance gate on writing.
  `BlessReason` gains `Refreshed(stats)` (the same stats type as `Changed`). A
  within-tolerance render in non-bless mode is still `Matched`.
- **Verification**: a private
  `verify_refreshed(path: &Path, actual: &RgbaImage) -> Result<(), String>` in
  `golden.rs`, called after each bless write. It is a read-back check, not a
  manifest: a per-process manifest cannot see other test binaries. Detecting
  orphaned goldens (files no case renders) is deliberately not done here.
- **Browser env**: unset `LD_LIBRARY_PATH` in both the shell wrapper and the
  node spawn. Chromium needs nothing from it.
- **Size gate**: reuse the PERF_BUDGETS.md table and `perf_budget.sh`'s
  comparison rather than a second budget in `wasm_size.sh`: one source of truth.
- Timing budgets are not enforced in CI (see Out of Scope).

## Escalation Triggers

- A bless run on an unchanged tree changes any golden's bytes (AC2 idempotence
  fails). Always-write would then churn goldens across machines; report which
  files before choosing a mitigation.
- `GoldenStore` bless is shared with non-gup-core goldens (chart builders,
  `tests/examples_smoke.rs`) such that always-write rewrites many unrelated PNGs
  on the first bless run.
- CI's `wasm.scatter.gz_bytes` differs from local by more than 1%.
- Chromium still fails to start with `LD_LIBRARY_PATH` unset (a different
  cause).
- `perf_budget.sh --wasm-only` needs more than a row filter to work.

## Out of Scope

- Perf timing budgets in CI (noisy); they stay in local `mask perf-budget`.
- Detecting orphaned goldens that no case renders.
- Changing tolerances or the diff metric.

## Technical Tasks

- [x] Add `BlessReason::Refreshed`; restructure `check` so bless writes before
      the within-tolerance early return; add `verify_refreshed`
- [x] Update `harness.rs` report text; add the unit tests
- [x] Update `check` docs, `tests/golden/README.md`, `story-worker.md`
- [x] Run bless twice for idempotence; review `git diff --stat tests/golden`
- [x] `browser_smoke.mjs`: env without `LD_LIBRARY_PATH`, stderr tail and hint
- [x] `wasm_browser.sh`: `env -u LD_LIBRARY_PATH node ...`
- [x] `perf_budget.sh --wasm-only`; `wasm_size.sh` budget call and
      `GUP_WASM_SIZE_NO_BUDGET`; maskfile `wasm-size` text
- [x] Workflow step rename and comment; PERF_BUDGETS.md text and, if needed, the
      CI-reference note

## Dependencies

### Prerequisite Stories

- GUP-388: Visual Regression Harness ✅ — the harness
- GUP-417: Performance budgets ✅ — `perf_budget.sh` and the table
- GUP-408: Browser smoke test in CI ✅ — `wasm_browser.sh`

### Enables Stories

- Every later visual story: goldens in review match what was rendered.

## Testing Strategy

- **Unit tests**: bless within tolerance (AC1), `verify_refreshed` (AC2)
- **Integration tests**: bless twice and compare `git status`;
  `mask wasm-browser` with `LD_LIBRARY_PATH` set; seeded `/bin/false` browser;
  seeded budget copies for `--wasm-only`
- **Visual validation**: read the browser PNG and one re-blessed golden
- **Performance**: none

## Success Metrics

- [x] `git diff --stat tests/golden` after a bless run lists only changed bytes
- [x] `mask wasm-browser` passes with `LD_LIBRARY_PATH` set
- [x] A 3% scatter size regression fails `mask wasm-size` locally and in CI

## Risk Assessment

- **Medium**: always-write may churn goldens if rendering is nondeterministic
  across machines. _Mitigation_: the AC2 idempotence check and the first
  escalation trigger.
- **Low**: CI's WASM size differs from local (different toolchain build).
  _Mitigation_: record CI's figure as the reference (AC5).

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `golden.rs`: `BlessReason::Refreshed`, bless always writes then
  `verify_refreshed` reads back; two new unit tests (6 golden tests pass);
  `harness.rs` prints `refreshed: ...`.
- `browser_smoke.mjs`/`wasm_browser.sh`: `LD_LIBRARY_PATH` removed for Chromium
  and node; early exit prints 20 stderr lines and the hint.
- `perf_budget.sh --wasm-only`, `wasm_size.sh` runs it
  (`GUP_WASM_SIZE_NO_BUDGET` guards recursion); workflow comment, maskfile,
  PERF_BUDGETS.md, README, story-worker.md updated.

### Evidence

- Bless twice (gup-core tests plus `mask visual-regression`, `GUP_BLESS=1`):
  `git status --porcelain tests/golden` empty after both runs; no golden bytes
  changed, so always-write causes no churn on this machine.
- `mask wasm-browser` with a non-empty nix `LD_LIBRARY_PATH` exited 0 with
  `GUP PASS`. `browser_scatter.png` shows the six-panel reference scatter (axes,
  legends, colour bars). Seeded failure: `GUP_CHROMIUM=$(command -v false)`
  exits 1 with "exited (1) before the page reported; check LD_LIBRARY_PATH and
  GUP_CHROMIUM" (`/bin/false` does not exist on NixOS, so `false` from PATH).
- Seeded budgets: scatter recorded lowered ~3% gives `FAIL` (exit 1); real value
  passes. `mask wasm-size` measured scatter 329,486 vs recorded 329,548: pass.
- Not done: comparing CI's `wasm.scatter` figure with the local one needs a CI
  run after push; the orchestrator should check it (AC5 last item, trigger >1%).

## Retrospective

**Completed**: 2026-10-17

### Key Technical Learnings

#### Early-return before the mode branch

- **Challenge**: `check` returned `Matched` before the bless branch.
- **Solution**: decide the reason, then bless-write, then verify by read-back.
- **Pattern**: a gate with a mode flag should branch on the flag first.

#### Script traps

- **Challenge**: a second `trap ... EXIT` in `wasm_size.sh` would have replaced
  the temp-dir cleanup.
- **Solution**: one trap covering both paths.

### Architectural Decisions

- Size budget reuses the PERF_BUDGETS.md table via `--wasm-only`; no second
  budget in `wasm_size.sh`.

### Development Workflow Insights

- Seeded checks used a metrics file with `--budgets` copies, so no rebuild was
  needed to prove both directions.

### Follow-up Stories

None.
