#!/bin/sh
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Cargo runs rustc for workspace members through this script
# (`build.rustc-workspace-wrapper` in .cargo/config.toml, GUP-411). It only
# execs rustc; its job is its path. Cargo names a member's artifacts by a hash
# of its path relative to the workspace root, so every checkout of this
# repository would produce the same file names, and a checkout sharing a build
# directory with another would reuse the other's artifacts whenever its own
# sources are older (freshness is by mtime). Cargo also hashes the workspace
# wrapper's path into members' artifact names, and this path is absolute and
# different in every checkout, so each checkout gets its own member artifacts
# while dependencies stay shared.
#
# Guard: a checkout nested inside another (an agent worktree under
# .claude/worktrees/) that has no .cargo/config.toml of its own, such as an
# older commit, inherits the outer checkout's config and so this script's
# path. Its members would then share the outer checkout's artifacts. Fail
# loudly instead.
if [ -n "${CARGO_MANIFEST_DIR:-}" ]; then
  root=$(cd "${0%/*}/.." && pwd -P)
  dir=$(cd "$CARGO_MANIFEST_DIR" && pwd -P)
  case $dir/ in
  "$root"/*)
    while [ "$dir" != "$root" ]; do
      if [ -e "$dir/.git" ]; then
        cat >&2 <<EOF
error: $dir is a separate checkout without its own .cargo/config.toml, so
cargo used $root's. Its workspace members would share artifacts with that
checkout (GUP-411). Bring in the commit that added .cargo/config.toml (merge or
rebase onto main), or copy .cargo/config.toml and
scripts/rustc_workspace_wrapper.sh into it, or give it its own
CARGO_BUILD_BUILD_DIR and CARGO_TARGET_DIR.
EOF
        exit 1
      fi
      dir=${dir%/*}
    done
    ;;
  esac
fi
exec "$@"
