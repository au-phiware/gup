#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Prunes the build directory that checkouts share (GUP-427), so that each
# commit leaves a bounded amount behind. The pre-commit hook runs it after its
# Rust checks; `mask prune-build` runs it by hand.
#
# Usage:
#   scripts/prune_build_dir.sh [--dry-run] [--forget CHECKOUT]... [BUILD_DIR]
#
# BUILD_DIR defaults to cargo's build directory for this checkout (with
# scripts/cargo_env.sh, the shared one). --forget treats CHECKOUT as gone:
# `mask clean` uses it to delete one checkout's artifacts, where `cargo clean`
# would delete every checkout's.
#
# What it deletes, and why that is safe:
#
#   1. What checkouts that no longer exist built. Workspace members are built
#      per checkout (GUP-411), and scripts/rustc_workspace_wrapper.sh records
#      which checkout built each unit (<profile>/.gup-owners/) and keeps each
#      checkout's incremental caches apart (<profile>/incremental/<checkout>/).
#      Once that checkout's directory is gone (a removed agent worktree, or
#      its pre-commit snapshot, which lives in its git directory), nothing can
#      use them: the units' files in deps/, examples/ or build/, their
#      .fingerprint/ directories, and the checkout's incremental caches go.
#      So does its target directory, BUILD_DIR/checkouts/<checkout>
#      (scripts/cargo_env.sh).
#   2. Superseded incremental sessions. rustc compiles a crate from its newest
#      session and deletes the older ones only when it next compiles that
#      crate, so between compiles every crate holds two (measured: the root
#      crate's clippy caches, 1.7 GB, twice). Every finalized session but the
#      newest goes, and so does a `-working` one (a crashed rustc's) older
#      than a day.
#   3. Idle incremental caches: a crate's whole cache when its newest session
#      is older than GUP_PRUNE_IDLE_DAYS (default 3).
#   4. Then, while a profile's incremental caches exceed
#      GUP_PRUNE_INCREMENTAL_MB (default 6144), the least recently compiled
#      crate's (the hard bound).
#
# A deleted incremental cache costs one non-incremental compile of its crate.
# Cargo's freshness does not depend on it, so a warm checkout stays Fresh.
# Never `cargo clean`, and nothing else a live checkout uses: dependencies and
# clippy's shared member artifacts are not touched. Each profile directory is
# pruned holding its `.cargo-lock`, which cargo holds for the whole of a build
# there, so no rustc is running in it. A profile still busy after
# GUP_PRUNE_WAIT seconds (default 20) is skipped; the next prune catches up.
set -euo pipefail

dry_run=0
build=''
declare -A forget=()
while [[ $# -gt 0 ]]; do
  case $1 in
  --dry-run) dry_run=1 ;;
  --forget)
    forget[$(realpath -m -- "$2")]=1
    shift
    ;;
  *) build=$1 ;;
  esac
  shift
done

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ -z $build ]]; then
  build=$(cargo metadata --no-deps --format-version 1 2>/dev/null |
    jq -r '.build_directory // .target_directory // empty' || true)
fi
if [[ -z $build || ! -d $build ]]; then
  echo "prune: no build directory${build:+ at $build}; nothing to do"
  exit 0
fi
build=$(cd "$build" && pwd -P)
if ! command -v flock >/dev/null; then
  echo "prune: flock not found; not pruning without cargo's lock" >&2
  exit 0
fi

idle_days=${GUP_PRUNE_IDLE_DAYS:-3}
cap_kb=$((${GUP_PRUNE_INCREMENTAL_MB:-6144} * 1024))
wait_secs=${GUP_PRUNE_WAIT:-20}
now=$(date +%s)
idle_before=$((now - idle_days * 86400))
working_before=$((now - 86400))

freed_kb=0
declare -A count=()
skipped=()
shopt -s nullglob

# remove KIND PATH...: deletes PATHs (inside the build directory only) and
# counts what they used.
remove() {
  local kind=$1 p kb
  shift
  for p in "$@"; do
    [[ -e $p || -L $p ]] || continue
    case $p in
    "$build"/?*) ;;
    *)
      echo "prune: refusing to delete $p, outside $build" >&2
      exit 1
      ;;
    esac
    kb=$(du -sk -- "$p" | cut -f1)
    freed_kb=$((freed_kb + kb))
    count[$kind]=$((${count[$kind]:-0} + 1))
    if [[ $dry_run -eq 1 ]]; then
      echo "prune: would delete ($kind) $p"
    else
      rm -rf -- "$p"
    fi
  done
}

