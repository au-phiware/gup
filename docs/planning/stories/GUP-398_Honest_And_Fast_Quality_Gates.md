# GUP-398: Honest and Fast Quality Gates

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: 🚧 In
Progress **Created**: 2026-10-05 **Revised**: 2026-10-05 (scope widened from
"Honest Clippy Gate" to cover every hook/CI check's failure behaviour and the
hook's speed, per story-scribe request — no implementation has started, so this
is an in-place rewrite, not a new story)

## Context

`mask all-check` (the pre-commit hook, installed by `flake.nix`'s `shellHook` as
a symlink to a script that runs exactly `mask all-check`) and `mask all-fix`
lint with:

```bash
cargo clippy --allow-no-vcs --fix --all-targets --all-features -- -D warnings
```

In `--fix` mode `-D warnings` does not fail the command. Clippy applies the
machine-applicable fixes and prints every other lint as a **warning**, then
exits 0. Verified during GUP-390 (2026-10-05):

- The command above exits 0 and prints 99 warnings across all targets (for
  example 14× "this operation will always return zero", 12× "field `x` is never
  read", 10× "too many arguments", 8× `type_complexity`).
- The same command without `--fix` fails: 43 errors in the `gup` lib and 63 in
  its unit tests. Because the lib fails, Cargo never lints the examples,
  integration tests or benches, so the real count is higher.

Agents and retros believe the hook enforces `clippy -D warnings` on every
target. GUP-388's Definition-of-Done evidence says so explicitly. It does not:
any lint without an automatic fix passes the gate.

The gate also covers only the root package and `gup-core`'s own strict,
separately-run clippy line in `all-check` (added when `crates/gup-core` landed
in GUP-395). `gup-macros`, `crates/gup-visual-regression` and
`crates/gup-culling-lod` (GUP-390) are still linted only via the unreliable
`--fix` command, or not linted by the hook at all.

**This is one instance of a broader problem: a gate can look like it enforces
something and not actually enforce it, and nobody finds out until long after the
gap was introduced.** The strategic review's root-cause table records two more
instances found and fixed on 2026-10-04, in the course of the same investigation
that found the clippy gap:

- A whitespace check that used a shell glob pattern against `*.rs` which did not
  recurse into subdirectories, so trailing whitespace in nested files passed
  silently. Fixed by switching to `git grep`'s pathspec matching
  (`maskfile.md`'s current `fmt-check`/`all-check` whitespace line), which does
  recurse — this fix already landed and is not this story's job to redo, but it
  is the clearest prior evidence that "the check exists" and "the check works"
  are different claims that need separately checking.
- `clippy --fix` itself, as above.

Both gaps existed for an unknown period before anyone noticed, because nobody
had ever deliberately introduced a violation and watched the gate catch it. **A
gate that has never been proven to fail is not a gate — it's a step that runs.**
This story generalises the fix: every check the hook runs, and every CI job,
gets a deliberately seeded violation and a recorded observation of the gate
actually rejecting it, not just an assumption that it does because it looks like
it should.

**The hook is also slow, and slowness drives exactly the bypass behaviour the
review is trying to stop.** `mask all-check` runs the full workspace build,
test-adjacent checks (`mask check`: four `cargo check` invocations), two
`cargo fmt --check` runs, two clippy runs, `nixfmt`/`statix`, `prettier`/`mdl`,
and mark validation, on every commit, regardless of what changed. GUP-395's
retrospective measured it at "about 3–5 minutes warm." `--no-verify` appears in
63 retrospectives, and the review's root-cause table names the slow,
all-or-nothing hook as a contributor: agents facing a multi-minute full-build
check on a one-line documentation fix, or a change to a single leaf crate, have
a standing incentive to skip it rather than wait — exactly the bypass this
project's process rules now forbid outright. A hook that is fast when the change
is small, and thorough when it needs to be, removes the incentive without
weakening the guarantee: **CI stays the unscoped, always-full gate** regardless
of what the local hook decided to skip, so a bypassed or under-scoped local
check can never merge silently.

## User Story

> "As a maintainer, I want the lint gate to fail when clippy finds a problem,
> every check and CI job to be provably able to fail, and the local hook to be
> fast for small or doc-only changes without weakening what actually merges, so
> that 'the hook passed' means what it says and nobody is tempted to
> `--no-verify` past it."

## Acceptance Criteria

### AC1: The clippy gate fails on lints

- [ ] `mask all-check` runs clippy without `--fix` (e.g.
      `cargo clippy --workspace --all-targets --all-features -- -D warnings`),
      so any lint fails the hook. `mask all-fix` may keep a `--fix` pass, but
      must follow it with the strict run.
- [ ] The strict run covers every workspace member, including `gup-macros`,
      `crates/gup-visual-regression`, `crates/gup-culling-lod` and
      `crates/gup-core` — replacing the separate ad hoc
      `cargo clippy -p     gup-core` line added in GUP-395 with one
      workspace-wide strict invocation, now that this story makes the whole
      workspace strict.

### AC2: Existing debt is fixed or explicitly allowed

- [ ] Each existing lint is either fixed or allowed at the narrowest scope
      (item-level `#[allow(clippy::...)]` with a one-line reason). No crate-wide
      blanket allows for `gup-core` or the support crates.
- [ ] For the frozen old path (RFC-001: `src/selection.rs`, `src/mark/`,
      `src/chart_builder*`, `src/context.rs`, `src/render.rs`, …), allowing
      rather than fixing is acceptable, since S14 deletes that code. Record the
      count of allows added there.
- [ ] Real bugs the lints point at (e.g. "this operation will always return
      zero") are fixed, not allowed, or split into a story if large.
- [ ] `gup-core` and every other non-frozen crate stay strict: no new allow is
      added there without the same narrow-scope, one-line-reason bar as
      everywhere else, and blanket allows remain forbidden there regardless of
      how large the lint count turns out to be.

### AC3: Every hook check and CI job is proven to fail when it should

- [ ] For every check `mask all-check` runs — the untracked-whitespace grep,
      `mask check`'s four `cargo check` invocations, both `cargo fmt --check`
      runs, the workspace clippy run (AC1), `nixfmt --check`, `statix check`,
      `prettier --check`, `mdl`, and `mask validate-marks` — a deliberate
      violation is introduced (a trailing-whitespace line in a nested `.rs`
      file, an unformatted `.rs`/`.nix`/`.md` file, a clippy-triggering snippet,
      a Nix anti-pattern, a markdown lint violation, an invalid mark
      registration), the check is run and observed to fail, and the violation is
      reverted. The check name, the seeded violation and the observed failure
      are recorded in a table in the retrospective.
- [ ] For every job across `.github/workflows/*.yml` (`visual-regression`;
      `dogfood`; `gallery`'s `build-gallery` and `deploy`; `android-ci`'s
      `build-rust`, `example-apk` and `platform-isolation`; `ios-ci`'s
      `build-rust`, `swift-package` and `platform-isolation`; `performance`'s
      `perf_check`, `performance`, `cross_platform_comparison`,
      `axis_performance`, `wasm_axis_performance`, `pattern_benchmarks_pr` and
      `benchmark`; `wasm`'s `wasm_check`) the same treatment is attempted: a
      seeded violation local to that job's actual check (a failing assertion, a
      broken golden image, a regression beyond the performance threshold, a
      build break), run (via `act`, a scratch branch and CI run, or documented
      manual reasoning about the job's script if neither is practical) and the
      observed failure recorded. Jobs that genuinely cannot be seeded without
      hardware or SDKs unavailable in this environment (e.g.
      `android-ci`/`ios-ci` jobs needing a device or paid Apple tooling) get an
      explicit, reasoned note explaining what would need to be true to seed
      them, rather than being silently skipped from the table.
- [ ] Any additional gate found to be failing open during this audit (beyond the
      two already known — the pre-2026-10-04 whitespace glob and `clippy --fix`)
      is fixed in this story, not deferred, unless it is large enough to need
      its own story — in which case it is split out and named in the Follow-up
      Stories section of this story's retrospective.

### AC4: The hook is proportional to the change

- [ ] A docs-only commit (every staged file matches a documentation pattern —
      `*.md`, `docs/**`, `LICENSE*`, or similar; the exact pattern list is
      recorded) skips every Rust-specific check (`mask check`, both clippy runs,
      both `cargo fmt --check` runs) in the local pre-commit hook, while still
      running the markdown/prettier checks that apply to the changed files.
- [ ] A commit that touches only one or a few workspace crates runs Rust checks
      scoped to those crates (e.g.
      `cargo check/clippy/fmt --check     -p <crate>` for each touched crate)
      instead of the full workspace, **except** when a workspace-level file
      changes (root `Cargo.toml`, `Cargo.lock`, `flake.nix`, or any file outside
      a single crate's directory), which still triggers the full, unscoped check
      — the scoping logic must be conservative: when in doubt about whether a
      change could affect another crate, it runs the full check.
- [ ] The proportionality logic lives in one place (e.g. a new
      `mask     pre-commit` task that inspects `git diff --cached --name-only`,
      which `flake.nix`'s hook script calls instead of `mask all-check`
      directly), so it is testable on its own rather than embedded in the hook's
      Nix-store symlink.
- [ ] **CI is unaffected**: every `.github/workflows/*.yml` job keeps running
      its current full, unscoped set of checks regardless of what changed in the
      triggering commit or PR. The proportional/skip logic is local-hook- only
      and is not imported into any CI workflow — a deliberately seeded violation
      in an untouched-looking crate, pushed in a commit that the local hook
      would have scoped away from, must still fail in CI. A test or documented
      manual check proves this (e.g. push a branch with a docs-only diff that
      also contains a hidden clippy violation in an unrelated crate, and confirm
      CI still catches it even though the local hook's scoping would have
      skipped that crate).

### AC5: Documentation matches behaviour

- [ ] `maskfile.md`'s `all-check`/`all-fix`/new `pre-commit` task comments
      describe the strict run and the proportional/scoping behaviour accurately.
- [ ] The story-worker agent prompt does not claim a stronger or faster gate
      than the one that exists, and reflects that CI is the full,
      always-unscoped gate while the local hook may legitimately skip or scope
      checks for small changes.

## Technical Tasks

- [ ] Run
      `cargo clippy --workspace --all-targets --all-features --keep-going     -- -D warnings`
      and list every failing lint per target. After fixing the lib, re-run:
      targets that depend on the lib are only linted once it passes.
- [ ] Fix or allow per AC2, one commit per area (lib, tests, examples, benches,
      support crates).
- [ ] Switch `all-check` to the strict, workspace-wide clippy command, replacing
      both the `--fix` line and the separate `gup-core`-only strict line; keep
      `--fix` only in `all-fix`, followed by the strict run.
- [ ] Confirm that a deliberately introduced lint (e.g. an unused variable)
      makes `mask all-check` fail, then remove it.
- [ ] Work through the AC3 matrix: for each `mask all-check` check and each CI
      job, seed a violation, observe and record the result, then revert the
      violation. Build the table incrementally in a scratch file and move it
      into the retrospective at the end; do not hold the whole matrix in working
      memory across a long session.
- [ ] Fix any newly discovered failing-open gate found during the AC3 audit (or
      split it into its own story if large, per AC3's last bullet).
- [ ] Design the docs-only and per-crate scoping rules for AC4: decide the
      documentation-file pattern list, decide which non-crate-local files force
      a full check, and implement a `mask pre-commit` (or equivalent) task
      containing the logic.
- [ ] Point `flake.nix`'s `pre-commit` script at the new task instead of
      `mask all-check` directly.
- [ ] Write the AC4 proof that CI still catches what the local hook's scoping
      would have skipped.
- [ ] Update `maskfile.md` comments and the story-worker agent prompt per AC5.

## Dependencies

### Prerequisite Stories

- None. Best done before RFC-001 S1 adds most of `crates/gup-core`'s production
  wiring into the old path, so the new core and its caller both start under a
  real, fast gate.

### Enables Stories

- RFC-001 S1–S14 (GUP-395 onward): a lint gate the new core can rely on, and a
  hook fast enough that touching only `crates/gup-core` or `crates/gup-text`
  doesn't force a full-workspace rebuild on every commit.

## Testing Strategy

- **Deliberate-failure checks**: the AC3 matrix (every hook check, every CI job)
  and the AC1 injected-lint check are themselves the primary test strategy for
  this story — proving gates fail is the point.
- `cargo test -- --test-threads=1`, `mask visual-regression` and
  `mask smoke-examples` stay green after the lint fixes (most fixes are
  mechanical, but some touch logic).
- A manual or scripted proof that CI's full checks still run on a commit the
  local hook's scoping would have narrowed (AC4's last bullet).

## Success Metrics

- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
      exits 0 on `main`.
- [ ] The hook runs that command (or a stricter one) for the files it decides
      need it.
- [ ] Every check in `mask all-check` and every CI job has a recorded
      seeded-violation proof (or a reasoned, explicit exception) in this story's
      retrospective.
- [ ] A docs-only commit's local pre-commit hook completes in a small fraction
      of the current 3–5 minute warm time; the measured before/after time is
      recorded.
- [ ] CI's checks are demonstrated to be unaffected by the local hook's scoping.

## Risk Assessment

- **Medium**: some lint fixes change logic (for example "always return zero" may
  hide a real bug, whose fix changes output). Mitigation: the visual-regression
  and smoke suites; split behaviour changes into their own commits.
- **Medium**: the AC3 matrix (9 local checks + 17 CI jobs) is large, and some CI
  jobs (mobile builds, performance benchmarks requiring specific hardware) may
  be impractical to seed-and-observe without `act` or a scratch CI run, or may
  require hardware this environment doesn't have (a physical Android/iOS device,
  a GPU for performance baselines). _Mitigation_: AC3 explicitly allows a
  reasoned, documented exception instead of an actual seeded run for jobs that
  genuinely can't be exercised here, as long as the exception explains what
  would need to be true to seed it; if the full matrix proves too large for one
  sitting, split the CI-job half into its own follow-up story rather than
  rushing or skipping entries silently.
- **Medium**: getting the per-crate scoping rule (AC4) wrong in the unsafe
  direction (skipping a check that should have run) would quietly recreate the
  exact failure mode this story is fixing — a gate that looks like it checks
  something but doesn't. _Mitigation_: AC4 requires the scoping to be
  conservative (any workspace-level file change forces a full check) and
  requires an explicit proof that CI — which never scopes — still catches what
  the local hook would have missed, so a wrong scoping decision is a slower
  local loop, never a silent merge.
- **Low**: the strict run adds time to the hook for changes that do need the
  full check. A clean check of the root package takes seconds once warm; linting
  the extra workspace members adds roughly as much as `cargo check -p` for each
  — this is the cost AC4's scoping is designed to avoid paying on every commit,
  not to eliminate for genuinely cross-cutting changes.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: the new strict `mask all-check` passes.
- [ ] All examples compile: `cargo check --examples`.
- [ ] The AC3 seeded-violation matrix and the AC4 before/after hook timing are
      recorded in the retrospective.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
