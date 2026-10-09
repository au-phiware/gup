# GUP-411: Keep Checkouts That Share a Target Directory from Reusing Each Other's Artifacts

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: ✅ Complete
(2026-10-10) **Created**: 2026-10-10

## Context

Agents run in git worktrees of this repository and share one `CARGO_TARGET_DIR`
(for example `/tmp/gup-target`) to save disk.
[GUP-409](GUP-409_Close_Gate_Gaps.md) found that this is not safe. Cargo hashes
a workspace member's package ID by its path _relative to the workspace root_, so
every checkout of the workspace produces the same artifact file names for `gup`,
`gup-core` and the other members. Cargo decides freshness by mtime. So a
checkout reuses an artifact that another checkout built from different source
whenever its own sources are older than that artifact:

- GUP-409's toy workspace: a second checkout with different content reported
  `Fresh a` and produced no build.
- GUP-409's seeded hook run: with the separation removed, the hook passed a
  staged clippy lint because the real tree had just built clean content into the
  same artifact.
- In the same run the real tree reported `Fresh` for `gup` and `gup-text` using
  artifacts built in the hook's snapshot (identical content that time).

A worktree is created with all its files written at once. If another agent then
builds `gup-core` from its own edits, this worktree's `gup-core` sources are
older than that artifact, so `cargo clippy`, `cargo test` or the pre-commit hook
can accept or reject code that is not in this checkout. The failure is silent
and depends on timing, the kind that costs a debugging session.

GUP-409 protected only the hook's snapshot, by giving workspace members a
different `incremental` profile setting in the snapshot. That trick cannot
separate N arbitrary checkouts: there are only two values.

## User Story

> "As an agent (or a developer with several worktrees) sharing a target
> directory to save disk, I want every build and lint to use artifacts built
> from my checkout's sources, so that a green or red result means something
> about my code."

## Acceptance Criteria

- [x] A seeded reproduction in two worktrees that share a target directory:
      worktree B (older sources) reports `Fresh` for a member that worktree A
      rebuilt from different content, and `cargo clippy` in B gives A's verdict.
      Recorded with commands and output (GUP-398's convention).
- [x] A fix, chosen and documented, after which the same reproduction makes B
      rebuild the member. Candidates: per-checkout target directories with
      shared dependency artifacts (cargo's `build.build-dir`, or `sccache`), a
      per-checkout `CARGO_TARGET_DIR` set by the dev shell, or another
      mechanism. Measure the disk and cold-build cost of the choice.
- [x] The orchestrator and story-worker guidance (`.github/agents/`) say how to
      share build output safely, and the dev shell's default (strategic review
      T0 item 5, "Set a default `CARGO_TARGET_DIR` in the dev shell") follows
      it.
- [x] The pre-commit hook's snapshot separation (GUP-409) is kept, replaced or
      removed to match, with its seeded test still passing.

## Technical Tasks

- [x] Reproduce with two `git worktree add` checkouts and one target directory;
      capture `cargo clippy -v` showing `Fresh`.
- [x] Prototype per-checkout isolation that keeps dependencies shared, and
      measure disk use and cold and warm `mask all-check` times.
- [x] Update `flake.nix` (if the dev shell sets the default), the agent docs and
      `scripts/pre_commit.sh`'s header comment.

## Dependencies

### Prerequisite Stories

- GUP-409 ✅: found the problem and the member-only profile separation.

### Enables Stories

(none known)

## Testing Strategy

- **Seeded reproduction**: two worktrees, one target directory, before and after
  the fix.
- **Script tests**: `scripts/test_pre_commit.sh`'s snapshot cases still pass.
- **Measurement**: disk use of the target directory and `mask all-check` times
  for one and for three concurrent worktrees.

## Success Metrics

- [x] No checkout reports `Fresh` for a member whose sources differ from the
      artifact's, in the seeded reproduction.
- [x] Disk use for three concurrent worktrees is measured and stays within what
      /tmp can hold (about 20 GB today).

## Risk Assessment

- **Medium**: per-checkout target directories multiply disk use, which has
  already exhausted /tmp in earlier stories. Mitigation: share dependency
  artifacts and isolate only workspace members.
- **Low**: changing the dev shell's environment affects every developer.
  Mitigation: keep it overridable and document it in `CLAUDE.md`.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] Lint and format clean: `mask all-check`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

### Mechanism

Cargo's `compute_metadata` (cargo 1.93) hashes the **workspace wrapper's path**
into the artifact names of workspace members only. The fix uses that, plus a
second knob for clippy, plus cargo's `build.build-dir` (stable since 1.91):

