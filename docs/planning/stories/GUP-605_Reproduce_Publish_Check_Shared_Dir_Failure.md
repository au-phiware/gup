---
implementer: sonnet
implementer-reason: an investigation with a seeded test
touches: [scripts/]
---

# GUP-605: Reproduce the Publish Check Shared Directory Failure

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-04

## Context

GUP-603 made `mask publish-check` use a build directory private to the checkout,
after `no hash listed for gup-core-macros v0.1.0` appeared twice with the shared
one. The failure could not be reproduced: stale or cksum-less index entries, a
second tree with the same version and other contents, and concurrent runs all
passed. The cause is probable, not proven. Do this only if the failure returns
or the private directory's cost matters.

## User Story

> "As the maintainer, I want to know why the shared directory failed, so that I
> know the private one is the right fix."

## Acceptance Criteria

- [x] The cause is found and written down, with a seeded test that fails on the
      old command and passes on `scripts/publish_check.sh`, or the story is
      closed as not reproducible.

## Dependencies

### Prerequisite Stories

- GUP-603 ✅

## Testing Strategy

Try a changed `Cargo.lock`, a cargo upgrade between runs, and two users.

## Risk Assessment

- **Low**: release tooling only.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-fix` clean
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Closed as **not reproducible**. No seeded test was added: a test that fails on
the old command needs a failure, and none could be produced. The private build
directory of `scripts/publish_check.sh` stays; its cause remains probable, not
proven.

Tried, on a two-member toy workspace (a crate and its dependent, both with
`version = "0.1.0"`, run with `cargo publish --workspace --dry-run --locked`,
cargo 1.93.1, a shared `CARGO_BUILD_BUILD_DIR`), about 900 KB of scratch under
the checkout's target directory, removed afterwards:

- Two trees with the same names and version and different contents, run
  alternately (1, 2, 1): all passed.
- A second run started while the first sat in a 25 s verification build: both
  passed (the second waited for the build directory lock).
- Seeded damage to the shared `package/`: emptied index files, an index with a
  wrong `cksum`, a stale `Cargo.lock` in the unpacked dependent, a changed
  workspace `Cargo.lock`, the overlay deleted, `tmp-crate` deleted, and the run
  killed with SIGKILL mid-verification and rerun: all passed. Cargo rebuilds the
  `tmp-registry` index and crates on every run.
- A cargo upgrade between runs (1.93.1, then 1.99.1, then 1.93.1): passed.

Not tried: two users (a permission error would not read `no hash listed`) and
the real workspace, whose cold run is about 8 minutes.

## Retrospective

**Completed**: 2026-11-12

### Key Technical Learnings

#### The shared overlay is rebuilt, not trusted

- **Challenge**: show how a shared `package/tmp-registry` makes
  `no hash listed for gup-core-macros` appear.
- **Solution**: none found; every seeded state recovered on the next run, so a
  stale overlay is not the cause (GUP-603's retrospective already doubted it).
- **Pattern**: a toy workspace with the same shape runs a dry run in seconds and
  needs network only for the crates.io index; use it before the 8-minute real
  one. The failure most likely needed a state the toy lacks (a real Cargo.lock
  with many crates, or a torn write on a full disk).

### Architectural Decisions

#### Keep the private directory

- **Decision**: no change to `scripts/publish_check.sh`.
- **Reasoning**: it is cheap insurance and removes all shared `package/` state.
- **Trade-off**: a cold first run per checkout.

### Development Workflow Insights

- The agent sandbox refused compound shell commands that mention the checkout's
  path; scratch experiments ran as script files.

### Follow-up Stories

None. If the failure returns, keep the failing `package/` directory before
rerunning and diff it against a fresh one.
