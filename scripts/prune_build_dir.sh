#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Prunes the build directory that checkouts share (GUP-427), so that each
# commit leaves a bounded amount behind. The pre-commit hook runs it after its
# Rust checks; `mask prune-build` runs it by hand.
#
# Usage:
#   scripts/prune_build_dir.sh [--dry-run | --report] [--forget CHECKOUT]...
#                              [BUILD_DIR]
#
# BUILD_DIR defaults to cargo's build directory for this checkout (with
# scripts/cargo_env.sh, the shared one). --forget treats CHECKOUT as gone:
# `mask clean` uses it to delete one checkout's artifacts, where `cargo clean`
# would delete every checkout's. --report deletes nothing and takes no lock: it
# prints where the space is (see `report`).
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
#      GUP_PRUNE_INCREMENTAL_MB (default 8192), the least recently compiled
#      crate's (the hard bound). Measured: the shared clippy caches take 1.8
#      GB, and a checkout that has run `cargo test` 2.2 GB of its own, so the
#      default keeps about three busy checkouts warm.
#
#   5. The hard cap (GUP-428). Rules 1-4 never see units without an owner
#      record (dependencies, clippy's units, anything built before GUP-427),
#      and cargo never deletes the units a changed dependency graph or toolchain
#      leaves behind. So when the build directory (`du -sk` after `sync -f`,
#      hard links once, checkouts/ included) exceeds GUP_PRUNE_CAP_MB (default
#      81920), it evicts units, least recently used first, until it is at or
#      under the cap. A unit is one hash: its .fingerprint/ directory and the
#      files of that hash in build/, deps/, examples/ and .gup-owners/, with
#      their hard links under checkouts/ (the uplifted binaries). Its last use
#      is the newest build of it or of anything that depends on it: cargo's
#      fingerprints list each unit's dependencies, so the graph is rebuilt from
#      them (`unit_graph`), and a dependency built once and used Fresh for
#      weeks is as live as its newest dependent. (Cargo runs nothing for a
#      Fresh unit, so no stamp can record use, and atime is off.) A unit used
#      within GUP_PRUNE_KEEP_MINUTES (default 60) is never evicted, nor is one
#      in a profile whose lock could not be taken; if only those remain, it
#      says how far over the cap it is. Under the cap rule 5 costs one
#      `sync -f` and one `du -sk`.
#
# A deleted incremental cache or evicted unit costs a rebuild of it (and of
# its dependents). Cargo's freshness does not depend on the incremental cache,
# so a warm checkout stays Fresh; cargo rebuilds a unit whose output is
# missing, so an eviction never breaks a build, and the keep window and the
# graph make a live unit's eviction unlikely. Never `cargo clean`. Each profile
# directory is pruned holding its `.cargo-lock`, which cargo holds for the
# whole of a build there, so no rustc is running in it (rule 5 holds them all).
# A profile still busy after GUP_PRUNE_WAIT seconds (default 20) is skipped;
# the next prune catches up.
set -euo pipefail

dry_run=0
report=0
build=''
declare -A forget=()
while [[ $# -gt 0 ]]; do
  case $1 in
  --dry-run) dry_run=1 ;;
  --report) report=1 ;;
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
cap_kb=$((${GUP_PRUNE_INCREMENTAL_MB:-8192} * 1024))
wait_secs=${GUP_PRUNE_WAIT:-20}
total_cap_kb=$((${GUP_PRUNE_CAP_MB:-81920} * 1024))
keep_minutes=${GUP_PRUNE_KEEP_MINUTES:-60}
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

# --- Rule 5 and --report (GUP-428) -------------------------------------------

work=''
cleanup_work() { [[ -z $work ]] || rm -rf -- "$work"; }
trap cleanup_work EXIT

# disk_kb: the build directory's size on disk. `sync -f` flushes what the last
# build wrote. ZFS still allocates (and frees) blocks only at its next
# transaction group, up to 5 s later, so `du` can lag by the last few seconds
# of work; that is noise against a cap of tens of GB.
disk_kb() {
  sync -f "$build" 2>/dev/null || true
  { du -sk -- "$build" 2>/dev/null || true; } | cut -f1
}

