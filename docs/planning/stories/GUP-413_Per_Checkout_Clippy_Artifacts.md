# GUP-413: Per-Checkout Clippy Artifacts for Concurrent Agents

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: 💡 New
**Created**: 2026-10-10

## Context

[GUP-411](GUP-411_Isolate_Checkouts_Sharing_A_Target_Dir.md) made checkouts that
share a build directory use their own workspace-member artifacts, through a
rustc workspace wrapper whose path differs per checkout. `cargo clippy` replaces
that wrapper with clippy-driver, whose path is the same in every checkout, so
clippy's member artifacts are still shared. GUP-411 made that correct rather
than separate: clippy-driver records `CLIPPY_CONF_DIR`, which
`.cargo/config.toml` sets per checkout, so a checkout re-lints when another
checkout linted last.

Correct, but slow when agents work concurrently. GUP-411 measured
`mask all-check` at 26 s warm and 85–93 s right after another checkout linted;
three concurrent runs took 182, 219 and 256 s, serialized by the build
directory's lock, each re-linting. The pre-commit hook's snapshots of different
worktrees share clippy artifacts in the same way (GUP-409's `incremental` flip
separates a snapshot from its own checkout only), so every agent's commit
re-lints the touched crates and their dependents, usually including `gup`.

The one profile setting that takes arbitrarily many values, and so could give
each checkout its own clippy artifacts, is `codegen-units`. It is in cargo's
artifact hash, and does nothing to `check` or `clippy` builds. A per-checkout
value for members only needs:

- `CARGO_PROFILE_DEV_CODEGEN_UNITS=<n>` and
  `CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_UNITS=<n>` (member proc-macros such
  as gup-macros and build scripts use `build-override`; without it a re-linted
  gup-macros makes every dependent re-lint), from `scripts/cargo_env.sh`;
- a tracked `[profile.dev.package."*"] codegen-units = …` pin, so dependencies
  keep one hash (`package."*"` outranks both the profile and `build-override`
  for non-members). Pinning changes every dependency's hash once.

It leaves `release` alone (benchmarks across checkouts must use the same
codegen), and it depends on each agent running `cargo_env.sh`: one that does not
falls back to GUP-411's re-lint, still correct.

## User Story

> "As an orchestrator running several agents at once, I want each checkout's
> clippy results to stay warm while the others lint, so that the hook and
> `mask all-check` do not re-lint the whole workspace after every switch."

## Acceptance Criteria

- [ ] Decide, with measurements, whether per-checkout clippy artifacts are worth
      their disk: per-checkout clippy state (rmeta and incremental sessions) for
      `mask all-check`, against the re-lint time GUP-411 measured. Record the
      decision; if no, close this story with the numbers.
- [ ] If yes: `scripts/cargo_env.sh` exports the codegen-units values derived
      from the checkout (a hash of its path), dependencies are pinned, and the
      hook's snapshot gets its own value (which could replace GUP-409's
      `incremental` flip).
- [ ] `scripts/test_shared_build_dir.sh` gains a case: with the environment, B's
      clippy after A's is `Fresh` for B's own unchanged artifacts and not
      re-linted; without it, the GUP-411 behaviour holds.
- [ ] Three concurrent `mask all-check` runs measured again, time and disk.

## Technical Tasks

- [ ] Prototype in three worktrees as GUP-411 did (`/tmp/g411`-style), with a
      fresh build directory.
- [ ] Check the `package."*"` pin against the hook's snapshot config, which also
      writes `[profile.dev.package."*"]` (config tables merge key by key).
- [ ] Update `CLAUDE.md`'s "Sharing build output between checkouts".

## Dependencies

### Prerequisite Stories

- GUP-411 ✅: shared build directory, per-checkout member artifacts, clippy
  re-lint on switch.

### Enables Stories

(none known)

## Testing Strategy

- **Script test**: `test_shared_build_dir.sh` case above.
- **Measurement**: one and three concurrent worktrees, warm and after a switch.

## Success Metrics

- [ ] Warm `mask all-check` after another checkout linted is within about 10 s
      of the no-switch time.
- [ ] Three concurrent worktrees still fit in /tmp (22 GB) with room for test
      builds.

## Risk Assessment

- **Medium**: each checkout's clippy incremental state may cost 1–2 GB, which
  multiplies with agents. Mitigation: the decision AC; measure first.
- **Low**: a pinned dependency `codegen-units` changes build behaviour for
  dependency build scripts and proc-macros (their `build-override` default is
  256). Mitigation: pin to the value they effectively use, and verify with
  `cargo build -v`.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Lint and format clean: `mask all-check`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
