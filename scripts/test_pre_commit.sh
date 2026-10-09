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

# The detached dogfood crate gets its own rustfmt check.
expect 'clippy=[] dogfood=1' dogfood/src/lib.rs

# Workspace-level and unrecognised files force the full check.
expect 'mode=full' Cargo.toml
expect 'mode=full' Cargo.lock
expect 'mode=full' rust-toolchain.toml
expect 'mode=full' flake.nix
expect 'mode=full' maskfile.md
expect 'mode=full' scripts/pre_commit.sh
expect 'mode=full' scripts/generate_gallery.sh

# Workflow files get actionlint, alone or on top of the other checks.
expect 'mode=workflows' .github/workflows/tests.yml
expect 'actionlint=1' .github/workflows/tests.yml
expect 'mode=docs' .github/workflows/README.md
expect 'clippy=[gup-culling-lod] dogfood=0 actionlint=1' \
  .github/workflows/lint.yml crates/gup-culling-lod/src/lib.rs
expect 'mode=full' .github/workflows/lint.yml maskfile.md
expect 'mode=full' gup-egui/src/lib.rs
expect 'mode=full' examples/gup-tauri/src-tauri/src/main.rs
expect 'mode=full' crates/gup-core/src/lib.rs Cargo.lock
expect 'mode=full' README.md some/new/file.rs

if [[ $failures -gt 0 ]]; then
  echo "test_pre_commit: $failures case(s) failed" >&2
  exit 1
fi
echo "test_pre_commit: all cases passed"
