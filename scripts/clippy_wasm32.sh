#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Strict clippy on the wasm32 target (GUP-412). The host clippy runs never see
# `#[cfg(target_arch = "wasm32")]` code, and wasm32 changes what some lints
# see: wgpu's handles are not `Send`/`Sync` there.
#
# Usage:
#   scripts/clippy_wasm32.sh                    # every wasm32 member
#   scripts/clippy_wasm32.sh CRATE...           # those of CRATE... that are
#                                               # wasm32 members
#   scripts/clippy_wasm32.sh --list [CRATE...]  # print the crates only
#
# `mask all-check` (and so CI's Lint workflow) runs it with no arguments; the
# pre-commit hook runs it on the crates it lints (scripts/pre_commit.sh).
#
# What is linted, and why:
#
#   - The lib of every workspace member except those listed in `excluded`
#     below, so a new member is linted on wasm32 unless someone says why not.
#   - The lib only. The test, example and bench targets run on the host: gup's
#     and gup-core's do not even build for wasm32 (blocking APIs, criterion,
#     tokio's runtime; GUP-285B), and the rest would only repeat the host lint.
#   - Default features and all features, like the host runs
#     (`--all-features` builds on wasm32 too).
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# Members that are not linted on wasm32, and why.
declare -A excluded=(
  [gup-macros]="a proc-macro, which always compiles for the host"
  [gup-culling-lod]="the quarantined parts bin (GUP-390): nothing builds it for wasm32, and code ported from it into gup-core is linted there"
)

list=0
if [[ ${1:-} == --list ]]; then
  list=1
  shift
fi

if [[ $# -gt 0 ]]; then
  candidates=("$@")
else
  mapfile -t candidates < <(cargo metadata --no-deps --format-version 1 |
    jq -r '.workspace_members as $m | .packages[]
      | select(.id as $id | $m | index($id)) | .name' | sort)
fi

crates=()
for c in "${candidates[@]}"; do
  [[ -n ${excluded[$c]:-} ]] || crates+=("$c")
done

if [[ $list -eq 1 ]]; then
  echo "${crates[*]}"
  exit 0
fi
if [[ ${#crates[@]} -eq 0 ]]; then
  echo "clippy_wasm32: no wasm32 crate to lint"
  exit 0
fi

pkgs=()
for c in "${crates[@]}"; do pkgs+=(-p "$c"); done
set -x
cargo clippy --target wasm32-unknown-unknown "${pkgs[@]}" --lib -- -D warnings
cargo clippy --target wasm32-unknown-unknown "${pkgs[@]}" --lib --all-features \
  -- -D warnings
