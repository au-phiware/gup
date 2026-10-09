# GUP-404: Triage Every Ignored Test

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 📋 Planned **Created**:
2026-10-06

## Context

The first full-workspace run of the new Tests workflow
(`.github/workflows/tests.yml`, added by GUP-398) reports:
`cargo test --workspace` on lavapipe produced 131 test-result lines (one per
binary — libs, integration-test files, and `--doc`), 6,691 passed, 0 failed,
**278 ignored** (GitHub Actions run
[37325460224](https://github.com/au-phiware/gup/actions/runs/37325460224) on
commit `d98af8d`, confirmed directly from the job log, not estimated).

A first-pass static scan of the source (`grep -rn '#\[ignore' --include='*.rs'`,
plus rustdoc `ignore` doctest fences) finds 93 `#[ignore]` test attributes
across 24 files and 95 rustdoc-ignored doctest code blocks across 42 files —
together only ~200, not 278. The gap is not yet explained (candidates: doctests
counted differently between `cargo test --doc` and the workflow's feature-matrix
re-runs via `scripts/test_debug_feature.sh`, or ignores inside
`#[cfg(feature = "debug")]` modules not visible to a feature-less grep).
Reconciling the static inventory against the workflow's authoritative count,
binary by binary, is this story's first task — the count must be explained, not
assumed.

Of the 93 `#[ignore]` attributes, 74 are GUP-398's deliberately quarantined
wall-clock-budget tests, each carrying the standard reason string
`"wall-clock budget: unreliable on shared CI runners; run with --ignored on a hardware GPU (GUP-398)"`.
The other 19 are unstandardized: 15 are bare `#[ignore]` justified only by a
trailing `//` comment (not a Rust reason string), and 4 carry one-off reason
strings. Some of these are clearly hardware/display constraints
(`crates/gup-core/tests/window_parity.rs`: "opens a window: needs a display").
Some are clearly known, unfixed bugs hidden behind `#[ignore]` rather than
tracked as failures — most notably `tests/visual_blend_mode_tests.rs:115`
("TODO: Fix multiply blend mode - currently causes GPU timeout") and `:279`
("TODO: Fix GPU resource management for multiple renderings"), and
`tests/histogram_tests.rs:195` ("GPU histogram has floating-point precision
issues with large datasets at exact bin boundaries"). Three in
`tests/gpu_statistics_integration_tests.rs` cite "GUP-149" for a
multi-workgroup-support limitation, but `docs/planning/stories/INDEX.md` lists
GUP-149 twice, for two unrelated ✅-complete stories (automatic device-loss
detection; box-plot GPU rendering) — neither obviously delivers multi-workgroup
compute support, so this reference may be stale or wrong, and the underlying
limitation may already be fixed under a different story. Four more
(`src/text/atlas.rs:515`, `src/text/renderer.rs:853,879,929`) are ignored
because their test mocks use `unsafe { mem::zeroed() }` on wgpu types — a
test-authoring hazard, not a product requirement, that should be fixed with a
real mock rather than left disabled indefinitely.

**Confirmed category-(d) hidden failure (2026-10-10):**
`tests/gpu_statistics_integration_tests.rs::test_statistics_compute_uniform_distribution`
is flaky in CI (it failed on `e5f69ef` with `left: 1e38, right: 5.0` and passed
on re-run). It is now ignored with a reason citing this story. The root cause,
from `src/shaders/statistics.compute.wgsl`: `compute_basic_stats` is written for
a single workgroup (its own comment: "single workgroup only - AC3 will add
multi-workgroup") but is dispatched over several. Every workgroup's thread 0
writes `result` (last writer wins), workgroups past the data write `min = 1e38`,
and `data_size` is read from `result.count` while other workgroups overwrite it.
The three other tests in the same file that cite "GUP-149" share this root
cause: multi-workgroup support was never built. The shader is frozen old-path
code (`src/shader_function/`, deleted at RFC-001 S14), so it is not to be fixed.

This project has a documented history of gates that look like they enforce
something and don't: the strategic review's root-cause table lists "quality gate
routinely bypassed" (`--no-verify` in 63 retrospectives) as a direct contributor
to the state the review was called to fix, and GUP-398's own audit found two
more gates that had been silently failing open (a non-recursing whitespace glob;
`clippy --fix -- -D warnings`, which cannot fail on a lint it cannot auto-fix)
for an unknown period before anyone deliberately tried to break them.
`#[ignore]` is exactly this kind of place: a test that is ignored for a good
reason (needs hardware CI doesn't have, or a wall-clock budget that is
unreliable on shared runners) looks identical in `cargo test` output to one that
is ignored because it is broken and nobody got around to fixing it or even
recording that it's broken. GUP-398's own retrospective found this gap and
explicitly declined to fix it ("Quarantined wall-clock tests (74): ... Revisit
at S14 rather than now" — Follow-up Stories), leaving the other 19
non-standardized ignores, and the eventual authoritative-count reconciliation,
untouched.

RFC-001 (accepted 2026-10-04) freezes the old render path (`src/selection.rs`,
`src/mark/`, `src/shader_function/`, `src/shader_pipeline.rs`,
`src/chart_builder*`, `src/context.rs`, `src/render.rs`) ahead of its step S14
flip-and-delete. Several of the 95 rustdoc `ignore` fences live inside these
frozen files. Per the strategic review and RFC-001, this story must not spend
effort fixing frozen-path test or doctest failures — those get deleted (or left
ignored with a reason citing S14) rather than repaired, since S14 deletes the
code they exercise anyway.

## User Story

> "As a maintainer, I want every ignored test and ignored doctest classified
> into a known category with a standard, checked reason string, so that a hidden
> failure can't sit silently in the ignore list the way the clippy and
> whitespace gates once failed open without anyone noticing."

## Acceptance Criteria

### AC1: A reconciled, checked-in inventory of every ignored test

- [ ] A new file (e.g. `docs/planning/IGNORED_TEST_INVENTORY.md`) lists every
      `#[ignore]` test attribute and every rustdoc `ignore` doctest fence in the
      workspace: file, line, test/doctest name, current reason (if any), and the
      category (a/b/c/d, per the Context's definitions) and disposition assigned
      to it.
- [ ] The inventory's total is reconciled against the Tests workflow's
      authoritative ignored count (re-run and read from the job log, the same
      way this story's research did via `gh run view --job <id> --log`), with
      any remaining gap between the static inventory and the workflow count
      explained in the inventory's preamble, not left as an unexplained number.
- [ ] Every row has exactly one of these dispositions: kept ignored with a
      standard reason (a), moved into a Performance-workflow timing binary (b),
      deleted (c), or fixed / tracked as an expected failure with a named owner
      (d).

### AC2: A lint rejects an unreasoned `#[ignore]`

- [ ] A script (e.g. `scripts/check_ignore_reasons.sh`) scans every `.rs` file
      for `#[ignore]` test attributes and fails, listing file:line, if the
      attribute is bare or its reason string does not start with one of a fixed,
      documented set of allowed prefixes (for example: `"wall-clock budget: "`,
      `"needs hardware/display: "`, `"expected failure (tracked): "`). The
      allowed-prefix set is defined in one place in the script.
- [ ] The check is wired into `mask all-check` following GUP-398's
      proportional-hook pattern (skipped for docs-only commits, scoped to
      touched crates otherwise, and run unscoped in CI's Lint job).
- [ ] A deliberately seeded bare `#[ignore]` makes the check fail; the violation
      is reverted and the observed failure recorded (per GUP-398's
      seeded-violation convention).
- [ ] Every rustdoc `ignore` fence has a one-line comment directly above it
      stating why (the same four categories apply); a second, equally simple
      check enforces this for new fences going forward.

### AC3: Every category-(a) test is actually exercised somewhere

- [ ] For every test kept ignored under category (a) (needs hardware or a
      display unavailable in CI), either an existing or new CI job runs it with
      `--ignored` (documented in the inventory row), or the inventory row
      explicitly records why no CI job can run it and what would need to be true
      to change that — mirroring GUP-398 AC3's "reasoned exception" bar rather
      than a silent skip.

### AC4: The ignored count is reduced and recorded, before and after

- [ ] The Tests workflow's ignored count before this story (278,
      run 37325460224) and after this story (next run on `main` once this story
      merges) are both recorded in this story's retrospective, together with a
      breakdown by category showing which changes moved the number (c) deletions
      and (d) fixes reduce it; (a)/(b) reclassifications may not).
- [ ] No test remains ignored under category (d) (hidden failure) without either
      being fixed, or being explicitly tracked as an expected failure with a
      named owner and a reason string pointing at the tracking story/issue.

## Technical Tasks

- [ ] Enumerate every `#[ignore]` attribute
      (`grep -rn '^\s*#\[ignore' --include='*.rs' .`) and every rustdoc `ignore`
      fence (grep for a triple-backtick-ignore fence or a `,ignore` language
      tag) across the whole workspace (`src/`, `tests/`, `crates/*/src`,
      `crates/*/tests`, `benches/`, `dogfood/`, `examples/`); record file, line
      and current reason/comment for each.
- [ ] Re-run (or read the latest run of) the Tests workflow's job log and sum
      every `test result:` line's `ignored` count, per binary; reconcile this
      against the static inventory and write the explanation into the
      inventory's preamble.
- [ ] Classify every row using the Context's four categories, cross-referencing
      RFC-001's frozen-path file list for category (c) candidates.
- [ ] Category (a): standardize the reason string to the
      `"needs hardware/display: ..."` prefix; confirm (or add) a CI job that
      runs it with `--ignored`, or write the documented exception.
- [ ] Category (b): standardize to the `"wall-clock budget: ..."` prefix (reuse
      GUP-398's existing string where it already applies) and confirm each test
      is actually exercised by the Performance workflow (following the
      `test = false` + named-binary pattern GUP-398 used for
      `interaction_performance_tests`, `cross_platform_axis_performance_tests`
      and `performance_ci_tests`); add it to a timing binary if it is not yet
      run anywhere.
- [ ] Category (c): confirm the test/doctest exercises only frozen-old-path or
      already-obsolete behaviour, then delete it (and any code it alone
      exercised). Do not fix frozen-path test failures — delete or defer to S14,
      per the strategic review's parked-work rule.
- [ ] Category (d): for each (the two `visual_blend_mode_tests.rs` GPU-timeout
      /resource-management TODOs, the `histogram_tests.rs` floating-point
      precision bug, the four `mem::zeroed()` test-mock hazards in
      `text/atlas.rs` and `text/renderer.rs`, and the three
      `gpu_statistics_integration_tests.rs` multi-workgroup entries — check
      first whether the cited "GUP-149" work already covers them under a
      different ID before assuming the limitation still exists): either fix and
      un-ignore, or add a `"expected failure (tracked): GUP-NNN"` reason and
      name an owner. Split any fix large enough to need its own story, and name
      it in this story's Follow-up Stories section.
- [ ] Write `docs/planning/IGNORED_TEST_INVENTORY.md` per AC1.
- [ ] Write and wire in `scripts/check_ignore_reasons.sh` (and the doctest-fence
      comment check) per AC2, following GUP-398's `scripts/pre_commit.sh`
      scoping pattern; add a small test script for it mirroring
      `scripts/test_pre_commit.sh`'s seeded-case approach.
- [ ] Seed and revert a bare `#[ignore]` to prove the new check fails (AC2).
- [ ] Record the before/after ignored counts and the category breakdown in the
      retrospective (AC4).

## Dependencies

### Prerequisite Stories

- GUP-398: Honest and Fast Quality Gates ✅ — established the
  `"wall-clock budget: ... (GUP-398)"` reason-string convention, the
  proportional pre-commit hook this story's new check plugs into, and the Tests
  workflow whose job log is this story's source of truth for the ignored count.

### Enables Stories

- RFC-001 S14 (the old-path flip and delete): starts from a classified,
  already-pruned ignore list instead of discovering stale frozen-path tests
  during the deletion itself.

## Testing Strategy

- **Unit tests**: `scripts/check_ignore_reasons.sh` gets its own seeded-case
  test script (bare `#[ignore]`, allowed-prefix reason, disallowed-prefix
  reason, a doctest fence with and without the required comment).
- **Integration tests**: each category-(d) fix is verified by running the test
  with `--ignored` before un-ignoring it, then confirming it passes unignored in
  the normal suite.
- **Visual validation**: the `visual_blend_mode_tests.rs` fixes (blend-mode GPU
  timeout, multi-render resource management) touch rendering output; verify with
  `mask visual-regression` and, if blend output changes visibly, a PNG read
  confirming the corrected blend appears as expected, not just "test passes."
- **CI**: before/after ignored counts read from the Tests workflow job log
  (`gh run view --job <id> --log | grep 'test result:'`), not estimated.

## Success Metrics

- [ ] Every `#[ignore]` attribute and every rustdoc `ignore` fence in the
      workspace appears exactly once in
      `docs/planning/IGNORED_TEST_INVENTORY.md` with a category and disposition.
- [ ] Zero bare `#[ignore]` remain; `mask all-check` fails on a seeded one.
- [ ] Zero category-(d) entries remain unfixed and untracked (each is either
      fixed or has a named owner and a tracking reference).
- [ ] The Tests workflow's before (278) and after ignored counts are recorded,
      with the delta explained by the inventory's category breakdown.

## Risk Assessment

- **Medium**: the 278-vs-~200 reconciliation gap (Context) might reveal the
  static grep approach misses a systematic source of ignores (e.g. per-feature
  re-runs counting the same test twice). _Mitigation_: use the Tests workflow's
  actual job log as ground truth, not the static grep, for the final count;
  treat any unexplained residual as a finding to write up, not to paper over.
- **Medium**: some category-(d) fixes may be nontrivial GPU bugs (the blend-mode
  timeout and resource-management TODOs have sat ignored long enough that the
  underlying cause is unknown). _Mitigation_: AC4 allows tracking as an expected
  failure with a named owner instead of mandating an immediate fix for every
  one; split any fix large enough to need its own story.
- **Low**: the new allowed-prefix lint could be too strict for a legitimate
  future one-off ignore. _Mitigation_: keep the allowed-prefix set short but
  documented and trivially extensible in one place in the script.
- **Worth flagging**: the three `gpu_statistics_integration_tests.rs` ignores
  cite "GUP-149" for a multi-workgroup limitation, but GUP-149 is used in
  `INDEX.md` for two unrelated, already-complete stories. This looks like a
  stale or incorrect reference — check whether the limitation was already fixed
  under a different story before assuming it still applies; if so these tests
  may simply be un-ignorable today (category d → fixed, not tracked).

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Rendered output verified by eye (PNG read) for any category-(d) fix that
      changes visible rendering.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
