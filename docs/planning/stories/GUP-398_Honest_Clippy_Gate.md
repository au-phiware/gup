# GUP-398: Honest Clippy Gate

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: 📋 Planned
**Created**: 2026-10-05

## Context

`mask all-check` (the pre-commit hook) and `mask all-fix` lint with:

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

The gate also covers only the root package. `gup-macros`,
`crates/gup-visual-regression`, `crates/gup-culling-lod` (GUP-390), and the
coming `crates/gup-core` (RFC-001, GUP-395) are never linted by the hook.
RFC-001 builds the new core in `crates/gup-core`; without a real gate it will
pick up the same lint debt the old path has.

## User Story

> "As a maintainer, I want the lint gate to fail when clippy finds a problem, so
> that 'clippy -D warnings passed' in a commit or story means what it says,
> especially for the new `gup-core` crate."

## Acceptance Criteria

### AC1: The gate fails on lints

- [ ] `mask all-check` runs clippy without `--fix` (e.g.
      `cargo clippy --workspace --all-targets --all-features -- -D warnings`),
      so any lint fails the hook. `mask all-fix` may keep a `--fix` pass, but
      must follow it with the strict run.
- [ ] The strict run covers every workspace member, including
      `crates/gup-culling-lod` and, once it exists, `crates/gup-core`.

### AC2: Existing debt is fixed or explicitly allowed

- [ ] Each existing lint is either fixed or allowed at the narrowest scope
      (item-level `#[allow(clippy::...)]` with a one-line reason). No crate-wide
      blanket allows for `gup-core` or the support crates.
- [ ] For the frozen old path (RFC-001: `src/selection.rs`, `src/mark/`,
      `src/chart_builder*`, ...), allowing rather than fixing is acceptable, as
      S14 deletes that code. Record the count of allows added there.
- [ ] Real bugs the lints point at (e.g. "this operation will always return
      zero") are fixed, not allowed, or split into a story if large.

### AC3: Documentation matches behaviour

- [ ] `maskfile.md` `all-check`/`all-fix` comments describe the strict run.
- [ ] The story-worker agent prompt does not claim a stronger gate than the one
      that exists.

## Technical Tasks

- [ ] Run
      `cargo clippy --workspace --all-targets --all-features --keep-going     -- -D warnings`
      and list every failing lint per target. After fixing the lib, re-run:
      targets that depend on the lib are only linted once it passes.
- [ ] Fix or allow per AC2, one commit per area (lib, tests, examples, benches,
      support crates).
- [ ] Switch `all-check` to the strict command; keep `--fix` only in `all-fix`.
- [ ] Confirm that a deliberately introduced lint (e.g. an unused variable)
      makes `mask all-check` fail, then remove it.

## Dependencies

### Prerequisite Stories

- None. Best done before RFC-001 S1 adds most of `crates/gup-core`, so the new
  crate starts under a real gate.

### Enables Stories

- RFC-001 S1–S14 (GUP-395 onward): a lint gate the new core can rely on.

## Testing Strategy

- Deliberate-failure check: an injected lint must fail `mask all-check`.
- `cargo test -- --test-threads=1`, `mask visual-regression` and
  `mask smoke-examples` stay green after the lint fixes (most fixes are
  mechanical, but some touch logic).

## Success Metrics

- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
      exits 0 on `main`.
- [ ] The hook runs that command (or a stricter one).

## Risk Assessment

- **Medium**: some lint fixes change logic (for example "always return zero" may
  hide a real bug, whose fix changes output). Mitigation: the visual-regression
  and smoke suites; split behaviour changes into their own commits.
- **Low**: the strict run adds time to the hook. A clean check of the root
  package takes seconds once warm; linting the extra workspace members adds
  roughly as much as `cargo check -p` for each.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: the new strict `mask all-check` passes.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
