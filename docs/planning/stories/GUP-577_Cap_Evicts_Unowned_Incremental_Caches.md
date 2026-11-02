---
implementer: sonnet
implementer-reason:
  extends rule 5's candidate list in one script with a second kind of candidate;
  ordering, locks, report line and the seeded case are fixed below
touches:
  [
    scripts/prune_build_dir.sh,
    scripts/test_prune_build_dir.sh,
    maskfile.md,
    CLAUDE.md,
  ]
---

# GUP-577: The Cap Evicts Unowned Incremental Caches

## Story Overview

**Initiative**: Strategic Review 2026-11 (Track N0) **Status**: 📋 Planned
**Created**: 2026-11-01 (made decision-complete 2026-11-02)

## Context

[GUP-428](GUP-428_Prune_Superseded_Shared_Units.md) added rule 5 to
`scripts/prune_build_dir.sh`: when the build directory is over
`GUP_PRUNE_CAP_MB` (80 GB), it evicts units, least recently used first, until
the directory is at or under the cap. A unit's last use comes from cargo's
fingerprint graph (`unit_graph`). Rule 5's candidates are units only. Its
estimate (`start_kb - freed_kb`) counts everything on disk, incremental caches
included, and those it never evicts.

Incremental caches are bounded only by rules 2–4. Rule 2 keeps one session per
crate, rule 3 drops a cache idle for 3 days, and rule 4 trims a profile's caches
to 8 GB. The caches live in two places. `incremental/<checkout>/<crate>-<id>/`
holds a workspace member's caches, per checkout, kept apart by the workspace
wrapper. `incremental/<crate>-<id>/` holds clippy's shared caches and those of
path dependencies outside the workspace, which cargo also builds incrementally.
The live report in GUP-428's retrospective showed 8.7 GB of incremental caches.
A dry run at a 1 MB cap with no keep window ended "still over the cap": units
alone could not get under it.

GUP-428's bound test avoided the problem by serving its `depw<n>` crates from a
vendored directory source, which cargo builds without incremental caches.

## Read First

- `scripts/prune_build_dir.sh` lines 1–80 (the rules), 189–250
  (`prune_incremental`: how a crate's last activity is its newest session's
  mtime), 265–310 (`inventory`: incremental files are counted into `incr`, not
  units), 416–458 (`report`'s "rule 5 would evict" line) and 518–565
  (`prune_lru`).
- `scripts/test_prune_build_dir.sh` lines 200–331: `settle`, `make_depw`,
  `round`, `backdate` and the rule 5 cases.
- GUP-428's retrospective: "ZFS `du` lags", and "CI fix 2026-11-01" (measure
  once, before deleting).

## User Story

> "As the orchestrator, I want the cap to be a cap on the whole directory, so
> that incremental caches cannot keep it over."

## Acceptance Criteria

### AC1: Caches are rule 5 candidates

- [ ] A new function, `cache_candidates`, appends one line per incremental crate
      cache to `$work/units`, in the existing format
      `"<last use> <position> <kb> <owner> <directory>"`. Last use is the newest
      session directory's mtime (`s-*`, seconds), or the crate directory's mtime
      when it has no session. Position is `0`, kb is `du -sk`, and owner is
      `cache`. It covers both layouts: crate directories directly under
      `<profile>/incremental/`, and those under a checkout's
      `<profile>/incremental/<checkout>/` (a directory holding `.gup-checkout`).
- [ ] `prune_lru`'s loop evicts a `cache` line with `remove lru-cache "$dir"`.
      `remove` counts the freed kB itself, so the loop subtracts the line's kb
      from `total` as it does for units. Every other line still goes through
      `evict`. The keep window and the held-lock test apply to both kinds. A
      cache's profile is the part of its path before `/incremental/`.
- [ ] The summary line lists `lru-cache=N` beside `lru=N`. `--dry-run` prints
      `prune: would delete (lru-cache) <dir>`.
- [ ] `--report` prints `  rule 5 would evict: U units, C caches, X GB`,
      computed over both kinds.

### AC2: Seeded case

- [ ] `scripts/test_prune_build_dir.sh` gains a rule 5 case with path
      dependencies, after the existing bound rounds. A `make_pathw N` function
      writes a crate `pathw<N>` with `make_depw`'s 3 MB `include_bytes!` blob,
      as a path dependency outside the workspace (not vendored), so cargo builds
      it incrementally. Two rounds (31, 32) each use a removed worktree's own
      `pathw<N>`. After `backdate`, which now also backdates every incremental
      session two days except those under A's own checkout directory, the prune
      runs with the cap of the existing bound case. Then each round asserts
      three things: (a) the directory is at or under the cap; (b)
      `find "$build" -path '*/incremental/pathw*'` finds nothing; (c) the
      prune's output contains `lru-cache=`.
