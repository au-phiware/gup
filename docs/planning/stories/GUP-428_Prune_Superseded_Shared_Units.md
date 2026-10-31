---
implementer: sonnet
implementer-reason:
  shell work on an existing script; the liveness rule, cap, knobs, report format
  and seeded test are fixed in Decisions, and the cargo fingerprint format it
  relies on was verified on the live build directory
touches: [scripts/, maskfile.md, CLAUDE.md]
---

# GUP-428: Prune Superseded Shared Units

## Story Overview

**Initiative**: Strategic Review 2026-11 (Track N0) **Status**: 📋 Planned
**Created**: 2026-10-16 (rewritten 2026-11-01, after the disk incident)

## Context

[GUP-427](GUP-427_Bound_Build_Directory_Disk_Growth_Per_Commit.md) prunes the
shared build directory (`~/.cache/gup/build`) after every Rust hook run. Its
only rule for compiled units is rule 1: delete a unit when
`scripts/rustc_workspace_wrapper.sh`'s owner record
(`<profile>/.gup-owners/<crate>-<hash>`) names a checkout that no longer exists.
The parked trigger for this story, a disk alert, fired on 2026-11-01: the build
directory reached 184 GB and filled the 188 GB disk, so every build failed
(debug/deps 100 GB, debug/examples 35 GB, x86_64-unknown-linux-gnu 29 GB,
checkouts 15 GB, debug/incremental 11 GB, release 4 GB, wasm32 1.9 GB). A
`--dry-run` would have freed 4.4 GB, all of it incremental sessions. The
orchestrator deleted the whole directory.

Why rule 1 did not help, checked on 2026-11-01:

- **Rule 1 works.** A dry run on the rebuilt directory found the 423 units (13.7
  GB) of a removed agent worktree and would have deleted them. The hook prunes
  after every Rust commit in any checkout, so removed worktrees' owned units
  were already gone at the time of the incident. Dependencies are not built per
  checkout: all 1,780 fingerprints share one `rustc` and one `config` hash, and
  none has `rustflags`.
- **Nothing reclaims superseded units.** Cargo never deletes an artifact whose
  hash it stops using, and a hash changes whenever the dependency graph does. In
  the two weeks, main had 9 `Cargo.lock` commits and 21 `Cargo.toml` commits,
  and each of the ~40 worktrees started from a different main. A throwaway
  experiment (member `a` with a path dependency, `cargo build` and
  `cargo clippy`, then a version bump of the dependency and the same again) left
  all four old units after the prune, which freed 0 MB:
  - the old dependency units (build and check) have no owner record, so rule 1
    never sees them;
  - clippy's member unit has no owner record either: clippy-driver replaces the
    workspace wrapper;
  - the member's old build unit is owned by a checkout that still exists, so
    rule 1 keeps it forever. This covers the main checkout and its persistent
    pre-commit snapshot (`.git/gup-pre-commit/tree`), whose test and example
    binaries are hundreds of MB each.
- **Variants multiply this.** One crate builds as many units: `naga` had 20
  units in the rebuilt directory within an hour, in 4 profile hashes (check,
  build, `CARGO_PROFILE_DEV_DEBUG=0` tasks) and 2 feature sets, times feature
  unification across the hook's `-p` package selections.

What decides whether a unit is still worth keeping is how recently something
used it, and owner records cannot show that. Cargo's fingerprints can: each
unit's `.fingerprint/<pkg>-<hash>/<kind>-<target>.json` lists its dependencies'
fingerprint values, and each dependency's `<kind>-<target>` file holds its own
fingerprint value. So the prune can rebuild the dependency graph and give every
unit the newest build time of itself or anything that depends on it. This story
adds rule 5: a hard cap on the build directory, enforced by evicting the least
recently used units under cargo's locks. It also adds a `--report` that shows
where the space is. atime cannot measure use here: the dataset is mounted
`noatime` (`zfs get atime`: off).

## Read First

- `CLAUDE.md` "Sharing build output between checkouts", the **Pruning** and
  **Bound** bullets (lines 111–128).