| Piece                                                | What it does                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `.cargo/config.toml` `build.rustc-workspace-wrapper` | `scripts/rustc_workspace_wrapper.sh`, a path relative to the config's checkout, so it is absolute and different in every checkout. `check`, `build`, `test`, `doc`, `bench` and `--release` get per-checkout member artifacts; dependencies keep their hashes and stay shared. Tracked, so it needs no setup and does not depend on an agent's (inherited) environment.                                                                  |
| `.cargo/config.toml` `[env] CLIPPY_CONF_DIR`         | `cargo clippy` replaces the workspace wrapper with clippy-driver, whose path is the same everywhere, so clippy's member artifacts stay shared. clippy-driver records `CLIPPY_CONF_DIR` in its dep-info and cargo compares it, so a checkout re-lints (`Dirty …: the environment variable CLIPPY_CONF_DIR changed`) when another checkout linted last. Correct, at the cost of a re-lint after each switch.                               |
| `scripts/rustc_workspace_wrapper.sh`                 | Execs rustc. Guard: a checkout nested inside another (an agent worktree under `.claude/worktrees/`) without its own config, i.e. a commit older than this story, inherits the outer checkout's config and wrapper path; the wrapper finds a `.git` between the member and its own checkout and fails with instructions instead of sharing silently.                                                                                      |
| `scripts/cargo_env.sh`                               | Prints `CARGO_BUILD_BUILD_DIR` (shared; argument, else inherited, else `~/.cache/gup/build`) and `CARGO_TARGET_DIR=<build dir>/checkouts/<checkout>-<hash>`. The target directory holds cargo's unhashed final artifacts (the binary `cargo run` executes, examples, docs, `.wasm`, visual-regression output), which a shared target directory lets another checkout overwrite between build and run. Hard links on the same filesystem. |
| `flake.nix`                                          | The dev shell evals `cargo_env.sh` on entry (T0 item 5), except under `CI` (the workflows cache `./target`) or over an explicit `CARGO_TARGET_DIR`.                                                                                                                                                                                                                                                                                      |
| GUP-409 snapshot flip                                | **Kept.** The snapshot is a checkout of its own, so the wrapper already separates its non-clippy artifacts; the `incremental` flip still gives it its own clippy artifacts, so the hook and the checkout do not re-lint each other on every commit. Header comment updated.                                                                                                                                                              |
| `tests/examples_smoke.rs`, `golden.rs`               | `gup_visual_regression::golden::target_dir` resolves the target directory (`CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR`, else `<root>/target`); examples_smoke used to look for example binaries next to its own binary, which is in the build directory once the two differ.                                                                                                                                                            |