- [ ] A control in the same case proves the change is needed: with rule 5's
      cache candidates turned off by `GUP_PRUNE_CAP_CACHES=0` (a test-only knob,
      documented in the script header as such), the same round ends over the
      cap. The existing "still over the cap by" line is printed.
- [ ] The existing liveness check still passes after the new rounds: A's `a`,
      `dep` and `inner` are `Fresh`.
- [ ] `scripts/test_prune_build_dir.sh` passes on the default scratch directory,
      and on tmpfs (`GUP_TEST_SCRATCH=/dev/shm/gup-prune-test`) when `/dev/shm`
      has 2 GB free. Paste both summaries.

### AC3: Live dry run

- [ ] On the live directory, read-only:
      `GUP_PRUNE_CAP_MB=1 GUP_PRUNE_KEEP_MINUTES=0 GUP_PRUNE_WAIT=1 scripts/prune_build_dir.sh --dry-run`.
      Either it prints no "still over the cap" line, or the amount it prints is
      at most the "checkouts' own files" plus "other" lines of a `--report` run
      just before it. Paste both outputs, trimmed to their summary lines.

### AC4: Docs

- [ ] The script's rule 5 header paragraph says caches are candidates too, with
      the same keep window, locks and least-recently-used order. `maskfile.md`
      "## prune-build" and `CLAUDE.md`'s "Pruning" bullet each get the same fact
      in one clause.

## Decisions

- **One queue, oldest first, ties to caches.** Caches and units are sorted
  together by last use (`sort -k1,1n -k2,2n`). A cache's position `0` puts it
  before any unit used at the same second. A cache costs one non-incremental
  compile of one crate, whereas an evicted unit costs a rebuild of it and its
  dependents, so on a tie the cache goes first. There is no separate pass:
  "least recently used first" stays the one rule.
- **Every crate cache is a candidate**, owned or not. A checkout's own caches
  are bounded by rules 1–4 already, but over the cap the oldest go whatever
  their kind. The keep window protects any cache compiled in the last
  `GUP_PRUNE_KEEP_MINUTES`.
- **The whole crate directory goes**, not single sessions: rule 2 has already
  left one session per crate.
- **The estimate stays `start_kb - freed_kb`** (GUP-428's CI fix). `remove` adds
  each cache's kB to `freed_kb`. The loop's `total` is decremented by the line's
  kb, as for units.
- **`GUP_PRUNE_CAP_CACHES=0`** disables cache candidates, for the control only.
  It defaults to `1`, and the header says it is for tests.
- **The title stays** although owned caches are included. The story's file name
  and index entry predate this decision.

## Escalation Triggers

- The seeded `pathw<N>` caches are under 2 MB on disk on the test filesystem
  (lz4 compression), so the case cannot show the cap at work. Report the sizes.
  Do not change the blob to something larger than 3 MB without saying why.
- A warm checkout's build is no longer `Fresh` after a cache eviction. Cargo's
  freshness should not depend on incremental caches (rule 2's note); if it does,
  that is a finding.
- `--report`'s new count makes it noticeably slower: more than 2 s added on the
  live directory.

## Out of Scope

- Changing rules 1–4 or the cap's default.
- Per-checkout clippy artifacts: GUP-413.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] `cache_candidates`, the loop change, summary and report lines (AC1).
      Commit.
- [ ] `make_pathw`, `backdate` change, rounds 31–32, the control (AC2). Commit.
- [ ] Live dry run (AC3); docs (AC4); `shellcheck scripts/*.sh`;
      `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-428: Prune superseded shared units ✅ — rule 5, the graph and the test.

### Enables Stories

- None.

## Testing Strategy

- **Seeded**: `scripts/test_prune_build_dir.sh` (in `mask all-check`), on ZFS
  and tmpfs.
- **Live**: AC3's dry run, deleting nothing.

## Success Metrics

- [ ] Over the cap, rule 5 can always get the directory down to its checkouts'
      own files plus whatever the keep window protects.

## Risk Assessment

- **Low**: an evicted cache costs one non-incremental compile.
- **Low**: the test runs longer, by two rounds of small builds (about a minute).
  _Mitigation_: the rounds reuse the existing build directory.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `scripts/test_prune_build_dir.sh` and `mask all-check`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: not affected
- [ ] Rendered output verified by eye: not applicable
- [ ] `mask wasm-browser`: not needed, scripts only
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
