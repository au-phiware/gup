# GUP-427: Bound Build-Directory Disk Growth Per Commit

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: ✅ Complete
(2026-10-16) **Created**: 2026-10-11

## Context

[GUP-411](GUP-411_Isolate_Checkouts_Sharing_A_Target_Dir.md) and
[GUP-409](GUP-409_Close_Gate_Gaps.md) made a shared build directory safe:
`.cargo/config.toml`'s `rustc-workspace-wrapper`
(`scripts/rustc_workspace_wrapper.sh`) gives every checkout its own
workspace-member artifacts inside `CARGO_BUILD_BUILD_DIR`, keyed by the
checkout's path, while dependencies stay shared; the pre-commit hook's snapshot
(`scripts/pre_commit.sh`) additionally gives its own tree the opposite
`[profile.dev]`/`[profile.dev.package."*"]` `incremental` setting from the real
checkout, so the snapshot's member artifacts do not collide with the checkout's.
Both mechanisms are correct — but neither bounds how much they leave behind.
[GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md)'s
retrospective recorded the cost directly: "the shared build directory's debug
incremental caches grew 2–4 GB per round. Each pre-commit hook run with Rust
changes lints the root crate's examples and leaves about 3.8 GB of them.
Toggling `CARGO_INCREMENTAL` rehashes every test binary (about 2.2 GB of
duplicates). Free space dipped to 3.8 GB once; pruning idle incremental caches
and the superseded test binaries (this checkout's only) brought it back to 8.2
GB. No `cargo clean`." That pruning was a one-off, by-hand intervention during
GUP-419's own work, not a mechanism anyone else benefits from.

GUP-420 measured it per commit: seven Rust commits' hook runs used 2,483 MB (the
full `mask all-check`), then 2,113, 2,112, 2,175, 2,086, 2,192 and 2,071 MB,
almost all of it debug incremental caches left by clippy over the root crate.
Docs-only commits used nothing. Free space fell below 5 GB three times.