- `scripts/prune_build_dir.sh`: the header (lines 1–47), `remove` (line 97),
  `prune_dead` (line 130), and the profile loop and summary (lines 219–249).
- `scripts/test_prune_build_dir.sh`: the scaffolding (lines 1–106), the seeded
  rounds (lines 131–163) and the Fresh case (lines 165–184).
- `scripts/rustc_workspace_wrapper.sh` lines 21–29 and 76–84 (owner records).
  Read only, because this story does not change it.
- `maskfile.md` `## prune-build` and `## clean` (lines 521–556).
- `scripts/pre_commit.sh` `prune_build_dir()` (lines 361–367).

## User Story

> "As the orchestrator running several agent worktrees, I want the shared build
> directory to stay under a fixed size no matter how many worktrees,
> `Cargo.lock` bumps or toolchain bumps go through it, so that a full disk never
> stops every build again."

## Acceptance Criteria

### AC1: `--report` shows where the space is

- [ ] `scripts/prune_build_dir.sh --report [BUILD_DIR]` deletes nothing, takes
      no lock, and prints the table in **Decisions** (D7).
- [ ] Run on the live `~/.cache/gup/build` (read-only), with the output and its
      run time pasted into the retrospective.
- [ ] `mask prune-build --report` runs it.

### AC2: Rule 5 caps the build directory

- [ ] After rules 1–4, when `du -sk` of the build directory (after `sync -f`)
      exceeds `GUP_PRUNE_CAP_MB` (default 81920), the prune evicts units in
      order of effective last use, oldest first, until the directory is at or
      under the cap (D1–D5).
- [ ] It never evicts a unit used within the last `GUP_PRUNE_KEEP_MINUTES`
      (default 60), or a unit in a profile whose lock it could not take. When
      only such units are left, it prints
      `prune: still over the cap by N MB: the rest was used in the last M minutes or is busy`.
- [ ] Evictions count as `lru=N` in the summary line. `--dry-run` prints
      `prune: would delete (lru) <path>` lines and deletes nothing.
- [ ] Under the cap, rule 5 costs one `sync -f` and one `du -sk`: on the live
      directory the hook's prune takes at most 2 s longer than before (measure
      with `time scripts/prune_build_dir.sh --dry-run` before and after; paste
      both into the retrospective).
- [ ] Over the cap, the dry run of rule 5 on the live directory
      (`GUP_PRUNE_CAP_MB=1 GUP_PRUNE_KEEP_MINUTES=0 scripts/prune_build_dir.sh --dry-run`)
      finishes in under 30 s. Paste the time.

### AC3: The graph matches cargo's fingerprints

- [ ] On the live directory, at least 95% of the dependency entries in
      fingerprint JSONs resolve to a fingerprint directory (D3). `--report`
      prints the share as `edges resolved: N of M`.
- [ ] Both test vectors in D3 resolve, including the one at or above 2^63.

### AC4: Seeded test proves the bound

`scripts/test_prune_build_dir.sh` gains the cases in D8, and all of them pass:

- [ ] Control: with the cap off, removed worktrees leave at least 2 MB of
      unowned units per round after rules 1–4, so the check would fail without
      rule 5.
- [ ] Bound: with the cap on, after every round's prune the build directory's
      on-disk size (after `sync -f`) is at or under the cap, the output has
      `lru=`, and no `depw<n>` file of a removed worktree is left in any `deps/`
      or `.fingerprint/`.
- [ ] Liveness: after the capped prunes, `cargo test -v --no-run` in the
      long-lived checkout prints a `Fresh` line for each of `a`, `dep` and
      `inner` (grep `'Fresh inner '` and so on, as GUP-427's case does). `inner`
      is reached only through `dep`, which proves the propagation.
- [ ] Keep window: with `GUP_PRUNE_KEEP_MINUTES=60` and nothing backdated, a
      capped prune evicts nothing and prints the "still over the cap" line.
- [ ] Safety: after `GUP_PRUNE_CAP_MB=0 GUP_PRUNE_KEEP_MINUTES=0`, the
      long-lived checkout's `cargo test -q` passes (it rebuilds what was
      evicted).
