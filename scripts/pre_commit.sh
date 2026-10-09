#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Proportional pre-commit checks (GUP-398). `mask pre-commit` runs this and
# the git hook installed by flake.nix runs `mask pre-commit`.
#
# Usage:
#   scripts/pre_commit.sh                 # check the staged snapshot
#   scripts/pre_commit.sh --plan [PATH…]  # print the plan only
#   scripts/pre_commit.sh PATH…           # classify PATHs and check the
#                                         # working tree
#   scripts/pre_commit.sh --exec CMD…     # run CMD where the checks would
#                                         # run (for test_pre_commit.sh)
#
# What is checked (GUP-409). With no PATH, the checks see exactly what is being
# committed: the index git hands the hook ($GIT_INDEX_FILE, which is a
# temporary index for `git commit -a` or `git commit PATH…`), not the working
# tree. Unstaged edits and untracked files can neither fail a clean commit nor
# rescue a broken one.
#
#   - If the working tree matches that index (no unstaged change to a tracked
#     file, no untracked file that is not ignored), the checks run in place.
#     This is the common case and costs one `git diff` and one `git ls-files`.
#   - Otherwise the index is written to a tree and checked out into a snapshot
#     directory, `$(git rev-parse --git-dir)/gup-pre-commit/tree` (per
#     worktree, removed with it; override with GUP_PRE_COMMIT_SNAPSHOT_DIR),
#     and this script re-runs there, from the snapshot's copy. The snapshot
#     has its own index file, so `git read-tree -u` rewrites only the files
#     that changed since the last run, and untracked debris is cleaned. No
#     stash and no change to the real working tree or index, so it is safe
#     while another process edits the checkout. Git commands in the snapshot
#     see the staged index (GIT_DIR, GIT_WORK_TREE, GIT_INDEX_FILE).
#   - Cargo in the snapshot uses the checkout's target directory, so
#     dependencies stay warm. Cargo names a workspace member's artifacts by
#     its path relative to the workspace root, so the snapshot and the
#     checkout would otherwise share member artifacts and either could reuse
#     the other's (stale) result. The snapshot's .cargo/config.toml gives
#     members the opposite `incremental` setting to the checkout, which gives
#     them their own artifacts (non-incremental) and leaves dependencies
#     shared. A `build.incremental` set in a user cargo config would defeat
#     this; CARGO_INCREMENTAL is handled.
#
# `mask pre-commit` runs this script, so it checks the staged snapshot too.
# `mask all-check` run by hand (or by CI) checks the tree it runs in: your
# working tree, or CI's clean checkout. In full mode the hook runs it inside
# the snapshot when there is one.
#
# Every staged path is classified, and the most demanding class wins:
#
#   full   Anything not recognised below: the workspace manifest, Cargo.lock,
#          rust-toolchain.toml, flake.nix/flake.lock, maskfile.md, this
#          script, root files outside the gup crate's directories, the parked
#          crates outside the workspace. Runs `mask all-check`, unscoped.
#   crate  A file inside a workspace member's directory (for the root `gup`
#          crate: src/, tests/, examples/, benches/, assets/, build.rs), or a
#          file some member reads at compile time (include_str!, include_bytes!,
#          include!, #[path]), e.g. docs/tutorials/*.md or assets/fonts/*.
#          rustfmt runs on the touched crates. Clippy (default features and
#          --all-features, every target, -D warnings) runs on the touched
#          crates plus every workspace member that depends on them, since a
#          change can break a dependent. validate-marks runs when `gup` is in
#          that set.
#   workflows
#          .github/workflows/* (except Markdown): actionlint on every workflow
#          file, and no Rust check. Scoped rather than full (GUP-409): a
#          workflow cannot change what the Rust checks see, and CI runs the
#          workflow itself.
#   docs   *.md, docs/**, COPYING, LICENSE*: no Rust check.
#   dogfood
#          dogfood/** (a detached crate): its rustfmt check.
#
# Outside full mode, these always run: the trailing-whitespace check on every
# .rs file and the gallery sync check (both repo-wide and cheap), and
# prettier and mdl on the changed Markdown files. nixfmt and statix need not:
# a flake.nix change forces full mode.
#
# The scoping is a local-hook optimisation only. CI never scopes: every
# workflow runs its full checks on every push.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

