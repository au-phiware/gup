#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Prints the cargo build environment for this checkout (GUP-411):
#
#   eval "$(scripts/cargo_env.sh [BUILD_DIR])"
#
# CARGO_BUILD_BUILD_DIR is shared by every checkout: dependencies are built
# once, and .cargo/config.toml keeps each checkout's workspace-member
# artifacts apart inside it. CARGO_TARGET_DIR is this checkout's own, under
# BUILD_DIR/checkouts/: cargo puts the final, unhashed artifacts there (the
# `target/debug/<bin>` that `cargo run` executes, example binaries, docs, the
# .wasm files), which would otherwise be overwritten by whichever checkout
# built last. On the same filesystem they are hard links into the build
# directory, so they cost almost no disk.
#
# BUILD_DIR defaults to $CARGO_BUILD_BUILD_DIR, else
# ${XDG_CACHE_HOME:-~/.cache}/gup/build. The dev shell runs this on entry
# (except in CI). An agent working in a worktree runs it there, in every
# shell, because it inherits the environment of the checkout it was started
# from. See "Sharing build output between checkouts" in CLAUDE.md.
set -euo pipefail

top=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
build=${1:-${CARGO_BUILD_BUILD_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/gup/build}}
[[ $build == /* ]] || build=$PWD/$build
hash=$(printf '%s' "$top" | sha256sum | cut -c1-8)
target=$build/checkouts/${top##*/}-$hash
printf 'export CARGO_BUILD_BUILD_DIR=%q CARGO_TARGET_DIR=%q\n' "$build" "$target"