- [ ] The cases GUP-427 added still pass unchanged.

### AC5: Docs say what the prune now does

- [ ] `scripts/prune_build_dir.sh`'s header documents rule 5, its knobs and why
      it is safe, in the style of rules 1–4.
- [ ] `CLAUDE.md`'s **Pruning** bullet names rule 5, the 80 GB default cap and
      `--report`. The **Bound** bullet drops "Units built before GUP-427 have no
      owner record and are never pruned; delete the build directory once" and
      states the cap as the bound.
- [ ] `maskfile.md` `## prune-build` describes rule 5, lists `GUP_PRUNE_CAP_MB`
      and `GUP_PRUNE_KEEP_MINUTES`, and has the `--report` option.

## Decisions

- **D1, one cap for the whole build directory.** The measure is `du -sk` of
  `BUILD_DIR` (on disk, hard links counted once, `checkouts/` included), taken
  after `sync -f "$build"`. Reason: the disk is what fills, and ZFS allocates
  seconds after a write, so without the sync, `du` misses what the hook's own
  build just wrote (measured: `sync -f` 370 ms, `du -sk` 470 ms on 23 GB). Knob:
  `GUP_PRUNE_CAP_MB`, default `81920` (80 GB). Reason: two build-heavy agents
  use 21.8 GB each (CLAUDE.md, measured 2026-10-11), and with the main checkout
  and the shared dependencies that is a working set of about 60 GB. 80 GB leaves
  room so that LRU evicts superseded and idle units rather than live work, and
  keeps the cap under half of the 188 GB disk. MB, as with
  `GUP_PRUNE_INCREMENTAL_MB`, so that the test can set a cap of a few MB.
- **D2, a unit is a hash.** A unit is a fingerprint directory
  `<profile>/.fingerprint/<pkg>-<H>` (H is 16 hex digits). Its files, all
  deleted together, are `<profile>/.fingerprint/*-H`, `<profile>/build/*-H`,
  `<profile>/deps/*-H`, `<profile>/deps/*-H.*`, `<profile>/examples/*-H`,
  `<profile>/examples/*-H.*` and `<profile>/.gup-owners/*-H`. Any other link of
  those files under `$build/checkouts/` is deleted too (the uplifted
  `target/debug/<bin>` or `examples/<name>`), found through one
  `find "$build/checkouts" -type f -links +1 -printf '%i %p\n'`. Without that,
  the space is not freed and the next prune would evict again. Incremental
  caches are not units: rules 2–4 keep bounding them.
- **D3, the dependency graph comes from cargo's fingerprints, not a wrapper.**
  For each `<kind>-<target>.json` in fingerprint directory U, the file
  `<kind>-<target>` beside it holds U's fingerprint value as the hex of its 8
  little-endian bytes, and the JSON's `.deps[]` entries are
  `[pkg_hash, name, public, fingerprint]` with the fingerprint a decimal u64,
  which jq 1.8 prints exactly. The prune maps each hash file's value (byte pairs
  reversed, converted to decimal with bash `printf '%u' "$((16#...))"`) to its
  directory, and resolves every `.deps[][3]` through that map to an edge U→D.
  One value may map to several directories (edges to all). Edges cross profiles,
  because a `--target` build's proc macros and build scripts live in the host
  `debug/`. Unresolved entries are ignored, since they belong to units already
  stale. Test vectors from the live directory, verified 2026-11-01:
  `cfg-if-f28885dd39c07101/lib-cfg_if` = `4a54d51154794957` ↔
  6289691756583015498, and
  `naga-bd8c4a314ac86212/run-build-script-build-script-build` =
  `88a5067e12e52bb7` ↔ 13198895000525907336 (above 2^63). Reason: cargo writes
  these for every unit (dependencies, clippy units and build-script runs
  included), so no wrapper, config or hash changes.
