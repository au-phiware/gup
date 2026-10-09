#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Tests that checkouts sharing a build directory do not use each other's
# workspace-member artifacts (GUP-411). A throwaway repository gets this
# repository's .cargo/config.toml and scripts/rustc_workspace_wrapper.sh, and
# two worktrees, A and B, share one CARGO_TARGET_DIR. B's sources are older
# than A's artifacts (cargo's freshness is by mtime), which is how a worktree
# created before another checkout's build looks.
#
# The control case runs the same steps without the config and must reproduce
# the bug; if it stops reproducing, cargo has changed and the mechanism may no
# longer be needed.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
config=$PWD/.cargo/config.toml
wrapper=$PWD/scripts/rustc_workspace_wrapper.sh

unset CARGO_BUILD_BUILD_DIR CARGO_BUILD_TARGET_DIR RUSTC_WORKSPACE_WRAPPER \
  CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER CLIPPY_CONF_DIR CARGO_INCREMENTAL \
  GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR \
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null CARGO_TERM_COLOR=never
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# A dependency outside the workspace (shared between checkouts) and a member
# with a clippy lint (ptr_arg) and a value to print.
mkdir -p "$tmp/dep/src"
printf '[package]\nname = "dep"\nversion = "0.1.0"\nedition = "2021"\n' \
  >"$tmp/dep/Cargo.toml"
echo 'pub fn one() -> i32 { 1 }' >"$tmp/dep/src/lib.rs"

# make_repo DIR WITH_CONFIG: a repository whose commit has B's content.
make_repo() {
  local repo=$1
  mkdir -p "$repo/a/src/bin"
  cd "$repo"
  git init -q .
  git config user.name test
  git config user.email test@example.com
  printf '[workspace]\nmembers = ["a"]\nresolver = "2"\n' >Cargo.toml
  printf '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\ndep = { path = "%s" }\n' \
    "$tmp/dep" >a/Cargo.toml
  printf 'pub fn answer() -> i32 { dep::one() }\npub fn len(v: &Vec<i32>) -> usize { v.len() }\n' \
    >a/src/lib.rs
  echo 'fn main() { println!("answer={}", a::answer()); }' >a/src/bin/show.rs
  if [[ $2 -eq 1 ]]; then
    mkdir -p .cargo scripts
    cp "$config" .cargo/config.toml
    cp "$wrapper" scripts/
  fi
  git add -A
  git commit -qm init
  git worktree add -q --detach "$repo-B"
  # B was checked out before A's builds below.
  find "$repo-B" -path "$repo-B/.git" -prune -o -type f \
    -exec touch -d '-1 hour' {} +
  # A fixes the lint and changes the answer.
  printf 'pub fn answer() -> i32 { dep::one() + 1 }\npub fn len(v: &[i32]) -> usize { v.len() }\n' \
    >a/src/lib.rs
}

failures=0
fail() {
  echo "FAIL: shared build dir: $1" >&2
  failures=$((failures + 1))
}

# run_case NAME WITH_CONFIG: prints B's observations after A's builds.
run_case() {
  local repo=$tmp/$1
  export CARGO_TARGET_DIR=$tmp/$1-target
  (make_repo "$repo" "$2")
  (cd "$repo" && cargo clippy -q -- -D warnings && cargo run -q --bin show) \
    >/dev/null 2>&1 || fail "$1: A's clippy or run failed"
  cd "$repo-B"
  clippy_exit=0
  cargo clippy -q -- -D warnings >/dev/null 2>&1 || clippy_exit=$?
  run_out=$(cargo run -v --bin show 2>&1) || true
  cd - >/dev/null
}

# Control: without the config, B replays A's verdict and runs A's code.
run_case control 0
[[ $clippy_exit -eq 0 ]] ||
  fail "control: B's clippy caught B's lint; cargo no longer shares member artifacts between checkouts"
grep -qx 'answer=2' <<<"$run_out" ||
  fail "control: B did not run A's build; cargo no longer shares member artifacts between checkouts"

# With the config: B lints and runs its own code, and the dependency is shared.
run_case isolated 1
[[ $clippy_exit -ne 0 ]] || fail "B's clippy passed on A's verdict"
grep -qx 'answer=1' <<<"$run_out" || fail "B ran A's build: $run_out"
grep -q 'Fresh dep' <<<"$run_out" || fail "B rebuilt the shared dependency: $run_out"

# A checkout nested inside another without its own config (an older commit in
# an agent worktree) must fail rather than share the outer checkout's
# artifacts.
repo=$tmp/isolated
git -C "$repo" commit -qam "A's change"
git -C "$repo" worktree add -q --detach .claude/worktrees/old HEAD~1
git -C "$repo/.claude/worktrees/old" rm -q -r .cargo scripts
nested_out=$(cd "$repo/.claude/worktrees/old" && cargo build 2>&1) &&
  fail "a nested checkout without its own config built with the outer one's"
grep -q 'separate checkout without its own .cargo/config.toml' <<<"$nested_out" ||
  fail "nested checkout: unexpected output: $nested_out"

if [[ $failures -gt 0 ]]; then
  echo "test_shared_build_dir: $failures case(s) failed" >&2
  exit 1
fi
echo "test_shared_build_dir: all cases passed"