# gone MARKER: true when MARKER names a checkout (its first line) that no
# longer exists, or that --forget names.
gone() {
  local root
  root=$(head -n 1 -- "$1" 2>/dev/null) || return 1
  [[ $root == /* ]] || return 1
  [[ ! -e $root || -n ${forget[$root]:-} ]]
}

# Rule 1 for one profile directory.
prune_dead() {
  local profile=$1 owner root outdir unit hash checkout
  for owner in "$profile"/.gup-owners/*; do
    gone "$owner" || continue
    { read -r root && read -r outdir; } <"$owner" || continue
    unit=${owner##*/}
    hash=${unit##*-}
    case $outdir in
    "$profile"/build/?*) remove dead-unit "$outdir" ;;
    "$profile"/deps | "$profile"/examples)
      remove dead-unit "$outdir/$unit" "$outdir/$unit".* "$outdir/lib$unit".*
      ;;
    esac
    if [[ $hash != "$unit" && -n $hash ]]; then
      remove dead-unit "$profile"/.fingerprint/*-"$hash"
    fi
    [[ $dry_run -eq 1 ]] || rm -f -- "$owner"
  done
  for checkout in "$profile"/incremental/*/.gup-checkout; do
    gone "$checkout" && remove dead-cache "${checkout%/*}"
  done
  return 0
}

# The session's lock file: s-<time>-<random>.lock beside s-<time>-<random>-<x>.
session_lock() { echo "${1%-*}.lock"; }

# Rules 2-4 for one profile's incremental directory. Crate caches are
# incremental/<crate>-<id>/ (clippy's, shared) and
# incremental/<checkout>/<crate>-<id>/ (a checkout's own).
prune_incremental() {
  local inc=$1 crate mtime path line latest kb total=0
  [[ -d $inc ]] || return 0
  local -A last=() keep=() keep_mtime=()
  local -a sessions=() crates=() by_age=()
  mapfile -t sessions < <(find "$inc" -mindepth 2 -maxdepth 3 -type d \
    -name 's-*-*' -printf '%T@ %p\n')
  # The latest activity per crate, and its newest finalized session.
  for line in "${sessions[@]}"; do
    mtime=${line%% *}
    mtime=${mtime%.*}
    path=${line#* }
    crate=${path%/*}
    [[ $mtime -gt ${last[$crate]:-0} ]] && last[$crate]=$mtime
    if [[ $path != *-working && $mtime -ge ${keep_mtime[$crate]:-0} ]]; then
      keep[$crate]=$path
      keep_mtime[$crate]=$mtime
    fi
  done
  # Rule 2.
  for line in "${sessions[@]}"; do
    mtime=${line%% *}
    mtime=${mtime%.*}
    path=${line#* }
    crate=${path%/*}
    if [[ $path == *-working ]]; then
      [[ $mtime -lt $working_before ]] || continue
    elif [[ $path == "${keep[$crate]:-}" ]]; then
      continue
    fi
    remove session "$path" "$(session_lock "$path")"
  done
  # Rule 3, then rule 4 (least recently compiled first).
  for path in "$inc"/*/; do
    path=${path%/}
    if [[ -f $path/.gup-checkout ]]; then
      for crate in "$path"/*/; do crates+=("${crate%/}"); done
    else
      crates+=("$path")
    fi
  done
  for crate in "${crates[@]}"; do
    latest=${last[$crate]:-$(stat -c %Y -- "$crate")}
    if [[ $latest -lt $idle_before ]]; then
      remove idle "$crate"
      continue
    fi
    kb=$(du -sk -- "$crate" | cut -f1)
    total=$((total + kb))
    by_age+=("$latest $kb $crate")
  done
  [[ $total -gt $cap_kb ]] || return 0
  while IFS=' ' read -r latest kb crate; do
    [[ $total -le $cap_kb ]] && break
    remove capped "$crate"
    total=$((total - kb))
  done < <(printf '%s\n' "${by_age[@]}" | sort -n)
}

# Profile directories: those holding a cargo lock, outside checkouts/.
mapfile -t profiles < <(find "$build" -mindepth 2 -maxdepth 3 \
  -path "$build/checkouts" -prune -o -name .cargo-lock -printf '%h\n' | sort)
for profile in "${profiles[@]}"; do
  exec {lock}<"$profile/.cargo-lock"
  if flock -w "$wait_secs" "$lock"; then
    prune_dead "$profile"
    prune_incremental "$profile/incremental"
  else
    skipped+=("${profile#"$build"/}")
  fi
  exec {lock}<&-
done

# Rule 1: the target directories of checkouts that are gone. Nothing builds
# into one once its checkout is gone, so it needs no lock.
for marker in "$build"/checkouts/*/.gup-checkout; do
  gone "$marker" && remove dead-checkout "${marker%/*}"
done

summary=""
for kind in dead-unit dead-cache dead-checkout session idle capped; do
  [[ -n ${count[$kind]:-} ]] && summary+=" $kind=${count[$kind]}"
done
verb=freed
[[ $dry_run -eq 1 ]] && verb="would free"
printf 'prune: %s %d.%d MB in %s%s\n' "$verb" $((freed_kb / 1024)) \
  $((freed_kb % 1024 * 10 / 1024)) "$build" "${summary:+ (${summary# })}"
if [[ ${#skipped[@]} -gt 0 ]]; then
  echo "prune: skipped (cargo busy for ${wait_secs}s): ${skipped[*]}"
fi
