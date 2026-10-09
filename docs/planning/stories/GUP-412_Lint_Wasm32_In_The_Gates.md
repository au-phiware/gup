# GUP-412: Lint the wasm32 Target in the Gates

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: 🚧 In
Progress **Created**: 2026-10-10

## Context

Every clippy gate (`mask all-check`, the pre-commit hook, CI's Lint workflow)
lints the host target only. Code behind `#[cfg(target_arch = "wasm32")]` is
compiled by the Wasm workflow (`cargo build --target wasm32-unknown-unknown`)
but never linted, and wasm32 changes what some lints see: on wasm32 wgpu's types
are not `Send`/`Sync`.

[GUP-410](GUP-410_Surface_WebGPU_Errors_From_Gup_Core.md) found one such
finding, and GUP-411 confirmed it on 2026-10-10:

```text
$ cargo clippy -p gup-core --target wasm32-unknown-unknown -- -D warnings
error: usage of an `Arc` that is not `Send` and `Sync`
   --> crates/gup-core/src/selection.rs:251:19
error: could not compile `gup-core` (lib) due to 1 previous error
```

`cargo clippy --workspace --target wasm32-unknown-unknown` stops at the same
error, because every other member depends on gup-core, so how many findings lie
beyond it is unknown. gup-core is the crate whose browser path (RFC-001 S8) is
next, so its wasm32 code will grow.

## User Story

> "As a contributor changing code that runs in the browser, I want the gates to
> lint the wasm32 target, so that a lint that only fires there is caught before
> it reaches main."

## Acceptance Criteria

- [ ] `selection.rs:251` is fixed (or allowed with a reason, if the `Arc` is
      deliberate on wasm32, where there are no threads), and every further
      wasm32 finding in the workspace is fixed or allowed with a reason.
- [ ] `mask all-check` runs `cargo clippy --target wasm32-unknown-unknown` with
      `-D warnings` over the members that build for wasm32 (at least gup-core,
      gup-text and gup; decide `--all-targets` and features per crate and record
      why).
- [ ] CI's Lint workflow gets the same check (it runs `mask all-check`).
- [ ] The pre-commit hook's scoped mode adds the wasm32 pass for touched crates
      that build for wasm32, or the story records why it is left to full mode
      and CI (time budget, GUP-398).
- [ ] Seeded proof (GUP-398's convention): a wasm32-only lint (e.g. the current
      `Arc` finding, reintroduced) fails `mask all-check` and passes a host-only
      clippy run.

## Technical Tasks

- [ ] List wasm32 findings crate by crate
      (`cargo clippy -p <crate> --target wasm32-unknown-unknown`), fixing
      gup-core first.
- [ ] Add the pass to `maskfile.md`'s `all-check` (and `all-fix`/`lint` for
      consistency) and, if cheap enough, to `scripts/pre_commit.sh`.
- [ ] Measure the added time, warm and cold, and the disk for wasm32 check
      artifacts in the shared build directory (GUP-411).

## Dependencies

### Prerequisite Stories

- GUP-398 ✅: the gate conventions and seeded-proof rule.
- GUP-410 ✅: found the finding.

### Enables Stories

- RFC-001 S8 (gup-core's wasm entry point) lands on a linted target.

## Testing Strategy

- **Seeded proof**: one wasm32-only lint, observed failing the new pass and
  passing the host pass.
- **Gate timing**: warm `mask all-check` before and after.

## Success Metrics

- [ ] Zero wasm32 clippy findings in the workspace, enforced by the gates.
- [ ] Warm `mask all-check` grows by no more than about 20 s.

## Risk Assessment

- **Medium**: findings beyond gup-core may be many in the root crate's wasm
  code. Mitigation: start with gup-core and gup-text, and add the root crate in
  a follow-up if it is large.
- **Low**: a wasm32 check build adds disk. Mitigation: check mode only (rmeta).

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Lint and format clean: `mask all-check`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