On 2026-10-11 the owner freed space: the `~/.cache` pool now has about 160 GB
free, and `/tmp` (the SSD pool that also holds the nix store) has about 75 GB.
Disk is no longer a hard limit on builds, but growth is still unbounded, so this
story still matters. Measured on 2026-10-11, cargo and rustc put only small,
short-lived files in `TMPDIR` (`rustc*/symbols.o`, rlib `lib.rmeta` staging, the
C linker driver's `cc*.res`). The past `/tmp` outages came from an explicit
`CARGO_TARGET_DIR=/tmp/...`. Also in scope: about 32 tests write to fixed-name
directories under `std::env::temp_dir()` (for example `gup_pdf_test`,
`gup_html_test`). They are small, but two checkouts running the same test at
once share the path, so give them unique per-run directories.

This is not a one-time cost: the project's history shows disk exhaustion has
stalled work "several times (`/tmp` filled twice, the ZFS pool hit 0 once)" per
this story's brief. Each Rust-touching commit's pre-commit hook run flips
`incremental` between the checkout and its snapshot, and that flip is exactly
what rehashes every incremental session and every test binary into a second copy
GUP-419 measured at ~2.2 GB. On a machine with a build directory living on a
filesystem with roughly 19 GB free (GUP-419's figure), a handful of commits
without any pruning can exhaust it, and unlike GUP-413's lint-speed concern this
is a hard failure (builds and tests stop working, not merely slow down), so it
deserves its own fix rather than waiting on GUP-413's decision.

[GUP-413](GUP-413_Per_Checkout_Clippy_Artifacts.md) (💡 New, undecided)
addresses a related but distinct problem: concurrent agents' `cargo clippy`
re-linting each other's work because clippy-driver's artifacts are shared across
checkouts (a **speed** cost, measured in seconds per switch), proposing
per-checkout `codegen-units` values to give each checkout its own clippy
artifacts. That would, if adopted, add yet another axis of per-checkout artifact
multiplication — exactly the kind of growth this story needs to bound. **This
story does not supersede GUP-413**: GUP-413 is still an open, measurement-gated
decision about lint speed; this story is about disk-space growth regardless of
whether GUP-413 is adopted. If GUP-413 is adopted, this story's bound and prune
mechanism must account for its new per-checkout artifacts rather than treat them
as the superseded state it prunes away.

## User Story

> "As an orchestrator running several agents against a shared build directory, I
> want each commit's pre-commit hook run to leave behind a bounded amount of
> build-directory growth, with superseded artifacts and idle incremental caches
> pruned automatically, so that disk exhaustion stops being a recurring failure
> mode that needs a human to notice and intervene."

## Acceptance Criteria

### AC1: Measured source of growth

- [x] A reproducible measurement (building on
      `scripts/test_shared_build_dir.sh`'s pattern — a throwaway repository, or
      this repository's own build directory before/after a seeded commit)
      attributes the GB-per-commit growth GUP-419 observed to specific causes:
      the `incremental` flip between checkout and snapshot
      (`scripts/pre_commit.sh`'s `[profile.dev]`/`[profile.dev.package."*"]`
      toggle), per-checkout member artifacts (`.cargo/config.toml`'s
      `rustc-workspace-wrapper`, `scripts/cargo_env.sh`'s per-checkout
      `CARGO_TARGET_DIR`), and anything else found (test binaries, example
      binaries, doc output).
- [x] The measurement records, per cause, bytes added by one representative
      Rust-touching commit (a small source change triggering the hook's scoped
      Rust checks) and whether that cost is one-time (first commit after a cold
      build directory) or recurring (every commit).

### AC2: Bounded growth per commit

- [x] A fix (or combination) keeps the _net_ build-directory growth per
      Rust-touching commit under a documented ceiling (propose a number grounded
      in AC1's measurement and the available disk headroom described above — for
      example low hundreds of MB, not multiple GB) once the prune mechanism
      (AC3) has run, proven by the seeded check (AC4).
- [x] The fix does not use `cargo clean` on the shared build directory (the
      project's standing rule, per `CLAUDE.md`'s "Sharing build output between
      checkouts": "Never `cargo clean` a shared build directory; delete your own
      `CARGO_TARGET_DIR` instead") — any pruning targets specific stale
      artifacts or idle incremental sessions, identified by path and age/
      supersession, not a blanket clean.

### AC3: Automatic pruning

- [x] A pruning step (run by the pre-commit hook itself, a `mask` task, or a
      cron-style periodic job — document which and why) removes: incremental
      compilation sessions no longer reachable from any checkout's current HEAD
      (superseded artifacts), and incremental caches idle past a documented
      age/count threshold (for example, keep only the last N sessions per crate,
      or sessions untouched for M days).
- [x] The pruning step is safe under concurrency: it must not delete an artifact
      another checkout's in-flight build is using (reuse the shared build
      directory's existing locking, per `CLAUDE.md`'s "Concurrency: safe. Cargo
      locks the build directory while it compiles" note, or add equivalent
      locking around the prune itself).
- [x] The pruning step never touches another checkout's current (non-stale)
      artifacts — a test with two checkouts, one pruning while the other is
      warm, confirms the warm checkout's next build/lint is still `Fresh` for
      its own unchanged code.

### AC4: Seeded check that proves the bound

- [x] A new or extended script (alongside `scripts/test_shared_build_dir.sh`)
      seeds N representative commits (varying file sizes/crates touched) in a
      throwaway repository sharing a build directory, runs the pre-commit hook
      for each, measures the build directory's size after each commit and after
      the prune step runs, and asserts the net per-commit growth stays under
      AC2's ceiling after pruning.
- [x] The check is runnable locally (documented command) and is wired into the
      relevant CI gate (or explicitly deferred with a reason, if running it in
      CI is impractical — for example because it needs real disk I/O at a scale
      CI runners do not have).

## Technical Tasks

- [x] Instrument or manually measure `scripts/pre_commit.sh`'s snapshot path and
      `.cargo/config.toml`'s per-checkout artifacts to attribute growth (AC1) —
      likely `du -sh` on `$CARGO_BUILD_BUILD_DIR`'s subdirectories before and
      after a seeded commit, broken down by `incremental/`, `checkouts/*/`, and
      dependency artifact directories.
- [x] Decide the pruning mechanism and its trigger (hook-integrated vs separate
      `mask` task vs periodic); implement it.
- [x] Add the supersession/idle-age bookkeeping needed to prune safely (for
      example, a small manifest or reliance on filesystem mtimes/cargo's own
      fingerprint files to decide what is stale).
- [x] Add locking around the prune step consistent with cargo's existing
      build-directory lock.
- [x] Extend or add a script implementing AC4's seeded check.
- [x] Update `CLAUDE.md`'s "Sharing build output between checkouts" section with
      the new bound, the prune mechanism, and the "never `cargo clean`" rule's
      continued standing.
- [x] Cross-check against GUP-413: if GUP-413 has landed (or lands first),
      confirm the prune mechanism accounts for its per-checkout `codegen-units`
      artifacts rather than treating them as stale; if GUP-413 has not landed,
      note in both stories' Dependencies sections that they touch the same
      scripts (`scripts/cargo_env.sh`, `.cargo/config.toml`) and should be
      sequenced or coordinated to avoid conflicting edits.

## Dependencies

### Prerequisite Stories

- GUP-411: Isolate Checkouts Sharing a Target Directory ✅ — the per-checkout
  member-artifact mechanism (`.cargo/config.toml`, `scripts/cargo_env.sh`) this
  story measures and bounds the growth of.
- GUP-409: Close Gate Gaps ✅ — the pre-commit hook's snapshot `incremental`
  flip (`scripts/pre_commit.sh`) this story's AC1 measurement attributes growth
  to.

### Related, Not Superseded

- GUP-413: Per-Checkout Clippy Artifacts for Concurrent Agents 💡 — a related
  but distinct concern (lint **speed** across concurrent checkouts, via
  per-checkout `codegen-units`), not superseded by this story. If GUP-413 is
  adopted, this story's prune mechanism must treat its new per-checkout clippy
  artifacts as live, not stale; see the Technical Tasks cross-check item.
  **Cross-checked (2026-10-11)**: GUP-413 has not landed. The prune is
  hash-agnostic and deletes a unit only when the checkout recorded as its owner
  is gone; clippy units have no recorded owner, so GUP-413's per-checkout clippy
  units would never be pruned as stale (safe), but nor would a removed
  checkout's. GUP-413's Dependencies now say so and what it must add (an owner
  record for its clippy units and a case in `scripts/test_prune_build_dir.sh`);
  both stories edit `scripts/cargo_env.sh` and the hook's snapshot config.

### Enables Stories

(none known — this is a standalone guardrail)

## Testing Strategy

- **Script test**: the seeded multi-commit check (AC4), extending
  `scripts/test_shared_build_dir.sh`'s pattern of a throwaway repository and two
  or more worktrees sharing a build directory.
- **Measurement**: `du`-based before/after accounting per commit and per prune
  run, recorded in the story's retrospective (machine, date, numbers, matching
  the convention GUP-417/`PERF_BUDGETS.md` already use for performance claims).

## Success Metrics

- [x] Net build-directory growth per Rust-touching commit, after pruning, stays
      under the documented ceiling across AC4's seeded multi-commit check.
- [x] No seeded concurrent build/lint is ever served a pruned-away or incorrect
      artifact (AC3's concurrency test passes).
- [x] `CLAUDE.md` documents the new bound and mechanism.

## Risk Assessment

- **High**: disk exhaustion is a hard-stop failure (not merely slow), and a
  pruning mechanism that is too aggressive could delete an artifact a concurrent
  build still needs, turning a disk problem into a correctness or flakiness
  problem. _Mitigation_: AC3's concurrency test is not optional; prefer a
  conservative staleness rule (prune only artifacts unreachable from any
  checkout's current HEAD, or idle well past any plausible single build's
  duration) over an aggressive one.
- **Medium**: the exact ceiling (AC2) is a judgement call without a clear
  "correct" number; too tight a ceiling could force pruning so often it adds its
  own overhead or correctness risk. _Mitigation_: ground the number in AC1's
  measurement and the project's observed disk headroom, document the reasoning,
  and treat it as revisable.
- **Low**: this story's scripts overlap with GUP-413's proposed edits to the
  same files (`scripts/cargo_env.sh`, `.cargo/config.toml`). _Mitigation_: the
  cross-check Technical Task, and flagging the overlap in both stories'
  Dependencies sections.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] Lint and format clean: `mask all-check`
- [x] The seeded multi-commit check (AC4) passes locally and, if wired into CI,
      there too
- [x] `CLAUDE.md` updated
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `scripts/prune_build_dir.sh` (new): under each profile's `.cargo-lock`
  (`flock -w`, skip when busy), deletes (1) what checkouts that no longer exist
  built: owned units in `deps/`, `examples/`, `build/` and their
  `.fingerprint/`, the checkout's incremental caches and its `checkouts/` target
  directory; (2) every incremental session but the newest per crate, and
  `-working` sessions over a day old; (3) crate caches idle 3 days
  (`GUP_PRUNE_IDLE_DAYS`); (4) least recently compiled caches over 8 GB per
  profile (`GUP_PRUNE_INCREMENTAL_MB`). `--dry-run`, `--forget CHECKOUT`.
- `scripts/rustc_workspace_wrapper.sh`: records each member unit's owner in
  `<profile>/.gup-owners/` and moves its incremental cache to
  `incremental/<checkout>-<hash>/` (no hash change, so nothing rebuilds).
  `scripts/cargo_env.sh` marks each target directory with its checkout.
- `scripts/pre_commit.sh`: prunes after Rust checks (10 s lock wait; a prune
  failure never fails the hook). `mask prune-build`; `mask clean` no longer runs
  `cargo clean` with a shared build directory (verified: it deleted the build
  directory too). `flake.nix` pins `jq` and `util-linux` (flock).
- `scripts/test_prune_build_dir.sh` (new, in `mask all-check`, so CI's Lint
  workflow): 7 seeded commits plus 4 short-lived worktrees through
  `pre_commit.sh --exec` snapshots; net growth per round 8–33 KB after the prune
  (ceiling 256 KB) against 13 MB unpruned; busy lock skipped; warm checkout and
  shared dependency `Fresh`; 15 s. `test_shared_build_dir.sh` no longer builds
  in `/tmp`.
- 32 test sites (13 unit, 19 integration) and 2 examples get per-process scratch
  names. Tests: gup lib 2638 passed, 9 affected integration targets 88 passed.

### Measurements (2026-10-11/16, own build directory, `du` after `sync`)

| Hook run (root-crate change)       | Before    | After  |
| ---------------------------------- | --------- | ------ |
| Cold, in place                     | 3,943 MB  | 3,941  |
| 2nd in place                       | +1,714 MB | +20 MB |
| 3rd in place                       | +20 MB    | +0 MB  |
| 1st in snapshot (one-time)         | +233 MB   | +233   |
| 2nd in snapshot                    | +0 MB     | +0 MB  |
| New checkout, first hook           | +504 MB   | —      |
| Worktree `cargo test -p gup` build | +21.8 GB  | +21.8  |
| That worktree removed, next prune  | +0 (kept) | −21.8  |

Causes: the recurring "2 GB per commit" GUP-420 saw was its own habit of
deleting `debug/incremental` before each commit, which the next hook then
rebuilt (1.8 GB); rustc otherwise bounds itself at two sessions per crate. The
incremental flip costs one extra set of clippy rmeta (233 MB, one-time). The
unbounded cost was per-checkout units of removed worktrees (21.8 GB per agent
worktree that ran the tests: deps 9.7 GB, examples 9.9 GB, incremental 2.2 GB),
which nothing reclaimed.

## Retrospective

**Completed**: 2026-10-16

### Key Technical Learnings

#### Measure on ZFS with apparent size

- **Challenge**: `du` on freshly written files reported ~50 KB for 13 MB of
  binaries; ZFS allocates at transaction commit.
- **Solution**: the seeded check uses `du --apparent-size`; real-scale
  measurements `sync` and wait 6 s.
- **Pattern**: never assert on allocated size of just-written files.

#### Incremental caches cannot be attributed after the fact

- **Challenge**: rustc names crate caches by stable crate id, and member `.d`
  files hold relative paths, so neither maps to a checkout.
- **Solution**: the workspace wrapper (already per checkout) rewrites
  `-C incremental=` into a per-checkout directory and stamps unit owners. Cargo
  never reads the incremental path, so hashes are unchanged.
- **Pattern**: ownership is cheap to record at build time, impossible later.

#### Simple options weighed

- Non-incremental hook clippy: the in-place hook's caches are the same units the
  developer's own `mask all-fix` builds, so it would add a set, not remove one.
- A stable `CARGO_INCREMENTAL` for the hook: the flip's cost is one-time (233
  MB); the test-binary rehash GUP-419 saw came from people toggling it.
- A size-capped LRU prune alone: bounds incremental only, not the 19.6 GB of
  dead worktrees' tests and examples. Kept as rule 4.

### Architectural Decisions

#### Prune in the hook, under cargo's lock

- **Decision**: run after every Rust hook run, `flock` on each profile's
  `.cargo-lock`, skip when busy.
- **Reasoning**: no daemon or cron; cargo holds that lock for every build, so no
  rustc runs while files go.
- **Trade-off**: a constantly busy directory defers pruning; at most 10 s added
  per commit.
- **Future**: GUP-413's per-checkout clippy units would need an owner record.

### Development Workflow Insights

- The session was interrupted for five days mid-measurement; the idle rule then
  pruned the stale A2 caches (513 crates) in the reclaim run, as designed.
- The prune's "freed" total counts hard-linked rmeta (shared with sessions)
  twice; the before/after `du` totals are the real numbers.

### Follow-up Stories

1. **GUP-428: Prune Superseded Shared Units** — dependency units left by
   `Cargo.lock` or toolchain changes, clippy units of deleted targets, and
   pre-GUP-427 units have no owner, so nothing prunes them.
