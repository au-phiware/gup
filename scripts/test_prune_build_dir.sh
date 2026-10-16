#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Seeded check for scripts/prune_build_dir.sh (GUP-427). A throwaway
# repository gets this repository's .cargo/config.toml and scripts, and its
# checkouts share one build directory, as scripts/cargo_env.sh sets up. Each
# seeded commit builds the tests and does the cargo work of the pre-commit
# hook (clippy on every target, a build) in the hook's staged snapshot. Each
# round commits in a long-lived checkout and in a short-lived worktree that is
# then removed, as an agent's is, and prunes: the build directory's size must
# come back to within CEILING_KB of its warm size, having grown by more than
# that unpruned (so the check would fail without the prune).
#
# Also checked: without the prune rustc leaves two incremental sessions per
# crate; a busy build directory is left alone; a warm checkout stays Fresh
# while another prunes, idle and over-cap incremental caches included.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
src=$PWD

unset CARGO_BUILD_BUILD_DIR CARGO_BUILD_TARGET_DIR CARGO_TARGET_DIR \
  RUSTC_WORKSPACE_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER CLIPPY_CONF_DIR \
  CARGO_INCREMENTAL GUP_PRUNE_IDLE_DAYS GUP_PRUNE_INCREMENTAL_MB GUP_PRUNE_WAIT \
  GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR \
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
  GUP_PRE_COMMIT_IN_SNAPSHOT GUP_PRE_COMMIT_SNAPSHOT_DIR
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null CARGO_TERM_COLOR=never
# Not /tmp, which build output must never fill, and not inside this checkout,
# whose .cargo/config.toml cargo would find.
scratch=${GUP_TEST_SCRATCH:-${XDG_CACHE_HOME:-$HOME/.cache}/gup/test-scratch}
mkdir -p "$scratch"
tmp=$(mktemp -d "$scratch/run.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
build=$tmp/build
# Net growth allowed per round once warm, after the prune. A round here
# recompiles a few KB of crate and leaves a worktree's units (MBs) behind.
CEILING_KB=${CEILING_KB:-256}

failures=0
fail() {
  echo "FAIL: prune build dir: $1" >&2
  failures=$((failures + 1))
}

# A dependency outside the workspace (shared) and a member with a test.
mkdir -p "$tmp/dep/src"
printf '[package]\nname = "dep"\nversion = "0.1.0"\nedition = "2021"\n' \
  >"$tmp/dep/Cargo.toml"
echo 'pub fn one() -> i32 { 1 }' >"$tmp/dep/src/lib.rs"

repo=$tmp/A
mkdir -p "$repo/a/src" "$repo/a/tests" "$repo/.cargo" "$repo/scripts"
cd "$repo"
git init -q .
git config user.name test
git config user.email test@example.com
printf '[workspace]\nmembers = ["a"]\nresolver = "2"\n' >Cargo.toml
printf '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\ndep = { path = "%s" }\n' \
  "$tmp/dep" >a/Cargo.toml
echo 'pub fn answer() -> i32 { dep::one() }' >a/src/lib.rs
echo '#[test] fn answers() { assert!(a::answer() > 0); }' >a/tests/t.rs
cp "$src/.cargo/config.toml" .cargo/
cp "$src/scripts/rustc_workspace_wrapper.sh" "$src/scripts/cargo_env.sh" \
  "$src/scripts/prune_build_dir.sh" "$src/scripts/pre_commit.sh" scripts/
printf 'target\n' >.gitignore
git add -A
git commit -qm init
git worktree add -q --detach "$tmp/B"

# at CHECKOUT CMD...: runs CMD in CHECKOUT with its cargo environment.
at() {
  local dir=$1
  shift
  (
    cd "$dir"
    eval "$(scripts/cargo_env.sh "$build")"
    "$@"
  )
}

# commit CHECKOUT N: a Rust change, a test build, and the cargo work of the
# hook (clippy on every target and a build), which runs in the hook's staged
# snapshot because of an untracked file. The snapshot gives members the
# opposite `incremental` setting, so its units are separate (GUP-409).
commit() {
  at "$1" sh -c "echo 'pub fn f_$2() -> i32 { 0 }' >>a/src/lib.rs &&
    echo $2 >untracked.txt && cargo test -q --no-run && git add a &&
    scripts/pre_commit.sh --exec sh -c 'cargo clippy -q --all-targets -- -D warnings && cargo build -q' >/dev/null &&
    git commit -qm 'commit $2'"
}

prune() { at "$1" scripts/prune_build_dir.sh "${@:2}"; }

# Apparent size: what was written, whether or not the filesystem has
# allocated it yet (ZFS allocates seconds later, so `du` lags), counting
# hard links once.
size_kb() { du -sk --apparent-size "$build" | cut -f1; }

# The most finalized sessions any incremental crate directory holds.
max_sessions() {
  find "$build" -path '*/incremental/*/s-*' -type d ! -name '*-working' \
    -printf '%h\n' | sort | uniq -c | sort -rn |
    awk 'NR == 1 { print $1 } END { if (NR == 0) print 0 }'
}

# Control: without a prune, rustc leaves two sessions per crate.
commit "$repo" 1
commit "$repo" 2
at "$tmp/B" cargo test -q --no-run
[[ $(max_sessions) -ge 2 ]] ||
  fail "control: rustc no longer keeps two sessions per crate; rule 1 may be unneeded"

# A busy build directory is left alone.
held=$tmp/held
flock -o "$build/debug/.cargo-lock" sh -c "touch '$held' && exec sleep 60" &
holder=$!
while [[ ! -e $held ]]; do sleep 0.1; done
busy_out=$(GUP_PRUNE_WAIT=1 prune "$repo")
pkill -P "$holder" 2>/dev/null || true
wait "$holder" 2>/dev/null || true
grep -q 'skipped (cargo busy for 1s): debug' <<<"$busy_out" ||
  fail "a held lock did not skip debug: $busy_out"
[[ $(max_sessions) -ge 2 ]] || fail "pruned debug while its lock was held"

prune "$repo" >/dev/null
[[ $(max_sessions) -eq 1 ]] || fail "prune left $(max_sessions) sessions in a crate"
base=$(size_kb)

# Seeded rounds: a commit in the long-lived checkout A, and one in a
# short-lived worktree (an agent's) that is removed afterwards, its snapshot
# with it. Without the prune each round adds that worktree's units; with it,
# the build directory returns to within CEILING_KB of its warm size.
for n in 3 4 5 6; do
  commit "$repo" "$n"
  w=$tmp/W$n
  git -C "$repo" worktree add -q --detach "$w"
  commit "$w" "w$n"
  w_root=$(cd "$w" && pwd -P)
  # shellcheck disable=SC2016 # expanded by the inner shell
  w_target=$(at "$w" sh -c 'echo "$CARGO_TARGET_DIR"')
  mapfile -t w_units < <(grep -lx "$w_root" "$build"/debug/.gup-owners/* || true)
  [[ ${#w_units[@]} -gt 0 ]] || fail "round $n: no ownership stamps for W$n's units"
  git -C "$repo" worktree remove --force "$w"
  before=$(size_kb)
  out=$(prune "$repo")
  after=$(size_kb)
  echo "round $n: $((before - base)) KB above the warm size unpruned, $((after - base)) KB after the prune"
  [[ $((before - base)) -gt $CEILING_KB ]] ||
    fail "round $n: grew only $((before - base)) KB unpruned; the check would pass without the prune"
  [[ $((after - base)) -le $CEILING_KB ]] ||
    fail "round $n: grew $((after - base)) KB after the prune (ceiling $CEILING_KB KB): $out"
  [[ $(max_sessions) -eq 1 ]] || fail "round $n: prune left $(max_sessions) sessions"
  grep -q 'dead-checkout=1' <<<"$out" ||
    fail "round $n: W$n's target directory was not pruned: $out"
  [[ ! -e $w_target ]] || fail "round $n: W$n's target directory survived"
  for owner in "${w_units[@]}"; do
    unit=${owner##*/}
    [[ -z $(find "$build" -name "*-${unit##*-}*" -print -quit) ]] ||
      fail "round $n: W$n's unit $unit survived"
  done
done

# A warm checkout (B) stays Fresh while another prunes everything it may:
# idle incremental caches, then everything over a zero cap.
find "$build" -path '*/incremental/*/s-*' -type d -exec touch -d '-10 days' {} +
idle_out=$(prune "$repo")
grep -q 'idle=' <<<"$idle_out" || fail "idle caches were not pruned: $idle_out"
commit "$repo" 7
cap_out=$(GUP_PRUNE_INCREMENTAL_MB=0 prune "$repo")
grep -q 'capped=' <<<"$cap_out" || fail "the size cap pruned nothing: $cap_out"
fresh_out=$(at "$tmp/B" cargo test -v --no-run 2>&1)
grep -q 'Fresh a ' <<<"$fresh_out" || fail "B was rebuilt after the prunes: $fresh_out"
grep -q 'Fresh dep ' <<<"$fresh_out" ||
  fail "the shared dependency was rebuilt: $fresh_out"
a_out=$(at "$repo" cargo test -q 2>&1) ||
  fail "A's tests failed after the prunes: $a_out"

if [[ $failures -gt 0 ]]; then
  echo "test_prune_build_dir: $failures case(s) failed" >&2
  exit 1
fi
echo "test_prune_build_dir: all cases passed"