Rejected: per-checkout `CARGO_TARGET_DIR` alone (every checkout rebuilds ~3 GB
of dependencies); `sccache` (cannot cache proc-macros, build scripts or linked
crates, and copies outputs into each target directory, so disk stays N×); a
per-checkout `RUSTC_WORKSPACE_WRAPPER` or profile setting in the environment
(agents inherit the orchestrator's environment, so the default would be wrong in
exactly the case that matters); a per-checkout `codegen-units` for members (the
only profile knob with arbitrarily many values; it would also separate clippy,
but needs per-checkout environment and changes release codegen, which would
confound before/after benchmarks like GUP-410's; see GUP-413).

### Seeded proof

Real repository, two worktrees (`/tmp/g411/wA`, `/tmp/g411/wB`) at `dbd9265`,
one `CARGO_BUILD_BUILD_DIR`, separate `CARGO_TARGET_DIR`s. B gets a seeded
`ptr_arg` lint in `crates/gup-culling-lod/src/lib.rs`
(`pub fn seeded_len(v: &Vec<i32>)`) dated an hour back (B was checked out
first); A gets the same function with `&[i32]`, edited now. A runs
`cargo clippy -p gup-culling-lod -- -D warnings` and `cargo check`, then B runs
them with `-v`. "Before" has no `.cargo/config.toml` in either; "after" has this
story's.

| Run                                 | Before                                          | After                                                                                                                                                 |
| ----------------------------------- | ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| A: clippy, check                    | exit 0, exit 0                                  | exit 0, exit 0                                                                                                                                        |
| B: `cargo clippy -v -- -D warnings` | **`Fresh gup-culling-lod`, exit 0: false pass** | `Dirty gup-culling-lod …: the dependency gup was rebuilt`, `Checking gup-culling-lod`, `error: writing &Vec instead of &[_]` at `lib.rs:41`, exit 101 |
| B: `cargo check -v`                 | **`Fresh gup-culling-lod`** (A's artifact)      | `Checking gup-culling-lod` (its own)                                                                                                                  |

The same steps on a toy workspace (member `a` with a proc-macro member `m` and
the registry crate `itoa`): before, B's clippy reported `Fresh a`, `Fresh m` and
exit 0, and `cargo run --bin show` printed A's `answer=2`; after, B re-ran
clippy (`Dirty m …: the environment variable CLIPPY_CONF_DIR changed`) and
failed on `ptr_arg`, `cargo run` printed B's `answer=1`, and `itoa` stayed
`Fresh`.

`scripts/test_shared_build_dir.sh` keeps this as a test (in `mask all-check`, so
CI's Lint workflow runs it): a control case without the config must reproduce
the bug, then with it B must lint and run its own code, a dependency outside the
workspace must stay `Fresh`, and a nested checkout without its own config must
fail. Seeded each piece away: without the wrapper line, 3 cases failed (B ran
A's build; the nested checkout built); without `CLIPPY_CONF_DIR`, 1 ("B's clippy
passed on A's verdict"); with the guard disabled, 2 (nested checkout).
`test_pre_commit.sh`'s snapshot cases, the GUP-409 seeded cases included, still
pass, and a real hook snapshot run used
`.git/gup-pre-commit/tree/scripts/rustc_workspace_wrapper.sh`.

### Measurements

Three worktrees of `dbd9265`, one fresh build directory, 8 cores, warm registry.
`mask all-check` (both clippy passes, validate-marks, the script tests):

| Run                                         | Time            | Build dir after |
| ------------------------------------------- | --------------- | --------------- |
| A, cold                                     | 262 s           | 3,248 MB        |
| B, first time (dependencies shared)         | 93 s            | 5,318 MB        |
| A, after B linted (clippy re-lints)         | 85 s            | 5,394 MB        |
| A, again, nothing changed                   | 26 s            | 5,427 MB        |
| A, B and C concurrently (C's first run)     | 256, 219, 182 s | 5,829 MB        |
| `cargo test -p gup-core --no-run`, A then B | 50 s, 18 s      | +1,053, +966 MB |

Each per-checkout target directory held 92 MB (`validate_marks`, a hard link
into the build directory). Three concurrent worktrees running `mask all-check`
used 5.8 GB, well within /tmp's 22 GB; test suites cost about 1 GB per crate per
checkout. The concurrent runs were serialized by cargo's build-directory lock
and each re-linted. Before this story the same three runs shared all member
artifacts (1× disk) but could judge each other's code.

### Files

- New: `.cargo/config.toml`, `scripts/rustc_workspace_wrapper.sh`,
  `scripts/cargo_env.sh`, `scripts/test_shared_build_dir.sh`.
- Changed: `flake.nix` (dev shell default), `maskfile.md` (all-check runs the
  new test; `ci tests` disk note), `scripts/pre_commit.sh` (header),
  `scripts/test_pre_commit.sh` (2 classifier cases: the new files force full
  mode), `crates/gup-visual-regression/src/golden.rs` (`target_dir`),
  `tests/examples_smoke.rs`, `CLAUDE.md` ("Sharing build output between
  checkouts": the recipe, concurrency, costs), `.github/agents/story-worker.md`
  (the "Build output" rule). There is no orchestrator file in `.github/agents/`;
  the orchestrator's recipe is the CLAUDE.md section, which every agent reads.

**Tests**: `test_shared_build_dir.sh` (5 checks, 3 seeds caught),
`test_pre_commit.sh` (all cases, 2 new), `cargo test -p gup-visual-regression`
(38 passed), examples_smoke filtered to `export_png` under a split build and
target directory (PASS; the PNG renders), `cargo check --examples`,
`mask all-check` clean.

## Retrospective

**Completed**: 2026-10-10

### Key Technical Learnings

#### Cargo hashes the workspace wrapper's path for members only

- **Challenge**: separate N checkouts' member artifacts while sharing
  dependencies. GUP-409's `incremental` flip has two values; profile settings
  have few, except `codegen-units`.
- **Solution**: reading cargo 1.93's `compute_metadata` showed that the
  workspace wrapper's path is hashed for workspace members only ("primarily here
  for clippy"). A config-relative `rustc-workspace-wrapper` is absolute and per
  checkout, tracked, and needs no environment.
- **Pattern**: when a tool's behaviour is the question, read the function that
  decides it (here a 100-line hash) before designing around guesses. Confirm on
  a toy workspace in seconds, then on the real one.

#### Clippy needs a different knob

- **Challenge**: `cargo-clippy` overwrites `RUSTC_WORKSPACE_WRAPPER` with
  clippy-driver's path, so the wrapper separates everything except the verdict
  the hook exists for.
- **Solution**: clippy-driver records `CLIPPY_CONF_DIR` in its dep-info, and
  cargo compares recorded env values against `[env]` config. A per-checkout
  `CLIPPY_CONF_DIR` makes clippy re-lint after a switch: correct, not separate.
- **Pattern**: freshness has two levers, the artifact name (separate) and the
  fingerprint (rebuild on mismatch). The second is enough for correctness and
  free in disk; it costs time under contention.

#### Final artifacts collide too

- **Challenge**: `cargo run` executes the unhashed `target/<profile>/<bin>` and
  releases the build lock before running it (checked on a toy), so with one
  target directory another checkout can replace the binary between build and
  run. examples_smoke, `validate_marks`, dogfood and `.wasm` outputs are all
  unhashed.
- **Solution**: share `build.build-dir` (stable since cargo 1.91) and give each
  checkout its own `CARGO_TARGET_DIR`; uplifts are hard links on the same
  filesystem. examples_smoke had to stop locating examples next to its own
  binary, which now lives in the build directory.
- **Pattern**: "share the target dir" conflates two things cargo now separates.
  Share intermediates, never final outputs.

#### Nested worktrees inherit config

- **Challenge**: agent worktrees live inside the main checkout, so a worktree of
  a commit without `.cargo/config.toml` silently inherits main's, wrapper path
  and all, and shares main's artifacts (GUP-410's benchmark case).
- **Solution**: the wrapper walks from the member up to its own checkout and
  fails if it meets another `.git` first. Clippy cannot be guarded this way;
  that residual (clippy on a pre-GUP-411 commit in a nested worktree) is
  documented.

### Architectural Decisions

#### Tracked config for correctness, environment only for placement

- **Decision**: correctness comes from `.cargo/config.toml`, which every
  checkout has and which follows the working directory; the environment
  (`cargo_env.sh`) only says where outputs go.
- **Reasoning**: agents inherit the orchestrator's environment, so anything
  per-checkout in the environment is wrong by default in exactly the case that
  matters. A forgotten `cargo_env.sh` now risks only final-artifact races, not a
  wrong lint or test verdict.
- **Trade-off**: clippy re-lints after a switch (26 s becomes ~90 s for
  `mask all-check`) instead of keeping per-checkout clippy artifacts.
- **Future**: GUP-413 can add per-checkout clippy artifacts via `codegen-units`
  if the orchestrator finds the re-lints costly.

#### Keep GUP-409's snapshot flip

- **Decision**: kept, re-documented.
- **Reasoning**: the snapshot is now separated anyway except for clippy, where
  the flip saves a re-lint of the checkout after every hook run, and back.
- **Trade-off**: two mechanisms to understand; the header comment explains how
  they divide the work.

#### Dev shell default: build dir in `~/.cache`, not `/tmp`

- **Decision**: `cargo_env.sh` defaults to
  `${XDG_CACHE_HOME:-~/.cache}/gup/build`, and respects an inherited
  `CARGO_BUILD_BUILD_DIR`; the orchestrator passes `/tmp/gup-target`. Skipped
  under `CI` (the workflows cache `./target`) and over an explicit
  `CARGO_TARGET_DIR`.
- **Reasoning**: `/tmp` is tmpfs on many systems; the cache directory is the
  conventional home for regenerable data. This machine's `/tmp` is a separate
  ZFS pool without snapshots, so the orchestrator should keep using it.

### Development Workflow Insights

- The toy workspace (two members, one registry crate, `--offline`) answered
  every cargo question in under a second per run; the real repository was only
  needed for the recorded proof and the measurements.
- `touch -d '-1 hour'` on the older checkout replaces sleeps and makes the mtime
  ordering deterministic; `scripts/test_shared_build_dir.sh` uses it.
- Measuring in fresh worktrees under `/tmp/g411` with a fresh build directory
  kept the numbers clean, but /tmp fell to 4 GB free at one point; deleting the
  measurement build directory as soon as the numbers were taken was necessary.
- The shell this session started in predated GUP-409's flake change
  (`actionlint` missing); `nix develop -c` per command fixed it.
- `/tmp/gup-target` still holds member artifacts under the old hashes (about 2.6
  GB). Dependencies keep their hashes and stay useful; the old member artifacts
  are dead weight until the directory is cleaned.

### Follow-up Stories

1. **GUP-412: Lint the wasm32 target in the gates** — GUP-410 found
   `arc_with_non_send_sync` at `crates/gup-core/src/selection.rs:251` with
   `cargo clippy --target wasm32-unknown-unknown` (confirmed here); no gate
   lints wasm32.
2. **GUP-413: Per-checkout clippy artifacts for concurrent agents** — optional:
   remove the ~60 s re-lint after a checkout switch with a per-checkout member
   `codegen-units`, if its disk cost is acceptable. Status 💡 New: a decision
   for the orchestrator, with the measurements to make it.
