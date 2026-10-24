#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Tests for scripts/checkpoint.sh (GUP-496), in a throwaway repository.
set -euo pipefail

src=$(git rev-parse --show-toplevel)/scripts/checkpoint.sh

# The hook (and a full-mode snapshot run) exports these; they would point git
# at the real repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR \
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.com
export GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.com

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fails=0
check() {
  local msg=$1
  shift
  if ! "$@" >/dev/null 2>&1; then
    echo "FAIL: $msg" >&2
    fails=$((fails + 1))
  fi
}
eq() { [[ $1 = "$2" ]]; }

git init -q --bare "$tmp/remote.git"
repo=$tmp/repo
git init -q -b main "$repo"
cd "$repo"
git remote add origin "$tmp/remote.git"
mkdir -p scripts sub
cp "$src" scripts/checkpoint.sh
printf 'ignored.log\n' >.gitignore
printf 'one\n' >mod.txt
printf 'two\n' >gone.txt
printf 'three\n' >sub/keep.txt
git add -A
git commit -qm init
git push -q origin main

# Hooks that leave a marker, installed after the initial commit.
for h in pre-commit post-commit; do
  printf '#!/bin/sh\ntouch "%s/marker-%s"\n' "$tmp" "$h" >".git/hooks/$h"
  chmod +x ".git/hooks/$h"
done

# Working tree: modified, staged new, deleted, untracked, ignored.
printf 'one changed\n' >mod.txt
printf 'staged\n' >staged.txt
git add staged.txt
rm gone.txt
printf 'untracked\n' >sub/untracked.txt
printf 'ignored\n' >ignored.log

head0=$(git rev-parse HEAD)
sha_index() { sha256sum .git/index | cut -d' ' -f1; }
index0=$(sha_index)

out=$(scripts/checkpoint.sh save)
ref=refs/checkpoints/main
c1=$(git rev-parse "$ref")
check "save prints the checkpoint line" eq "$out" "checkpoint $(git rev-parse --short "$c1") -> $ref"
show() { git show "$ref:$1"; }
check "modified file content" eq "$(show mod.txt)" "one changed"
check "staged new file content" eq "$(show staged.txt)" "staged"
check "untracked file content" eq "$(show sub/untracked.txt)" "untracked"
check "unchanged file content" eq "$(show sub/keep.txt)" "three"
check "deleted file absent" bash -c '! git cat-file -e "$0:gone.txt"' "$ref"
check "ignored file absent" bash -c '! git cat-file -e "$0:ignored.log"' "$ref"
check "HEAD unchanged" eq "$(git rev-parse HEAD)" "$head0"
check "branch unchanged" eq "$(git rev-parse refs/heads/main)" "$head0"
check "index unchanged" eq "$(sha_index)" "$index0"
check "stash list empty" eq "$(git stash list)" ""
check "first parent is HEAD" eq "$(git rev-parse "$c1^1")" "$head0"
check "pre-commit hook did not run" test ! -e "$tmp/marker-pre-commit"
check "post-commit hook did not run" test ! -e "$tmp/marker-post-commit"
check "remote has no checkpoints" eq "$(git ls-remote "$tmp/remote.git" 'refs/checkpoints*')" ""
git push -q origin main
check "remote has no checkpoints after a push" eq "$(git ls-remote "$tmp/remote.git" 'refs/checkpoints*')" ""

out=$(scripts/checkpoint.sh save)
check "second save is unchanged" eq "$out" "unchanged: $ref $(git rev-parse --short "$c1")"
check "ref did not move" eq "$(git rev-parse "$ref")" "$c1"

check "list names the checkpoint" bash -c 'scripts/checkpoint.sh list | grep -q "^main	"'

# Restore.
expected_files=$(git ls-files -co --exclude-standard | sort)
sums() { git ls-files -co --exclude-standard | sort | while read -r f; do [[ -e $f ]] && sha256sum "$f"; done; true; }
expected_sums=$(sums)
printf 'changed after\n' >mod.txt
rm sub/untracked.txt
printf 'late\n' >late.txt
check "restore without --force refuses a dirty tree" bash -c '! scripts/checkpoint.sh restore'
check "refused restore left the late file" test -e late.txt
bref=refs/checkpoints-backup/main
out=$(scripts/checkpoint.sh restore --force) || { echo "FAIL: restore --force failed" >&2; fails=$((fails + 1)); }
check "restore --force prints the backup ref" bash -c '[[ $1 = *"$2"* ]]' _ "$out" "$bref"
check "backup ref written" git rev-parse --verify --quiet "$bref"
check "backup holds the late file" bash -c 'git cat-file -e "$0:late.txt"' "$bref"
check "file list restored" eq "$(git ls-files -co --exclude-standard | sort)" "$expected_files"
check "contents restored" eq "$(sums)" "$expected_sums"
check "file added after the checkpoint deleted" test ! -e late.txt
check "untracked file brought back" test -e sub/untracked.txt
check "deleted file stays deleted" test ! -e gone.txt
check "ignored file left alone" eq "$(cat ignored.log)" "ignored"
check "restore left HEAD" eq "$(git rev-parse HEAD)" "$head0"
check "restore left the index" eq "$(sha_index)" "$index0"
check "restore ran no hooks" test ! -e "$tmp/marker-pre-commit"
check "restore is a no-op when already there" bash -c 'scripts/checkpoint.sh restore | grep -q "^already at"'

# Restore onto a clean tree, without --force.
git reset -q --hard
git clean -fdq
check "clean tree before plain restore" eq "$(git status --porcelain)" ""
check "plain restore of a clean tree works" scripts/checkpoint.sh restore
check "plain restore restores contents" eq "$(sums)" "$expected_sums"

# HEAD moves: restore without --force refuses, naming both commits.
tree=$(git rev-parse 'HEAD^{tree}')
newhead=$(git commit-tree -p HEAD -m moved "$tree")
git reset -q --hard
git clean -fdq
git update-ref refs/heads/main "$newhead"
rc=0
msg=$(scripts/checkpoint.sh restore 2>&1) || rc=$?
check "restore refuses when HEAD moved" eq "$rc" 1
check "refusal names both commits" bash -c \
  '[[ $1 = *"$2"* && $1 = *"$3"* ]]' _ "$msg" "$(git rev-parse --short "$head0")" "$(git rev-parse --short "$newhead")"
check "refused restore wrote nothing" eq "$(git status --porcelain)" ""

# Clean tree, no checkpoint: new branch.
git switch -q -c other
git reset -q --hard
git clean -fdq
out=$(scripts/checkpoint.sh save)
check "clean tree prints clean" eq "$out" "clean: nothing to checkpoint"
check "clean tree creates no ref" bash -c '! git rev-parse --verify --quiet refs/checkpoints/other'

# Drop.
scripts/checkpoint.sh drop main >/dev/null
check "drop removes the checkpoint" bash -c '! git rev-parse --verify --quiet refs/checkpoints/main'
check "drop removes the backup" bash -c '! git rev-parse --verify --quiet refs/checkpoints-backup/main'

# Detached HEAD.
git checkout -q --detach
rc=0
msg=$(scripts/checkpoint.sh save 2>&1) || rc=$?
check "detached HEAD exits 1" eq "$rc" 1
check "detached HEAD message" eq "$msg" "checkpoint: needs a branch (git switch -c <name>)"

check "no push in the script" bash -c '! grep -n push scripts/checkpoint.sh'

if [[ $fails -gt 0 ]]; then
  echo "test_checkpoint: $fails case(s) failed" >&2
  exit 1
fi
echo "test_checkpoint: all cases passed"
