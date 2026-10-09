#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Tests for the pre-commit hook's classifier (scripts/pre_commit.sh --plan).
# Each case gives staged paths and the plan line the hook must print. The
# unsafe direction (scoping away a crate that should be checked) is what these
# cases guard: when in doubt the expected plan is `mode=full`.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

failures=0
expect() {
  local want=$1
  shift
  local got
  got=$(./scripts/pre_commit.sh --plan "$@" | grep -F -- "$want" || true)
  if [[ -z $got ]]; then
    echo "FAIL: [$*] expected '$want', got:" >&2
    ./scripts/pre_commit.sh --plan "$@" | sed 's/^/    /' >&2
    failures=$((failures + 1))
  fi
}

# Documentation only: no Rust check.
expect 'mode=docs' README.md
expect 'mode=docs' docs/planning/stories/INDEX.md crates/gup-core/README.md
expect 'mode=docs' COPYING

# Docs a crate compiles (doctests, fonts) are that crate's sources.
expect 'clippy=[gup gup-culling-lod]' docs/tutorials/01_getting_started.md
expect 'clippy=[gup gup-culling-lod]' assets/fonts/default.ttf
expect 'clippy=[gup gup-core gup-culling-lod gup-text]' \
  crates/gup-text/fonts/Inter-Regular.ttf

# A leaf crate is checked alone; a dependency drags in its dependents.
expect 'fmt=[gup-text] clippy=[gup gup-core gup-culling-lod gup-text]' \
  crates/gup-text/src/lib.rs
expect 'clippy=[gup-culling-lod]' crates/gup-culling-lod/src/lib.rs
expect 'fmt=[gup-core] clippy=[gup gup-core gup-culling-lod]' \
  crates/gup-core/src/lib.rs
expect 'clippy=[gup gup-culling-lod gup-macros]' gup-macros/src/lib.rs
expect 'clippy=[gup gup-core gup-culling-lod gup-visual-regression]' \
  crates/gup-visual-regression/src/lib.rs
expect 'clippy=[gup gup-culling-lod]' src/lib.rs tests/kde_tests.rs
expect 'clippy=[gup gup-culling-lod]' src/lib.rs README.md

# The crates that build for wasm32 are linted on that target too (GUP-412);
# the proc-macro and the quarantined parts bin are not.
expect 'clippy=[gup gup-core gup-culling-lod] wasm32=[gup gup-core]' \
  crates/gup-core/src/lib.rs
expect 'clippy=[gup gup-culling-lod gup-macros] wasm32=[gup]' \
  gup-macros/src/lib.rs
expect 'wasm32=[gup gup-core gup-text]' crates/gup-text/src/lib.rs
expect 'wasm32=[gup gup-core gup-visual-regression]' \
  crates/gup-visual-regression/src/lib.rs
expect 'clippy=[gup-culling-lod] wasm32=[] ' crates/gup-culling-lod/src/lib.rs
expect 'mode=full' scripts/clippy_wasm32.sh

# The detached dogfood crate gets its own rustfmt check.
expect 'clippy=[] wasm32=[] dogfood=1' dogfood/src/lib.rs

# Workspace-level and unrecognised files force the full check.
expect 'mode=full' Cargo.toml
expect 'mode=full' Cargo.lock
expect 'mode=full' rust-toolchain.toml
expect 'mode=full' flake.nix
expect 'mode=full' maskfile.md
expect 'mode=full' scripts/pre_commit.sh
expect 'mode=full' scripts/generate_gallery.sh
expect 'mode=full' .cargo/config.toml
expect 'mode=full' scripts/rustc_workspace_wrapper.sh

# Workflow files get actionlint, alone or on top of the other checks.
expect 'mode=workflows' .github/workflows/tests.yml
expect 'actionlint=1' .github/workflows/tests.yml
expect 'mode=docs' .github/workflows/README.md
expect 'clippy=[gup-culling-lod] wasm32=[] dogfood=0 actionlint=1' \
  .github/workflows/lint.yml crates/gup-culling-lod/src/lib.rs
expect 'mode=full' .github/workflows/lint.yml maskfile.md
expect 'mode=full' gup-egui/src/lib.rs
expect 'mode=full' examples/gup-tauri/src-tauri/src/main.rs
expect 'mode=full' crates/gup-core/src/lib.rs Cargo.lock
expect 'mode=full' README.md some/new/file.rs