plan_only=0
exec_cmd=()
if [[ ${1:-} == --plan ]]; then
  plan_only=1
  shift
elif [[ ${1:-} == --exec ]]; then
  shift
  exec_cmd=("$@")
  set --
fi

# True when the working tree differs from the index being committed. An error
# counts as a difference: the snapshot is always correct, only slower.
worktree_differs() {
  ! git diff --quiet --no-ext-diff 2>/dev/null && return 0
  local untracked
  untracked=$(git ls-files --others --exclude-standard --directory \
    --no-empty-directory 2>/dev/null | head -n 1) || return 0
  [[ -n $untracked ]]
}

# Check the staged snapshot rather than the working tree (see the header).
snapshot=0
if [[ $plan_only -eq 0 && $# -eq 0 && -z ${GUP_PRE_COMMIT_IN_SNAPSHOT:-} ]]; then
  if worktree_differs; then
    snapshot=1
  else
    echo "pre-commit: checking the working tree (it matches the index)"
  fi
fi
if [[ $snapshot -eq 1 ]]; then
  gitdir=$(git rev-parse --absolute-git-dir)
  snap=${GUP_PRE_COMMIT_SNAPSHOT_DIR:-$gitdir/gup-pre-commit}
  mkdir -p "$snap/tree" "$snap/.cargo"
  snap=$(cd "$snap" && pwd -P)
  if command -v flock >/dev/null; then
    exec 9>"$snap/lock"
    flock 9
  fi
  # Copy the index so write-tree's cache-tree update cannot touch git's own
  # (possibly locked) index file.
  index=${GIT_INDEX_FILE:-$(git rev-parse --git-path index)}
  cp "$index" "$snap/staged-index"
  tree=$(GIT_INDEX_FILE=$snap/staged-index git write-tree)
  snapgit() {
    GIT_DIR=$gitdir GIT_WORK_TREE=$snap/tree GIT_INDEX_FILE=$snap/index \
      git -C "$snap/tree" "$@"
  }
  # Never clean anything but the snapshot.
  if [[ $(snapgit rev-parse --show-toplevel) != "$snap/tree" ]]; then
    echo "pre-commit: snapshot $snap/tree is not its own work tree" >&2
    exit 1
  fi
  snapgit read-tree --reset -u "$tree"
  snapgit clean -ffdq

  target=${CARGO_TARGET_DIR:-$(cargo metadata --no-deps --format-version 1 \
    2>/dev/null | jq -r .target_directory || true)}
  [[ -n $target && $target != null ]] || target=$(pwd -P)/target
  if [[ ${CARGO_INCREMENTAL:-1} == 0 ]]; then
    members=true others=false
  else
    members=false others=true
  fi
  cat >"$snap/.cargo/config.toml" <<EOF
# Written by scripts/pre_commit.sh on every run (GUP-409). Gives workspace
# members their own artifacts in the shared target directory; dependencies
# (package "*") keep the checkout's setting and stay shared.
[profile.dev]
incremental = $members
[profile.dev.package."*"]
incremental = $others
EOF
  echo "pre-commit: checking the staged snapshot in $snap/tree" \
    "(the working tree differs from it)"
  export GIT_DIR=$gitdir GIT_WORK_TREE=$snap/tree GIT_INDEX_FILE=$snap/index
  export CARGO_TARGET_DIR=$target GUP_PRE_COMMIT_IN_SNAPSHOT=1
  unset CARGO_INCREMENTAL
  cd "$snap/tree"
  if [[ ${#exec_cmd[@]} -gt 0 ]]; then
    exec "${exec_cmd[@]}"
  fi
  exec ./scripts/pre_commit.sh
fi
if [[ ${#exec_cmd[@]} -gt 0 ]]; then
  exec "${exec_cmd[@]}"
fi

if [[ $# -gt 0 ]]; then
  paths=("$@")
else
  mapfile -t paths < <(git diff --cached --name-only --no-renames)
fi

# Workspace members: name and directory relative to the repository root.
declare -A member_dir=() dir_member=()
root_member=""
metadata=$(cargo metadata --no-deps --format-version 1 2>/dev/null || true)
full_reasons=()
if [[ -z $metadata ]]; then
  full_reasons+=("cargo metadata failed")
else
  top=$(pwd -P)
  while IFS=$'\t' read -r name manifest; do
    dir=$(dirname "$manifest")
    rel=${dir#"$top"}
    rel=${rel#/}
    [[ -z $rel ]] && rel=. && root_member=$name
    member_dir[$name]=$rel
    dir_member[$rel]=$name
  done < <(jq -r '.workspace_members as $m | .packages[]
      | select(.id as $id | $m | index($id)) | [.name, .manifest_path] | @tsv' \
    <<<"$metadata")
fi

# Reverse dependencies between members (path dependencies only).
declare -A rdeps=()
if [[ -n $metadata ]]; then
  while IFS=$'\t' read -r name dep; do
    [[ -n ${member_dir[$dep]:-} ]] && rdeps[$dep]+=" $name"
  done < <(jq -r '.workspace_members as $m | .packages[]
      | select(.id as $id | $m | index($id)) | .name as $n
      | .dependencies[] | select(.path != null) | [$n, .name] | @tsv' \
    <<<"$metadata")
fi

# The member owning a directory: the nearest ancestor that is a member
# directory. Prints nothing for the root crate's directory or above.
owning_member_dir() {
  local d=$1
  while [[ $d != . && $d != / ]]; do
    if [[ -n ${dir_member[$d]:-} ]]; then
      echo "$d"
      return
    fi
    d=$(dirname "$d")
  done
}

# The nearest ancestor below the root with a Cargo.toml that is not a member.
foreign_manifest_dir() {
  local d=$1
  while [[ $d != . && $d != / ]]; do
    if [[ -f $d/Cargo.toml && -z ${dir_member[$d]:-} ]]; then
      echo "$d"
      return
    fi
    d=$(dirname "$d")
  done
}

# Files read at compile time, mapped to the member that reads them.
declare -A included_by=()
while IFS=: read -r file _ rest; do
  while [[ $rest =~ (include(_str|_bytes)?!\(\"([^\"]+)\"|#\[path\ =\ \"([^\"]+)\") ]]; do
    rel=${BASH_REMATCH[3]:-${BASH_REMATCH[4]}}
    rest=${rest#*"${BASH_REMATCH[1]}"}
    target=$(realpath -m --relative-to=. "$(dirname "$file")/$rel")
    owner_dir=$(owning_member_dir "$(dirname "$file")")
    owner=${dir_member[${owner_dir:-.}]:-}
    if [[ -z $owner ]]; then
      # Not inside a member directory (dogfood, a parked crate): no member
      # reads it, so the plain path rules below apply.
      continue
    fi
    included_by[$target]+=" $owner"
  done
done < <(git grep -nE 'include(_str|_bytes)?!\("|#\[path = "' -- '*.rs' || true)

is_doc() {
  case $1 in
  *.md | docs/* | COPYING | LICENSE*) return 0 ;;
  *) return 1 ;;
  esac
}

declare -A touched=()
docs=0
dogfood=0
workflows=0
for p in "${paths[@]}"; do
  case $p in
  Cargo.toml | Cargo.lock | rust-toolchain.toml | flake.nix | flake.lock | \
    maskfile.md | scripts/pre_commit.sh | scripts/test_pre_commit.sh | \
    .cargo/* | clippy.toml | .clippy.toml | rustfmt.toml | .rustfmt.toml)
    full_reasons+=("$p is workspace-level")
    continue
    ;;
  esac
  if [[ -n ${included_by[$p]:-} ]]; then
    for m in ${included_by[$p]}; do touched[$m]=1; done
    continue
  fi
  if is_doc "$p"; then
    docs=1
    continue
  fi
  case $p in
  .github/workflows/*)
    workflows=1
    continue
    ;;
  dogfood/*)
    dogfood=1
    continue
    ;;
  esac
  d=$(owning_member_dir "$(dirname "$p")")
  if [[ -n $d ]]; then
    touched[${dir_member[$d]}]=1
    continue
  fi
  if [[ -n $root_member ]]; then
    case $p in
    src/* | tests/* | examples/* | benches/* | assets/* | build.rs)
      # A crate outside the workspace can live inside these directories
      # (the parked Tauri app is under examples/).
      if [[ -n $(foreign_manifest_dir "$(dirname "$p")") ]]; then
        full_reasons+=("$p is in a crate outside the workspace")
      else
        touched[$root_member]=1
      fi
      continue
      ;;
    esac
  fi
  full_reasons+=("$p is outside every workspace crate")
done

# Close the touched set over reverse dependencies.
declare -A lint=()
queue=("${!touched[@]}")
while [[ ${#queue[@]} -gt 0 ]]; do
  m=${queue[0]}
  queue=("${queue[@]:1}")
  [[ -n ${lint[$m]:-} ]] && continue
  lint[$m]=1
  for r in ${rdeps[$m]:-}; do queue+=("$r"); done
done

if [[ ${#full_reasons[@]} -gt 0 ]]; then
  mode=full
elif [[ ${#lint[@]} -gt 0 || $dogfood -eq 1 ]]; then
  mode=scoped
elif [[ $workflows -eq 1 ]]; then
  mode=workflows
elif [[ $docs -eq 1 ]]; then
  mode=docs
else
  mode=none
fi

sorted() { printf '%s\n' "$@" | sort | tr '\n' ' ' | sed 's/ $//'; }
fmt_crates=$(sorted "${!touched[@]}")
lint_crates=$(sorted "${!lint[@]}")
echo "pre-commit: mode=$mode"
if [[ $mode == full ]]; then
  printf 'pre-commit: full because %s\n' "${full_reasons[@]}"
fi
if [[ $mode == scoped ]]; then
  echo "pre-commit: fmt=[$fmt_crates] clippy=[$lint_crates] dogfood=$dogfood actionlint=$workflows"
elif [[ $mode == workflows ]]; then
  echo "pre-commit: actionlint=1"
fi
[[ $plan_only -eq 1 ]] && exit 0

if [[ $mode == full ]]; then
  exec mask all-check
fi

checks=(
  'git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" -- "*.rs"; test $? -eq 1'
  './scripts/check_gallery_sync.sh'
)
names='\s,gallery'

# Markdown checks on the changed Markdown files (a markdown config change is
# outside every crate, so it already forced the full, repo-wide check).
md=()
for p in "${paths[@]}"; do
  [[ $p == *.md && -f $p ]] && md+=("$(printf '%q' "$p")")
done
if [[ ${#md[@]} -gt 0 ]]; then
  checks+=("prettier --log-level warn --check ${md[*]} && mdl ${md[*]}")
  names+=',md'
fi

# actionlint checks every workflow file (well under a second), not only the
# staged ones: a workflow can break by referring to another.
if [[ $workflows -eq 1 ]]; then
  checks+=('actionlint')
  names+=',gha'
fi

if [[ $mode == scoped ]]; then
  rust=()
  pkgs=()
  for m in ${lint_crates}; do pkgs+=(-p "$m"); done
  fmt_pkgs=()
  for m in ${fmt_crates}; do fmt_pkgs+=(-p "$m"); done
  if [[ ${#fmt_pkgs[@]} -gt 0 ]]; then
    rust+=("cargo fmt ${fmt_pkgs[*]} -- --check")
  fi
  if [[ $dogfood -eq 1 ]]; then
    rust+=("cargo fmt --manifest-path dogfood/Cargo.toml -- --check")
  fi
  if [[ ${#pkgs[@]} -gt 0 ]]; then
    rust+=("cargo clippy ${pkgs[*]} --all-targets -- -D warnings")
    rust+=("cargo clippy ${pkgs[*]} --all-targets --all-features -- -D warnings")
  fi
  if [[ ${#rust[@]} -gt 0 ]]; then
    joined=$(printf ' && %s' "${rust[@]}")
    checks+=("${joined# && }")
    names+=',rs'
  fi
  if [[ -n ${lint[$root_member]:-} ]]; then
    checks+=('mask validate-marks')
    names+=',marks'
  fi
fi

exec concurrently --group --names "$names" "${checks[@]}"
