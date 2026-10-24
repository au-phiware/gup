#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Work-in-progress snapshots of the whole working tree (GUP-496).
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/checkpoint.sh [save [-m MESSAGE]]
       scripts/checkpoint.sh restore [--force] [BRANCH]
       scripts/checkpoint.sh list
       scripts/checkpoint.sh drop [BRANCH]

save     Snapshot the working tree as `git add -A` sees it (tracked edits,
         staged changes, deletions, untracked files; not ignored files) into a
         commit on refs/checkpoints/<branch>. Plumbing only: no hook runs, and
         HEAD, the branch, the real index and the stash are untouched. Nothing
         is sent to any remote. It never replaces a real commit.
restore  Make the working tree match refs/checkpoints/<BRANCH> (default: the
         current branch): write its files and delete other tracked or untracked
         non-ignored files. The staging area is not restored, only files.
         Refuses when the checkpoint is not based on HEAD, or when the working
         tree has changes the checkpoint lacks, unless --force, which first
         saves the current state to refs/checkpoints-backup/<BRANCH>.
list     One line per checkpoint: name, sha, committer date, base.
drop     Delete refs/checkpoints/<BRANCH> and refs/checkpoints-backup/<BRANCH>.
USAGE
}

case ${1:-} in
  -h | --help | help)
    usage
    exit 0
    ;;
esac

cd "$(git rev-parse --show-toplevel)"

current_branch() {
  git symbolic-ref --quiet --short HEAD || {
    echo "checkpoint: needs a branch (git switch -c <name>)" >&2
    exit 1
  }
}