# inventory: $work/sizes ("<profile>|<hash> <kb>", on disk, hard links once),
# $work/owned ("<profile>|<hash>") and $work/cats (the kb of the rest).
inventory() {
  { find "$build" -type f -printf '%i %n %k %p\n' 2>/dev/null || true; } | awk -v build="$build" -v work="$work" '
    BEGIN {
      hex = ""
      for (i = 0; i < 16; i++) hex = hex "[0-9a-f]"
      pre = length(build) + 2
    }
    {
      ino = $1; nl = $2; kb = $3
      rel = substr($0, length($1 $2 $3) + 4)
      rel = substr(rel, pre)
      if (substr(rel, 1, 10) == "checkouts/") {
        if (nl == 1) ckown += kb
        next
      }
      if (nl > 1) {
        if (ino in seen) next
        seen[ino] = 1
      }
      if (match(rel, /\/(incremental|\.fingerprint|build|deps|examples|\.gup-owners)\//)) {
        kind = substr(rel, RSTART + 1, RLENGTH - 2)
        profile = build "/" substr(rel, 1, RSTART - 1)
        rest = substr(rel, RSTART + RLENGTH)
        if (kind == "incremental") { incr += kb; next }
        n = index(rest, "/")
        first = n ? substr(rest, 1, n - 1) : rest
        re = "-" hex ((kind == "deps" || kind == "examples") ? "(\\.|$)" : "$")
        if (match(first, re)) {
          key = profile "|" substr(first, RSTART + 1, 16)
          size[key] += kb
          next
        }
      }
      other += kb
    }
    END {
      for (k in size) print k, size[k] > (work "/sizes")
      print "incr", incr + 0 > (work "/cats")
      print "ckown", ckown + 0 > (work "/cats")
      print "other", other + 0 > (work "/cats")
    }'
  touch "$work/sizes"
}

# unit_graph: $work/eff, one line per unit (fingerprint directory):
# "<last use, epoch seconds> <position, dependents first> <directory>", and
# $work/stats ("<dependency entries> <resolved>"). Cargo writes, for every
# unit, <kind>-<target> (its fingerprint as 8 little-endian bytes in hex) and
# <kind>-<target>.json, whose deps are [pkg_hash, name, public, fingerprint]
# with the fingerprint a decimal u64. An entry that resolves to no directory
# belongs to a unit already stale and is ignored.
# shellcheck disable=SC2016 # awk and jq programs, not shell
unit_graph() {
  local profile path hex rev i dec
  : >"$work/own"
  : >"$work/jsons"
  for profile in "${profiles[@]}"; do
    [[ -d $profile/.fingerprint ]] || continue
    find "$profile/.fingerprint" -mindepth 2 -maxdepth 2 -type f \
      -printf '%h %T@\n' 2>/dev/null >>"$work/own" || true
    find "$profile/.fingerprint" -mindepth 2 -maxdepth 2 -type f \
      -name '*.json' 2>/dev/null >>"$work/jsons" || true
  done
  # Fingerprint value (decimal) -> directory.
  sed 's/\.json$//' "$work/jsons" |
    xargs -r -d '\n' awk 'FNR == 1 { print FILENAME, $1 }' |
    while read -r path hex; do
      [[ $hex =~ ^[0-9a-f]{16}$ ]] || continue
      rev=''
      for ((i = 14; i >= 0; i -= 2)); do rev+=${hex:i:2}; done
      printf -v dec '%u' "$((16#$rev))"
      echo "$dec ${path%/*}"
    done >"$work/values"
  # Edges: dependent, dependency. A file jq cannot parse (one cargo is
  # writing) only loses its edges, which makes its dependencies look older.
  xargs -r -d '\n' jq -r 'input_filename as $f | .deps[]? | "\($f)\t\(.[3])"' \
    <"$work/jsons" 2>/dev/null |
    awk -F'\t' -v stats="$work/stats" '
      FILENAME == ARGV[1] {
        split($0, f, " ")
        m[f[1]] = m[f[1]] " " f[2]
        next
      }
      {
        n++
        if (!($2 in m)) next
        r++
        dependent = $1
        sub(/\/[^\/]*$/, "", dependent)
        k = split(m[$2], ds, " ")
        for (i = 1; i <= k; i++) print dependent, ds[i]
      }
      END { print n + 0, r + 0 > stats }' "$work/values" - >"$work/edges" || true
  [[ -s $work/stats ]] || echo "0 0" >"$work/stats"
  awk '{ print $1, $1 }' "$work/own" | sort -u >"$work/nodes"
  # tsort lists dependents first. It reports a cycle on stderr and goes on.
  cat "$work/edges" "$work/nodes" | tsort 2>/dev/null >"$work/order" || true
  awk '
    FILENAME == ARGV[1] {
      t = int($2)
      if (t > own[$1]) own[$1] = t
      next
    }
    FILENAME == ARGV[2] { adj[$1] = adj[$1] " " $2; next }
    {
      n = $1
      ord[++p] = n
      e = eff[n] + 0
      if (own[n] + 0 > e) e = own[n] + 0
      eff[n] = e
      k = split(adj[n], ds, " ")
      for (i = 1; i <= k; i++) if (e > eff[ds[i]] + 0) eff[ds[i]] = e
    }
    END { for (i = 1; i <= p; i++) print eff[ord[i]], i, ord[i] }
  ' "$work/own" "$work/edges" "$work/order" >"$work/eff"
}

# units: $work/units, "<last use> <position> <kb> <owner> <directory>" where
# owner is live (a checkout that exists), gone or none.
units() {
  local profile owner
  for profile in "${profiles[@]}"; do
    for owner in "$profile"/.gup-owners/*; do
      if gone "$owner"; then
        echo "$profile|${owner##*-} gone"
      else
        echo "$profile|${owner##*-} live"
      fi
    done
  done >"$work/owners"
  awk '
    FILENAME == ARGV[1] { kb[$1] = $2; next }
    FILENAME == ARGV[2] { own[$1] = $2; next }
    {
      dir = $3
      profile = substr(dir, 1, index(dir, "/.fingerprint/") - 1)
      key = profile "|" substr(dir, length(dir) - 15)
      print $1, $2, kb[key] + 0, (key in own ? own[key] : "none"), dir
    }
  ' "$work/sizes" "$work/owners" "$work/eff" >"$work/units"
}

# graph_work: the three steps above, in a scratch directory.
graph_work() {
  work=$(mktemp -d)
  inventory
  unit_graph
  units
}

# report: where the space is. Takes no lock and deletes nothing.
report() {
  local total
  total=$(disk_kb)
  graph_work
  awk -v now="$now" -v total="$total" -v cap="$total_cap_kb" -v build="$build" \
    -v cats="$work/cats" -v stats="$work/stats" '
    function gb(kb) { return sprintf("%.1f", kb / 1048576) }
    BEGIN {
      while ((getline line < cats) > 0) { split(line, c, " "); cat[c[1]] = c[2] }
      getline line < stats
      split(line, st, " ")
    }
    {
      kb[$4] += $3; n[$4]++
      used += $3
      age = now - $1
      b = age < 3600 ? 1 : age < 86400 ? 2 : age < 259200 ? 3 : age < 604800 ? 4 : 5
      bucket[b] += $3
    }
    END {
      printf "prune report: %s (on disk %s GB, cap %s GB)\n", build, gb(total), gb(cap)
      printf "  owned units, live checkouts        %s GB  %d units\n", gb(kb["live"]), n["live"]
      printf "  owned units, gone checkouts        %s GB  %d units\n", gb(kb["gone"]), n["gone"]
      printf "  unowned units (deps, clippy, runs) %s GB  %d units\n", gb(kb["none"]), n["none"]
      printf "  incremental caches                 %s GB\n", gb(cat["incr"])
      printf "  checkouts'"'"' own files               %s GB\n", gb(cat["ckown"])
      other = total - used - cat["incr"] - cat["ckown"]
      if (other < 0) other = 0
      printf "  other                              %s GB\n", gb(other)
      printf "  by last use: <1h %s, <1d %s, <3d %s, <7d %s, older %s GB\n", gb(bucket[1]), gb(bucket[2]), gb(bucket[3]), gb(bucket[4]), gb(bucket[5])
      printf "  edges resolved: %d of %d\n", st[2], st[1]
    }
  ' "$work/units"
  # Rule 5 as if every profile were free: oldest first, to the cap.
  sort -k1,1n -k2,2n "$work/units" |
    awk -v total="$total" -v cap="$total_cap_kb" \
      -v keep="$((now - keep_minutes * 60))" '
      total > cap && $1 < keep { total -= $3; n++; freed += $3 }
      END { printf "  rule 5 would evict: %d units, %.1f GB\n", n, freed / 1048576 }'
}

# Dependent files of a hard link: INODE -> checkouts/ paths linking it.
declare -A links=()
load_links() {
  local ino path
  [[ -d $build/checkouts ]] || return 0
  while read -r ino path; do
    links[$ino]+="$path"$'\n'
  done < <(find "$build/checkouts" -type f -links +1 -printf '%i %p\n' 2>/dev/null || true)
}

# evict DIR KB: deletes the unit whose fingerprint directory is DIR (D2),
# and its hard links under checkouts/, which would keep its disk space.
evict() {
  local dir=$1 kb=$2 profile hash p ino
  local -a files=() inos=()
  profile=${dir%/.fingerprint/*}
  hash=${dir##*-}
  freed_kb=$((freed_kb + kb))
  count[lru]=$((${count[lru]:-0} + 1))
  if [[ $dry_run -eq 1 ]]; then
    echo "prune: would delete (lru) $dir"
    return 0
  fi
  files=("$profile"/.fingerprint/*-"$hash" "$profile"/build/*-"$hash"
    "$profile"/deps/*-"$hash" "$profile"/deps/*-"$hash".*
    "$profile"/examples/*-"$hash" "$profile"/examples/*-"$hash".*
    "$profile"/.gup-owners/*-"$hash")
  [[ ${#files[@]} -gt 0 ]] || return 0
  mapfile -t inos < <(find "${files[@]}" -type f -links +1 -printf '%i\n' 2>/dev/null)
  for p in "${files[@]}"; do
    case $p in
    "$build"/?*) ;;
    *)
      echo "prune: refusing to delete $p, outside $build" >&2
      exit 1
      ;;
    esac
  done
  rm -rf -- "${files[@]}"
  for ino in "${inos[@]}"; do
    while IFS= read -r p; do
      if [[ $p == "$build"/checkouts/?* ]]; then rm -f -- "$p"; fi
    done <<<"${links[$ino]:-}"
  done
  return 0
}

# Rule 5: evict least recently used units while the build directory is over
# the cap, holding every profile's lock.
prune_lru() {
  local total profile fd eff kb dir i=0 cutoff left=0
  local -A held=()
  local -a fds=() cands=()
  [[ $build != *[[:space:]]* ]] || return 0
  # What rules 1-4 just deleted may not show in `du` yet (or, in a dry run,
  # never will): count it as gone. At worst this under-counts, so it never
  # evicts more than the cap needs.
  total=$(($(disk_kb) - freed_kb))
  [[ $total -gt $total_cap_kb ]] || return 0
  for profile in "${profiles[@]}"; do
    if [[ $dry_run -eq 1 ]]; then
      held[$profile]=1 # nothing is deleted, so nothing needs a lock
      continue
    fi
    exec {fd}<"$profile/.cargo-lock"
    if flock -w "$wait_secs" "$fd"; then
      held[$profile]=1
      fds+=("$fd")
    else
      skipped+=("${profile#"$build"/}")
      exec {fd}<&-
    fi
  done
  graph_work
  load_links
  cutoff=$((now - keep_minutes * 60))
  mapfile -t cands < <(sort -k1,1n -k2,2n "$work/units")
  # The estimate counts files only, so the directory is at or under the cap
  # by at least the directories' overhead.
  while [[ $i -lt ${#cands[@]} && $total -gt $total_cap_kb ]]; do
    read -r eff _ kb _ dir <<<"${cands[i]}"
    i=$((i + 1))
    [[ $eff -lt $cutoff ]] || continue
    [[ -n ${held[${dir%/.fingerprint/*}]:-} ]] || continue
    evict "$dir" "$kb"
    total=$((total - kb))
  done
  if [[ $total -gt $total_cap_kb ]]; then
    left=$(((total - total_cap_kb) / 1024))
    printf 'prune: still over the cap by %d MB: the rest was used in the last %d minutes or is busy\n' \
      "$left" "$keep_minutes"
  fi
  for fd in "${fds[@]}"; do exec {fd}<&-; done
}

# Profile directories: those holding a cargo lock, outside checkouts/.
mapfile -t profiles < <(find "$build" -mindepth 2 -maxdepth 3 \
  -path "$build/checkouts" -prune -o -name .cargo-lock -printf '%h\n' | sort)
if [[ $report -eq 1 ]]; then
  report
  exit 0
fi
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

prune_lru

summary=""
for kind in dead-unit dead-cache dead-checkout session idle capped lru; do
  [[ -n ${count[$kind]:-} ]] && summary+=" $kind=${count[$kind]}"
done
verb=freed
[[ $dry_run -eq 1 ]] && verb="would free"
printf 'prune: %s %d.%d MB in %s%s\n' "$verb" $((freed_kb / 1024)) \
  $((freed_kb % 1024 * 10 / 1024)) "$build" "${summary:+ (${summary# })}"
if [[ ${#skipped[@]} -gt 0 ]]; then
  echo "prune: skipped (cargo busy for ${wait_secs}s): ${skipped[*]}"
fi
