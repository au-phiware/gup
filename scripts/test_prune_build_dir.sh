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
#
# Rule 5 (GUP-428, the cap on the whole directory): removed worktrees each
# leave a distinct dependency (depw<n>, over 2 MB) that no owner record names.
# With the cap off the directory grows by it every round. With the cap on it
# stays at or under the cap across four rounds, nothing of a removed worktree's
# dependency survives, the long-lived checkout stays Fresh (including `inner`,
# which only `dep` uses), the keep window protects recent units, and a zero
# cap still leaves a checkout that builds.
#
# Rule 5 also evicts incremental caches (GUP-577): removed worktrees each leave
# a path dependency outside the workspace (pathw<n>, whose incremental cache
# no checkout directory holds). The cap must hold with the caches as
# candidates, none of pathw<n> survives, and with the cache candidates off
# (GUP_PRUNE_CAP_CACHES=0) the same state stays over the cap.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
src=$PWD

unset CARGO_BUILD_BUILD_DIR CARGO_BUILD_TARGET_DIR CARGO_TARGET_DIR \
  RUSTC_WORKSPACE_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER CLIPPY_CONF_DIR \
  CARGO_INCREMENTAL GUP_PRUNE_IDLE_DAYS GUP_PRUNE_INCREMENTAL_MB GUP_PRUNE_WAIT \
  GUP_PRUNE_CAP_MB GUP_PRUNE_KEEP_MINUTES GUP_PRUNE_CAP_CACHES \
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
mkdir -p "$tmp/dep/src" "$tmp/inner/src"
printf '[package]\nname = "inner"\nversion = "0.1.0"\nedition = "2021"\n' \
  >"$tmp/inner/Cargo.toml"
echo 'pub fn one() -> i32 { 1 }' >"$tmp/inner/src/lib.rs"
printf '[package]\nname = "dep"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\ninner = { path = "%s" }\n' \
  "$tmp/inner" >"$tmp/dep/Cargo.toml"
echo 'pub fn one() -> i32 { inner::one() }' >"$tmp/dep/src/lib.rs"
# Old sources: the rule 5 cases backdate fingerprints, and cargo calls a unit
# stale when a source is newer than its fingerprint.
find "$tmp/dep" "$tmp/inner" -exec touch -d '-10 days' {} +

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
# The rule 5 cases' dependencies come from a directory source (see make_depw).
mkdir -p "$tmp/vendor"
printf '\n[source.crates-io]\nreplace-with = "vendored"\n[source.vendored]\ndirectory = "%s"\n' \
  "$tmp/vendor" >>.cargo/config.toml
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

# --- Rule 5: the cap on the whole build directory (GUP-428) -----------------

# settle: ZFS allocates a write's blocks (and frees a delete's) at the next
# transaction group, up to 5 s later, and neither `sync -f` nor `sync` waits
# for that, so `du` lags. Wait it out, so that the prune and the checks below
# see the same sizes.
settle() {
  sync -f "$build"
  [[ $(stat -f -c %T "$build") != zfs ]] || sleep 6
}

# On-disk size.
disk_kb() {
  settle
  du -sk "$build" | cut -f1
}

# prune_settled CHECKOUT [ARG...]: prune after the filesystem has settled.
prune_settled() {
  settle
  prune "$@"
}

# make_depw N: a crate depw<N> holding 3 MB of random bytes
# that nothing but a removed worktree uses, in the vendored directory source.
# A vendored crate is not a path dependency, so cargo builds it without an
# incremental cache (rules 2-4 would bound one, and this check is of rule 5).
make_depw() {
  local d=$tmp/vendor/depw$1
  mkdir -p "$d/src"
  printf '[package]\nname = "depw%s"\nversion = "0.1.0"\nedition = "2021"\n' \
    "$1" >"$d/Cargo.toml"
  echo '{"files":{},"package":null}' >"$d/.cargo-checksum.json"
  # Random bytes do not compress (the filesystem may), in the rlib and, as a
  # const, in the rmeta that clippy's check build leaves.
  head -c 3000000 /dev/urandom >"$d/src/blob.bin"
  cat >"$d/src/lib.rs" <<'RUST'
pub const BLOB: &[u8] = include_bytes!("blob.bin");
pub fn one() -> i32 { BLOB.len() as i32 }
RUST
}