- **D4, effective last use.** own(U) is the newest mtime of the files directly
  in U's fingerprint directory, which cargo rewrites (`invoked.timestamp`, the
  hash file) whenever it compiles or runs the unit. eff(D) = max(own(D), eff(U)
  over every edge U→D), computed in one pass over `tsort`'s order (dependents
  first). Declare edgeless nodes to tsort as `X X` pairs. Reason: a dependency
  built once and used Fresh for weeks is as live as its newest dependent, and
  evicting it would rebuild everything above it.
- **D5, eviction order and locking.** Rule 5 runs after rules 1–4 and the
  dead-checkout pass, and builds the graph only when the directory is over the
  cap. It takes every profile's `.cargo-lock` (each with `GUP_PRUNE_WAIT`) and
  holds them all while it evicts. A busy profile's units still count toward the
  size and still contribute edges, but are not candidates. Candidates are units
  in held profiles with eff older than `GUP_PRUNE_KEEP_MINUTES` (default 60),
  sorted by eff ascending, ties broken by tsort position (dependents first).
  Each is measured with `du -skc` of its files before deletion and subtracted
  from the total, until total ≤ cap. Reason for the keep window: a command
  sequence (`cargo test && cargo clippy`) releases the lock between commands,
  and the prune must not evict what the next command needs.
- **D6, no use stamps.** Cargo runs no rustc for a Fresh unit, so no wrapper
  (`build.rustc-wrapper` included) can see use. Effective last use is the newest
  build of the unit or of anything depending on it. Evicting a unit that is used
  but not rebuilt costs one rebuild and never breaks a build, because cargo
  rebuilds a unit whose output is missing.
- **D7, `--report` format.** It takes no locks and deletes nothing. Sizes are on
  disk, in GB with one decimal:

  ```text
  prune report: <build> (on disk X GB, cap Y GB)
    owned units, live checkouts        X GB  N units
    owned units, gone checkouts        X GB  N units
    unowned units (deps, clippy, runs) X GB  N units
    incremental caches                 X GB
    checkouts' own files               X GB
    other                              X GB
    by last use: <1h X, <1d X, <3d X, <7d X, older X GB
    edges resolved: N of M
    rule 5 would evict: N units, X GB
  ```

  "Checkouts' own files" counts files under `checkouts/` with a link count of 1
  (`find -links 1 -printf '%k'`). "Other" is the total minus the rest. A unit is
  owned when `.gup-owners/*-H` exists.

- **D8, the seeded cases** go in `scripts/test_prune_build_dir.sh` after the
  existing cases, in the same scratch (`~/.cache/gup/test-scratch`, never
  `/tmp`):
  - A dependency chain `a → dep → inner`: add a path crate `inner` that `dep`
    uses (`dep::one()` calls `inner::one()`).
  - Round n (n = 1..4): a worktree Wn whose `a/Cargo.toml` has
    `dep = { package = "depw<n>", path = "$tmp/depw<n>" }`. `depw<n>` is a copy
    of `dep` named `depw<n>` with 3,000 generated `pub fn f_<i>() -> u64`, so
    that its rlib is at least 2 MB. Wn runs `commit`, then is removed. A commits
    in the same round (its normal `dep`).
  - Control rounds run with `GUP_PRUNE_CAP_MB=999999`, the bound rounds in a
    fresh build directory. Before each capped prune, the test backdates every
    file under each profile's `.fingerprint/` with `touch -d '-2 days'`, then
    touches the fingerprint files of A's owned units
    (`grep -lx "$A_root" .gup-owners/*`) to now. The cap is `base_mb + 1`, where
    `base_mb` is A's warm on-disk size after `sync -f`, in MB, rounded up.
  - Size checks use a new `disk_kb()` (`sync -f "$build"; du -sk`). The existing
    `size_kb()` (apparent) stays for GUP-427's cases.
- **D9, where the code lives.** Everything goes in `scripts/prune_build_dir.sh`
  (functions `unit_graph`, `prune_lru`, `report`), with no new script. Tools:
  bash, jq, coreutils (`tsort`, `du`, `sync`, `find`, `stat`), all in the dev
  shell and the hook already. `rustc_workspace_wrapper.sh`, `cargo_env.sh` and
  `.cargo/config.toml` do not change.

