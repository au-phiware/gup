#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Seeded tests for the consistency checks of scripts/release_check.sh
# (GUP-598), run on a scratch copy of the files they read: the root
# Cargo.toml, CHANGELOG.md, a `cargo metadata` JSON and a git repository. Each
# case breaks one thing and must make exactly its check fail. The gates the
# script runs (all-check, tests, ...) are not run here.
set -uo pipefail

# Run from the pre-commit hook, git inherits GIT_DIR and friends, which would
# point the scratch repositories' git commands at the real repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
script=$root/scripts/release_check.sh

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fails=0
fail() {
  echo "FAIL: $*" >&2
  fails=$((fails + 1))
}

# The real metadata and a slug list that contains every category in use.
(cd "$root" && cargo metadata --format-version 1 --no-deps) >"$tmp/metadata.json" || {
  echo "FAIL: cargo metadata" >&2
  exit 1
}
jq -r '.packages[].categories[]' "$tmp/metadata.json" | sort -u >"$tmp/slugs.txt"

# new_case NAME: a scratch tree that passes every consistency check.
new_case() {
  local d=$tmp/$1
  mkdir -p "$d"
  cp "$root/Cargo.toml" "$root/CHANGELOG.md" "$d/"
  cp "$tmp/metadata.json" "$d/metadata.json"
  cp "$tmp/slugs.txt" "$d/slugs.txt"
  git -C "$d" init -q -b main
  git -C "$d" add Cargo.toml CHANGELOG.md
  git -C "$d" -c user.name=t -c user.email=t@example.com commit -qm base
  git -C "$d" update-ref refs/remotes/origin/main HEAD
  # metadata.json and slugs.txt are scratch inputs, not part of the tree.
  printf 'metadata.json\nslugs.txt\n' >"$d/.git/info/exclude"
}

# run_case NAME CHECK: run one check on the scratch tree; sets $out and $rc.
run_case() {
  local d=$tmp/$1
  out=$(RELEASE_CHECK_ROOT=$d RELEASE_CHECK_METADATA=$d/metadata.json \
    RELEASE_CHECK_SLUGS=$d/slugs.txt CARGO_TARGET_DIR=$tmp/logs \
    "$script" --only "$2" 2>&1)
  rc=$?
}

expect_pass() {
  run_case "$1" "$2"
  ((rc == 0)) || fail "$1: $2 should pass: $out"
}

# expect_fail NAME CHECK PATTERN: the check fails and its output has PATTERN.
expect_fail() {
  run_case "$1" "$2"
  if ((rc == 0)); then
    fail "$1: $2 should fail: $out"
  elif ! grep -q -- "$3" <<<"$out"; then
    fail "$1: $2 failed without '$3': $out"
  fi
}

# The untouched copy passes every consistency check.
new_case ok
for c in one-version seven categories clean-tree; do
  expect_pass ok "$c"
done

# A 0.1.1 in one place but not the others: each place in turn.
new_case ver_workspace
sed -i '0,/^version = "0.1.0"/s//version = "0.1.1"/' "$tmp/ver_workspace/Cargo.toml"
git -C "$tmp/ver_workspace" commit -qam v
# The metadata still says 0.1.0, so the manifest disagrees with it.
expect_fail ver_workspace one-version 'version is not'

new_case ver_dependency
sed -i 's/^gup-text = \(.*\)"=0.1.0"/gup-text = \1"=0.1.1"/' "$tmp/ver_dependency/Cargo.toml"
git -C "$tmp/ver_dependency" commit -qam v
expect_fail ver_dependency one-version '=0.1.1'

new_case ver_metadata
jq '(.packages[] | select(.name == "gup-text") | .version) = "0.1.1"' \
  "$tmp/metadata.json" >"$tmp/ver_metadata/metadata.json"
expect_fail ver_metadata one-version 'gup-text has version 0.1.1'

new_case ver_requirement
jq '(.packages[] | select(.name == "gup") | .dependencies[] | select(.name == "gup-core") | .req) = "=0.1.1"' \
  "$tmp/metadata.json" >"$tmp/ver_requirement/metadata.json"
expect_fail ver_requirement one-version 'gup requires gup-core =0.1.1'

new_case ver_changelog
sed -i 's/^## \[0.1.0\]/## [0.1.1]/' "$tmp/ver_changelog/CHANGELOG.md"
git -C "$tmp/ver_changelog" commit -qam v
expect_fail ver_changelog one-version 'CHANGELOG.md'

# A missing CHANGELOG heading.
new_case no_heading
sed -i '/^## \[0.1.0\]/d' "$tmp/no_heading/CHANGELOG.md"
git -C "$tmp/no_heading" commit -qam v
expect_fail no_heading one-version 'CHANGELOG.md'

# publish = false on a published crate, and an unpublished crate that is not.
new_case unpublished
jq '(.packages[] | select(.name == "gup-egui") | .publish) = []' \
  "$tmp/metadata.json" >"$tmp/unpublished/metadata.json"
expect_fail unpublished seven 'gup-egui'

new_case extra_published
jq '(.packages[] | select(.name == "gup-visual-regression") | .publish) = null' \
  "$tmp/metadata.json" >"$tmp/extra_published/metadata.json"
expect_fail extra_published seven 'gup-visual-regression'

# A category that is not a slug.
new_case bad_slug
jq '(.packages[] | select(.name == "gup") | .categories) += ["not-a-category"]' \
  "$tmp/metadata.json" >"$tmp/bad_slug/metadata.json"
expect_fail bad_slug categories "gup: 'not-a-category'"

# Offline: the slug list cannot be fetched, which is a SKIP and not a pass.
new_case offline
out=$(RELEASE_CHECK_ROOT=$tmp/offline RELEASE_CHECK_METADATA=$tmp/offline/metadata.json \
  CARGO_TARGET_DIR=$tmp/logs HTTPS_PROXY=http://127.0.0.1:9 https_proxy=http://127.0.0.1:9 \
  "$script" --only categories 2>&1)
if ! grep -q 'SKIP' <<<"$out"; then
  fail "offline: categories should be SKIP: $out"
fi

# A dirty tree, and a HEAD that is behind origin/main.
new_case dirty
echo x >"$tmp/dirty/stray.txt"
expect_fail dirty clean-tree 'stray.txt'

new_case behind
git -C "$tmp/behind" -c user.name=t -c user.email=t@example.com commit -q --allow-empty -m ahead
git -C "$tmp/behind" update-ref refs/remotes/origin/main HEAD
git -C "$tmp/behind" reset -q --hard HEAD~1
expect_fail behind clean-tree 'not on or above origin/main'

# The script never publishes: its only mention of `cargo publish` must be a
# dry run, and it must not tag or push.
if grep -n 'cargo publish' "$script" | grep -v -e '--dry-run' -e '^[0-9]*:#'; then
  fail "release_check.sh mentions cargo publish without --dry-run"
fi
if grep -nE 'git (tag|push)|cargo login' "$script" | grep -v '^[0-9]*:#'; then
  fail "release_check.sh must not tag, push or log in"
fi

if ((fails > 0)); then
  echo "test_release_check: $fails failure(s)" >&2
  exit 1
fi
echo "test_release_check: OK"