# round N: A commits, and so does a worktree that uses its own depw<N> and is
# then removed, as an agent's is.
round() {
  local w=$tmp/W$1
  commit "$repo" "r$1"
  git -C "$repo" worktree add -q --detach "$w"
  make_depw "$1"
  sed -i "s|^dep = .*|dep = { package = \"depw$1\", version = \"0.1.0\" }|" \
    "$w/a/Cargo.toml"
  commit "$w" "w$1"
  git -C "$repo" worktree remove --force "$w"
}

# Control: with the cap off, each round leaves a removed worktree's dependency
# behind, which no owner record names, so rules 1-4 cannot see it.
for n in 11 12; do
  before=$(disk_kb)
  round "$n"
  GUP_PRUNE_CAP_MB=999999 prune_settled "$repo" >/dev/null
  grew=$(($(disk_kb) - before))
  echo "control round $n: $grew KB left after rules 1-4"
  [[ $grew -ge 2048 ]] ||
    fail "control round $n: left only $grew KB; the cap check would pass without rule 5"
done

# A fresh build directory, warm with A alone, sets the cap.
build=$tmp/build2
a_root=$(cd "$repo" && pwd -P)
commit "$repo" b1
commit "$repo" b2
prune_settled "$repo" >/dev/null
base_mb=$((($(disk_kb) + 1023) / 1024))
cap_mb=$((base_mb + 1))
echo "bound: warm size $base_mb MB, cap $cap_mb MB"