## Escalation Triggers

- AC3 resolves fewer than 95% of the live directory's dependency entries, or
  either test vector fails. The fingerprint format then differs from the one
  verified on cargo 1.93.1.
- The over-cap dry run on the live directory takes more than 30 s.
- The Liveness case fails, so some edge reaches a unit through something other
  than fingerprints (for example `OUT_DIR` of a build-script run), and D3 is not
  enough.
- The live report shows more than 70 GB used in the last day. The default cap
  would then evict live work, and the owner decides the cap.
- Making it work seems to need a change to `rustc_workspace_wrapper.sh`,
  `.cargo/config.toml` or `cargo_env.sh` (use stamps, per-unit records).
- `sync -f` is missing or takes more than 5 s in the hook.

## Out of Scope

- Use stamps for Fresh units, and a global `build.rustc-wrapper` (D6).
- Per-checkout clippy artifacts: GUP-413.
- Eager rules below the cap (toolchain-stale units, old `Cargo.lock` units). The
  cap is the bound, and below it those units do no harm.
- Pruning at times other than the hook and `mask prune-build` (from
  `with_cargo_env.sh` or a timer). Propose it as a follow-up if the
  retrospective shows peaks between commits near the disk's size.
- `~/.cargo` (registry, git checkouts), which is not the build directory.

## Technical Tasks

- [ ] `unit_graph`: list fingerprint directories in every profile, own times,
      the hash-value map and the edges (D3), and compute eff (D4) with `tsort`.
- [ ] `report` and `--report` (D7). Run it on the live directory (AC1, AC3).
- [ ] `prune_lru` (D5) with the hard-link handling of D2; wire `lru` into the
      summary, `--dry-run` and the "still over the cap" line.
- [ ] Add the D8 cases to `scripts/test_prune_build_dir.sh` and its header.
- [ ] Update the script header, `CLAUDE.md` and `maskfile.md` (AC5).
- [ ] Time the hook's prune before and after, and the over-cap dry run (AC2).

## Dependencies

### Prerequisite Stories

- GUP-427 ✅: the prune, its locking, owner records and the seeded check.

### Coordinate With

- GUP-413 💡: per-checkout clippy units would add units. Rule 5 bounds them
  without changes.

## Testing Strategy

- **Script test**: the D8 cases in `scripts/test_prune_build_dir.sh`, run by
  `mask all-check`; shellcheck through `mask all-check`.
- **Live, read-only**: `--report` and the over-cap `--dry-run` on
  `~/.cache/gup/build`, with outputs and times in the retrospective. Never run
  rule 5 for real on the live directory as part of the story.

## Success Metrics

- [ ] The seeded build directory stays at or under its cap across 4 rounds of
      removed worktrees with distinct dependency versions, and the long-lived
      checkout stays Fresh.
- [ ] The live report accounts for at least 95% of the build directory in its
      named categories ("other" at most 5%).

## Risk Assessment

- **Medium**: evicting a live dependency rebuilds everything above it (minutes).
  _Mitigation_: eff propagation (D4), the keep window, an 80 GB default, and the
  Liveness case.
- **Medium**: fingerprints are cargo internals and may change format.
  _Mitigation_: the AC3 resolution share is printed by every `--report`, and an
  unresolved edge only makes a unit look older. That costs a rebuild, never a
  broken build (D6).
- **Low**: holding every profile lock delays a build by up to the prune's run
  time. _Mitigation_: the graph is built only over the cap, and the time is
  bounded by AC2.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `scripts/test_prune_build_dir.sh` (and
      `cargo test -- --test-threads=1` is unaffected: no Rust change)
- [ ] Lint and format clean: `mask all-fix`, then `mask all-check`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output: not applicable (no visual change)
- [ ] `mask wasm-browser`: not needed (no layout, input or wasm code change)
- [ ] WASM size: unchanged: no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