# ── The checks see the staged snapshot, not the working tree (GUP-409) ──
#
# A throwaway repository with this script in it. The "check" is a grep for
# VIOLATION, run with `pre_commit.sh --exec` where the real checks would run;
# its verdict must match the staged content alone.
snapshot_cases() (
  # The hook (and a full-mode snapshot run) exports these; they would point
  # git at the real repository.
  unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR \
    GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
    GUP_PRE_COMMIT_IN_SNAPSHOT GUP_PRE_COMMIT_SNAPSHOT_DIR CARGO_INCREMENTAL
  export CARGO_TARGET_DIR=/nonexistent-target GIT_CONFIG_NOSYSTEM=1
  export GIT_CONFIG_GLOBAL=/dev/null
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  hook=$PWD/scripts/pre_commit.sh
  repo=$tmp/repo
  git init -q "$repo"
  cd "$repo"
  git config user.name test
  git config user.email test@example.com
  mkdir -p scripts
  cp "$hook" scripts/pre_commit.sh
  printf 'clean\n' >a.txt
  printf 'clean\n' >b.txt
  printf 'clean\n' >gone.txt
  git add -A
  git commit -qm init
  fails=0
  snap=$repo/.git/gup-pre-commit/tree
  where() { ./scripts/pre_commit.sh --exec pwd -P 2>/dev/null | tail -n 1; }
  # check: 0 when no VIOLATION is visible where the checks run.
  check() {
    ./scripts/pre_commit.sh --exec sh -c '! grep -rq --exclude-dir=.git VIOLATION .' \
      >/dev/null 2>&1
  }
  case_() {
    local name=$1 want=$2 got=0
    shift 2
    "$@" || got=$?
    [[ $got -ne 0 ]] && got=1
    if [[ $got -ne $want ]]; then
      echo "FAIL: snapshot: $name (expected exit $want, got $got)" >&2
      fails=$((fails + 1))
    fi
  }

  # A clean working tree is checked in place.
  case_ 'clean tree runs in place' 0 test "$(where)" = "$(pwd -P)"

  # Direction 1: an untracked file with a violation that is not being
  # committed must not fail the commit.
  printf 'fixed\n' >a.txt
  git add a.txt
  printf 'VIOLATION\n' >untracked.txt
  case_ 'dirty tree runs in the snapshot' 0 test "$(where)" = "$snap"
  case_ 'untracked violation is not checked' 0 check
  case_ 'untracked file is not in the snapshot' 0 test ! -e "$snap/untracked.txt"
  rm untracked.txt

  # Direction 2: an unstaged edit to a staged file. A violation only in the
  # working tree must not fail the commit ...
  printf 'VIOLATION\n' >a.txt
  case_ 'unstaged violation is not checked' 0 check
  # ... and an unstaged fix must not let a staged violation through.
  git add a.txt
  printf 'fixed\n' >a.txt
  case_ 'staged violation is checked despite an unstaged fix' 1 check
  case_ 'working tree is untouched' 0 grep -qx fixed a.txt

  # The index git hands the hook (git commit -a / git commit PATH use a
  # temporary one) is the one checked.
  cp .git/index "$tmp/next-index"
  GIT_INDEX_FILE=$tmp/next-index git add a.txt
  printf 'VIOLATION\n' >a.txt
  case_ 'GIT_INDEX_FILE is honoured' 0 env GIT_INDEX_FILE="$tmp/next-index" \
    sh -c 'exec ./scripts/pre_commit.sh --exec \
      sh -c "! grep -rq --exclude-dir=.git VIOLATION ." >/dev/null 2>&1'
  printf 'fixed\n' >a.txt
  git add a.txt

  # Re-syncing rewrites only changed files (cargo's mtime freshness), removes
  # files deleted from the index and cleans debris left in the snapshot.
  printf 'dirty\n' >b.txt
  check || true
  before=$(stat -c %y "$snap/a.txt")
  printf 'debris\n' >"$snap/debris.txt"
  git rm -q --cached gone.txt
  check || true
  case_ 'unchanged file is not rewritten' 0 test \
    "$(stat -c %y "$snap/a.txt")" = "$before"
  case_ 'file deleted from the index is removed' 0 test ! -e "$snap/gone.txt"
  case_ 'debris is cleaned' 0 test ! -e "$snap/debris.txt"

  # Git in the snapshot sees the staged index, and cargo gets its own member
  # artifacts.
  case_ 'git in the snapshot sees the staged change' 0 test \
    "$(./scripts/pre_commit.sh --exec git diff --cached --name-only 2>/dev/null |
      tail -n 1)" = gone.txt
  case_ 'snapshot cargo config separates members' 0 grep -qx \
    'incremental = false' "$repo/.git/gup-pre-commit/.cargo/config.toml"

  exit "$fails"
)
if ! snapshot_cases; then
  failures=$((failures + 1))
fi

# Bare `actionlint` finds the workflows next to the nearest `.git` above the
# working directory, which from the snapshot is the real checkout's: every
# invocation must name the files.
if grep -nE "'actionlint'|'actionlint &&|&& actionlint'" maskfile.md \
  scripts/pre_commit.sh >&2; then
  echo "FAIL: actionlint runs without naming the workflow files" >&2
  failures=$((failures + 1))
fi

if [[ $failures -gt 0 ]]; then
  echo "test_pre_commit: $failures case(s) failed" >&2
  exit 1
fi
echo "test_pre_commit: all cases passed"