# Print the tree of the working tree (as `git add -A` sees it), using a
# temporary copy of the index so the real one is never written.
snap_tree() (
  local real tmp
  real=$(git rev-parse --git-path index)
  [[ $real = /* ]] || real=$PWD/$real
  tmp=$(mktemp)
  trap 'rm -f "$tmp"' EXIT
  if [[ -e $real ]]; then cp "$real" "$tmp"; else rm -f "$tmp"; fi
  GIT_INDEX_FILE=$tmp git add -A >/dev/null
  GIT_INDEX_FILE=$tmp git write-tree
)

short() { git rev-parse --short "$1"; }

# make_commit TREE REF BRANCH MESSAGE: prints the new commit. HEAD is the first
# parent, the previous commit on REF (if any) the second.
make_commit() {
  local tree=$1 ref=$2 branch=$3 msg=$4 old
  local parents=(-p HEAD) args
  if old=$(git rev-parse --verify --quiet "$ref^{commit}"); then
    parents+=(-p "$old")
  fi
  args=(-m "checkpoint $branch $(date -Iseconds)")
  [[ -z $msg ]] || args+=(-m "$msg")
  git commit-tree --no-gpg-sign "${parents[@]}" "${args[@]}" "$tree"
}

# write_ref REF COMMIT: compare-and-swap on the old value.
write_ref() {
  local ref=$1 commit=$2 old
  if old=$(git rev-parse --verify --quiet "$ref"); then
    git update-ref "$ref" "$commit" "$old"
  else
    git update-ref "$ref" "$commit" ""
  fi
}

cmd_save() {
  local msg=
  while [[ $# -gt 0 ]]; do
    case $1 in
      -m | --message)
        msg=${2:?checkpoint: -m needs a message}
        shift 2
        ;;
      *)
        usage >&2
        exit 2
        ;;
    esac
  done
  local branch ref tree old commit
  branch=$(current_branch)
  ref=refs/checkpoints/$branch
  tree=$(snap_tree)
  if old=$(git rev-parse --verify --quiet "$ref^{tree}"); then
    if [[ $tree = "$old" ]]; then
      echo "unchanged: $ref $(short "$ref")"
      return 0
    fi
  elif [[ $tree = "$(git rev-parse 'HEAD^{tree}')" ]]; then
    echo "clean: nothing to checkpoint"
    return 0
  fi
  commit=$(make_commit "$tree" "$ref" "$branch" "$msg")
  write_ref "$ref" "$commit"
  echo "checkpoint $(short "$commit") -> $ref"
}

cmd_restore() {
  local force=0 branch=
  while [[ $# -gt 0 ]]; do
    case $1 in
      --force)
        force=1
        shift
        ;;
      -*)
        usage >&2
        exit 2
        ;;
      *)
        branch=$1
        shift
        ;;
    esac
  done
  [[ -n $branch ]] || branch=$(current_branch)
  local ref=refs/checkpoints/$branch commit base head cur_tree want_tree
  commit=$(git rev-parse --verify --quiet "$ref^{commit}") || {
    echo "checkpoint: no $ref" >&2
    exit 1
  }
  want_tree=$(git rev-parse "$commit^{tree}")
  head=$(git rev-parse HEAD)
  base=$(git rev-parse "$commit^1")
  cur_tree=$(snap_tree)
  if [[ $cur_tree = "$want_tree" ]]; then
    echo "already at $(short "$commit")"
    return 0
  fi
  if [[ $force -eq 0 ]]; then
    if [[ $base != "$head" ]]; then
      echo "checkpoint: $ref is based on $(short "$base"), but HEAD is $(short "$head"); use --force" >&2
      exit 1
    fi
    if [[ $cur_tree != "$(git rev-parse 'HEAD^{tree}')" ]]; then
      echo "checkpoint: the working tree has changes that differ from $(short "$commit"); use --force to back them up and restore" >&2
      exit 1
    fi
  elif [[ $cur_tree != "$(git rev-parse 'HEAD^{tree}')" ]]; then
    local bref=refs/checkpoints-backup/$branch bcommit
    bcommit=$(make_commit "$cur_tree" "$bref" "$branch" "backup before restore")
    write_ref "$bref" "$bcommit"
    echo "backup $(short "$bcommit") -> $bref"
  fi

  # Write the checkpoint's files through a temporary index; the real index is
  # never written.
  local tmp keep f
  tmp=$(mktemp -u)
  GIT_INDEX_FILE=$tmp git read-tree "$commit"
  GIT_INDEX_FILE=$tmp git checkout-index -a -f
  rm -f "$tmp"

  # Delete what the checkpoint does not have.
  keep=$(mktemp)
  git ls-tree -r -z --name-only "$commit" >"$keep"
  while IFS= read -r -d '' f; do
    if ! grep -qzxF -- "$f" "$keep"; then
      rm -f -- "$f"
    fi
  done < <(git ls-files -z -co --exclude-standard)
  rm -f "$keep"

  if [[ $(snap_tree) != "$want_tree" ]]; then
    echo "checkpoint: restore verification failed: working tree differs from $(short "$commit")" >&2
    exit 1
  fi
  echo "restored $(short "$commit") <- $ref"
}

cmd_list() {
  local name sha date parent
  while IFS=$'\t' read -r name sha date parent; do
    printf '%s\t%s\t%s\t%s\n' "$name" "$sha" "$date" "$(short "${parent%% *}")"
  done < <(git for-each-ref \
    --format='%(refname:lstrip=2)%09%(objectname:short)%09%(committerdate:iso-strict)%09%(parent)' \
    refs/checkpoints/)
}

cmd_drop() {
  local branch=${1:-} ref
  [[ -n $branch ]] || branch=$(current_branch)
  for ref in refs/checkpoints/$branch refs/checkpoints-backup/$branch; do
    if git rev-parse --verify --quiet "$ref" >/dev/null; then
      git update-ref -d "$ref"
      echo "dropped $ref"
    fi
  done
}

sub=${1:-save}
[[ $# -eq 0 ]] || shift
case $sub in
  save) cmd_save "$@" ;;
  restore) cmd_restore "$@" ;;
  list) cmd_list ;;
  drop) cmd_drop "$@" ;;
  *)
    usage >&2
    exit 2
    ;;
esac
