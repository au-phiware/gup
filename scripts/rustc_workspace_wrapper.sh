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
#
# Ownership (GUP-427): the unit this rustc invocation builds is this
# checkout's alone, so record which checkout built it, in
# <profile dir>/.gup-owners/<crate name><extra-filename> (the checkout's root,
# then the output directory), and move its incremental cache into a directory
# of this checkout's own, incremental/<checkout>-<hash>/ (named as
# scripts/cargo_env.sh names its target directory), with the root in
# .gup-checkout. scripts/prune_build_dir.sh deletes the units and caches of
# checkouts that no longer exist. Clippy runs do not come through here; their
# member artifacts are shared, and their caches are pruned by age and size.
root=$(cd "${0%/*}/.." && pwd -P)
if [ -n "${CARGO_MANIFEST_DIR:-}" ]; then
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
id=${root##*/}-$(printf '%s' "$root" | sha256sum | cut -c1-8)
crate='' extra='' outdir='' inc='' prev=''
for arg in "$@"; do
  shift
  case $prev in
  --crate-name) crate=$arg ;;
  --out-dir) outdir=$arg ;;
  -C)
    case $arg in
    extra-filename=*) extra=${arg#extra-filename=} ;;
    incremental=*)
      inc=${arg#incremental=}
      arg=incremental=$inc/$id
      ;;
    esac
    ;;
  esac
  prev=$arg
  set -- "$@" "$arg"
done
if [ -n "$inc" ] && mkdir -p "$inc/$id" 2>/dev/null; then
  printf '%s\n' "$root" >"$inc/$id/.gup-checkout" 2>/dev/null || :
fi
if [ -n "$crate" ] && [ -n "$extra" ]; then
  case $outdir in
  */deps | */examples) profile=${outdir%/*} ;;
  */build/*) profile=${outdir%/build/*} ;;
  *) profile='' ;;
  esac
  if [ -n "$profile" ] && mkdir -p "$profile/.gup-owners" 2>/dev/null; then
    printf '%s\n%s\n' "$root" "$outdir" \
      >"$profile/.gup-owners/$crate$extra" 2>/dev/null || :
  fi
fi
exec "$@"