# backdate: every fingerprint file two days old (a fixed time, so that ages
# tie), except A's own units' (the checkout in use), which are now. So is every
# incremental session, except those in A's own checkout directory.
backdate() {
  local old owners unit hash profile marker
  local -a own_caches=()
  old=$(($(date +%s) - 172800))
  while IFS= read -r marker; do
    [[ $(head -n 1 "$marker") != "$a_root" ]] || own_caches+=(-path "${marker%/*}" -prune -o)
  done < <(find "$build" -path "$build/checkouts" -prune -o -name .gup-checkout -type f -print)
  find "$build" -path "$build/checkouts" -prune -o "${own_caches[@]}" \
    -path '*/incremental/*' -type d -name 's-*-*' -exec touch -d "@$old" {} +
  find "$build" -path "$build/checkouts" -prune -o -path '*/.fingerprint/*' \
    -type f -exec touch -d "@$old" {} +
  while IFS= read -r owners; do
    profile=${owners%/.gup-owners}
    for unit in "$owners"/*; do
      grep -qx "$a_root" "$unit" || continue
      hash=${unit##*-}
      touch -c "$profile"/.fingerprint/*-"$hash"/*
    done
  done < <(find "$build" -path "$build/checkouts" -prune -o -name .gup-owners -type d -print)
}

# Keep window: nothing is older than an hour, so nothing may be evicted.
round 21
keep_out=$(GUP_PRUNE_CAP_MB=$cap_mb GUP_PRUNE_KEEP_MINUTES=60 prune_settled "$repo")
grep -q 'lru=' <<<"$keep_out" && fail "the keep window did not protect recent units: $keep_out"
grep -q 'still over the cap by' <<<"$keep_out" ||
  fail "no 'still over the cap' line within the keep window: $keep_out"

# Bound: four rounds with the cap on.
for n in 21 22 23 24; do
  [[ $n -eq 21 ]] || round "$n"
  backdate
  out=$(GUP_PRUNE_CAP_MB=$cap_mb prune_settled "$repo")
  size=$(disk_kb)
  echo "bound round $n: $size KB on disk after the prune, cap $((cap_mb * 1024)) KB"
  [[ $size -le $((cap_mb * 1024)) ]] ||
    fail "round $n: $size KB on disk exceeds the cap of $((cap_mb * 1024)) KB: $out"
  grep -q 'lru=' <<<"$out" || fail "round $n: rule 5 evicted nothing: $out"
  left=$(find "$build" \( -path '*/deps/*' -o -path '*/.fingerprint/*' \) \
    -name '*depw*' -print -quit)
  [[ -z $left ]] || fail "round $n: a removed worktree's dependency survived: $left"
done

# Rule 5 with caches (GUP-577): path dependencies outside the workspace are
# built incrementally, so a removed worktree leaves their caches in
# incremental/<crate>-<id>/, which no owner record or checkout directory names.

# make_pathw N: the crate pathw<N> (make_depw's 3 MB blob) as a path dependency.
make_pathw() {
  local d=$tmp/pathw$1
  mkdir -p "$d/src"
  printf '[package]\nname = "pathw%s"\nversion = "0.1.0"\nedition = "2021"\n' \
    "$1" >"$d/Cargo.toml"
  head -c 3000000 /dev/urandom >"$d/src/blob.bin"
  cat >"$d/src/lib.rs" <<'RUST'
pub const BLOB: &[u8] = include_bytes!("blob.bin");
pub fn one() -> i32 { BLOB.len() as i32 }
RUST
  # Old sources, as for dep: backdated fingerprints must not look stale.
  find "$d" -exec touch -d '-10 days' {} +
}

# round_path N: round, with a path dependency.
round_path() {
  local w=$tmp/W$1
  commit "$repo" "r$1"
  git -C "$repo" worktree add -q --detach "$w"
  make_pathw "$1"
  sed -i "s|^dep = .*|dep = { package = \"pathw$1\", path = \"$tmp/pathw$1\" }|" "$w/a/Cargo.toml"
  commit "$w" "w$1"
  git -C "$repo" worktree remove --force "$w"
}

for n in 31 32; do
  round_path "$n"
  backdate
  settle # ZFS: du lags the writes by up to 5 s
  pathw_kb=$(find "$build" -path "*/incremental/pathw$n-*" -type d -prune \
    -exec du -sk {} + | awk '{ s += $1 } END { print s + 0 }')
  echo "cache round $n: pathw$n's incremental cache holds $pathw_kb KB"
  [[ $pathw_kb -ge 2048 ]] ||
    fail "cache round $n: pathw$n's cache is only $pathw_kb KB; the case cannot show the cap at work"
  # Control: units alone cannot get under the cap.
  ctl_out=$(GUP_PRUNE_CAP_CACHES=0 GUP_PRUNE_CAP_MB=$cap_mb prune_settled "$repo")
  ctl_size=$(disk_kb)
  echo "cache round $n control (caches not candidates): $ctl_size KB on disk, cap $((cap_mb * 1024)) KB"
  [[ $ctl_size -gt $((cap_mb * 1024)) ]] ||
    fail "cache round $n: the control ended at $ctl_size KB, within the cap: the change is not needed"
  grep -q 'still over the cap by' <<<"$ctl_out" ||
    fail "cache round $n: the control printed no 'still over the cap' line: $ctl_out"
  out=$(GUP_PRUNE_CAP_MB=$cap_mb prune_settled "$repo")
  size=$(disk_kb)
  echo "cache round $n: $size KB on disk after the prune, cap $((cap_mb * 1024)) KB"
  [[ $size -le $((cap_mb * 1024)) ]] ||
    fail "cache round $n: $size KB on disk exceeds the cap of $((cap_mb * 1024)) KB: $out"
  left=$(find "$build" -path '*/incremental/pathw*' -print -quit)
  [[ -z $left ]] || fail "cache round $n: a path dependency's cache survived: $left"
  grep -q 'lru-cache=' <<<"$out" || fail "cache round $n: rule 5 evicted no cache: $out"
done

# Liveness: A's units, and `inner` that only `dep` uses, are still Fresh.
fresh_out=$(at "$repo" cargo test -v --no-run 2>&1)
for unit in a dep inner; do
  grep -q "Fresh $unit " <<<"$fresh_out" ||
    fail "rule 5 evicted a live unit: $unit was rebuilt: $fresh_out"
done

# Safety: with nothing kept, the next build only rebuilds.
GUP_PRUNE_CAP_MB=0 GUP_PRUNE_KEEP_MINUTES=0 prune_settled "$repo" >/dev/null
a_out=$(at "$repo" cargo test -q 2>&1) ||
  fail "A's tests failed after a zero cap: $a_out"

if [[ $failures -gt 0 ]]; then
  echo "test_prune_build_dir: $failures case(s) failed" >&2
  exit 1
fi
echo "test_prune_build_dir: all cases passed"
