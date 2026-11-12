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

# Nothing here may read or change the user's git configuration or the real
# repository (a `git init` with GIT_DIR set once rewrote the shared config).
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.com
export GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.com

# Every git command in this script runs through this function, which refuses
# anything that is not `git -C <directory under $tmp> ...`.
git() {
  if [[ ${1:-} != -C || ${2:-} != "$tmp"/* ]]; then
    echo "FAIL: test_release_check.sh ran git outside its scratch directory: git $*" >&2
    exit 1
  fi
  command git "$@"
}

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

# --- the ci check (GUP-604), against a fake `gh` that serves canned JSON ---

mkdir -p "$tmp/bin"
cat >"$tmp/bin/gh" <<'EOF'
#!/usr/bin/env bash
# A fake gh: canned JSON from $FAKE_GH_DIR, and a log of what was asked.
echo "$*" >>"$FAKE_GH_DIR/calls"
case "$1 $2" in
"auth status") [[ -z ${FAKE_GH_NOAUTH:-} ]] || exit 1 ;;
"run list") cat "$FAKE_GH_DIR/runs.json" ;;
"run view") cat "$FAKE_GH_DIR/jobs-$3.json" ;;
*)
  echo "fake gh: unexpected: $*" >&2
  exit 99
  ;;
esac
EOF
chmod +x "$tmp/bin/gh"

# A PATH without gh: the tools the script uses, linked into one directory.
mkdir -p "$tmp/nogh"
for t in bash git jq awk cut dirname mkdir cat grep sed sort head tail diff; do
  ln -sf "$(command -v "$t")" "$tmp/nogh/$t"
done

# new_ci NAME: canned runs and jobs for a green main, one run per workflow
# (ids 1..8) with the jobs the script requires and a skipped deploy job.
new_ci() {
  new_case "$1"
  local d=$tmp/$1 id=0 wf job line
  mkdir -p "$d/gh"
  echo '[]' >"$d/gh/runs.json"
  for wf in 'Tests' 'Lint' 'Visual regression' 'Dogfood' 'Gallery' 'WASM Compilation Check' 'Release check' 'Platforms'; do
    id=$((id + 1))
    jq --arg w "$wf" --argjson id "$id" \
      '. += [{databaseId: $id, workflowName: $w, status: "completed", conclusion: "success", createdAt: "2026-11-04T00:00:00Z"}]' \
      "$d/gh/runs.json" >"$d/gh/runs.tmp" && mv "$d/gh/runs.tmp" "$d/gh/runs.json"
    echo '{"jobs": [{"name": "deploy", "conclusion": "skipped"}]}' >"$d/gh/jobs-$id.json"
    # Job names as the workflows name them; the script matches substrings.
    while IFS= read -r line; do
      [[ ${line%%|*} == "$wf" ]] || continue
      job=${line#*|}
      jq --arg j "$job (lavapipe)" '.jobs += [{name: $j, conclusion: "success"}]' \
        "$d/gh/jobs-$id.json" >"$d/gh/jobs.tmp" && mv "$d/gh/jobs.tmp" "$d/gh/jobs-$id.json"
    done <<<"$ci_required"
  done
}

# The script's own list of required jobs, so the fixture follows it.
ci_required=$(sed -n "/^required_jobs=(/,/^)/p" "$script" | sed -n "s/^  '\(.*\)'$/\1/p")
if [[ $(wc -l <<<"$ci_required") -lt 10 ]]; then
  fail "could not read required_jobs from release_check.sh"
fi

# run_ci NAME [ENV...]: the ci check with the fake gh first on PATH.
run_ci() {
  local d=$tmp/$1
  shift
  : >"$d/gh/calls"
  out=$(env "$@" PATH="$tmp/bin:$PATH" FAKE_GH_DIR="$d/gh" RELEASE_CHECK_ROOT="$d" CARGO_TARGET_DIR="$tmp/logs" \
    "$script" --only ci 2>&1)
  rc=$?
}

new_ci ci_ok
run_ci ci_ok
((rc == 0)) || fail "ci_ok: ci should pass: $out"
grep -q 'PASS' <<<"$out" || fail "ci_ok: no PASS row: $out"
# Read-only: only these three gh commands.
if grep -vE '^(auth status|run list --commit [0-9a-f]+ |run view [0-9]+ --json jobs$)' "$tmp/ci_ok/gh/calls"; then
  fail "ci_ok: gh was asked for something other than auth status, run list or run view"
fi

# A green workflow with a red job (the Platforms macOS job).
new_ci ci_red_job
jq '(.jobs[] | select(.name | contains("macOS")) | .conclusion) = "failure"' \
  "$tmp/ci_red_job/gh/jobs-8.json" >"$tmp/ci_red_job/gh/j.tmp" && mv "$tmp/ci_red_job/gh/j.tmp" "$tmp/ci_red_job/gh/jobs-8.json"
run_ci ci_red_job
((rc != 0)) || fail "ci_red_job: ci should fail: $out"
grep -q "macOS render smoke' is failure" "$tmp/logs/release-check/ci.log" || fail "ci_red_job: the log does not name the macOS job: $(cat "$tmp/logs/release-check/ci.log")"

# A required job that does not exist.
new_ci ci_missing_job
jq '.jobs |= map(select(.name | contains("feature-matrix") | not))' \
  "$tmp/ci_missing_job/gh/jobs-1.json" >"$tmp/ci_missing_job/gh/j.tmp" && mv "$tmp/ci_missing_job/gh/j.tmp" "$tmp/ci_missing_job/gh/jobs-1.json"
run_ci ci_missing_job
((rc != 0)) || fail "ci_missing_job: ci should fail: $out"
grep -q "feature-matrix' is missing" "$tmp/logs/release-check/ci.log" || fail "ci_missing_job: the log does not name the job: $(cat "$tmp/logs/release-check/ci.log")"

# A workflow whose run failed, and one with no run at all.
new_ci ci_failed_run
jq '(.[] | select(.workflowName == "Lint") | .conclusion) = "failure"' \
  "$tmp/ci_failed_run/gh/runs.json" >"$tmp/ci_failed_run/gh/r.tmp" && mv "$tmp/ci_failed_run/gh/r.tmp" "$tmp/ci_failed_run/gh/runs.json"
run_ci ci_failed_run
((rc != 0)) || fail "ci_failed_run: ci should fail: $out"
grep -q 'Lint: run 2 is completed/failure' "$tmp/logs/release-check/ci.log" || fail "ci_failed_run: wrong reason: $(cat "$tmp/logs/release-check/ci.log")"

new_ci ci_no_run
jq 'map(select(.workflowName != "Dogfood"))' \
  "$tmp/ci_no_run/gh/runs.json" >"$tmp/ci_no_run/gh/r.tmp" && mv "$tmp/ci_no_run/gh/r.tmp" "$tmp/ci_no_run/gh/runs.json"
run_ci ci_no_run
((rc != 0)) || fail "ci_no_run: ci should fail: $out"
grep -q 'Dogfood: no run on' "$tmp/logs/release-check/ci.log" || fail "ci_no_run: wrong reason: $(cat "$tmp/logs/release-check/ci.log")"

# An older failed run of a workflow does not count when a later run passed.
new_ci ci_rerun
jq '. += [{databaseId: 9, workflowName: "Lint", status: "completed", conclusion: "failure", createdAt: "2026-11-03T00:00:00Z"}]' \
  "$tmp/ci_rerun/gh/runs.json" >"$tmp/ci_rerun/gh/r.tmp" && mv "$tmp/ci_rerun/gh/r.tmp" "$tmp/ci_rerun/gh/runs.json"
run_ci ci_rerun
((rc == 0)) || fail "ci_rerun: the latest Lint run passed, ci should pass: $out"

# gh absent, and gh not logged in: SKIP, never FAIL.
new_ci ci_nogh
out=$(env PATH="$tmp/nogh" RELEASE_CHECK_ROOT="$tmp/ci_nogh" CARGO_TARGET_DIR="$tmp/logs" "$tmp/nogh/bash" "$script" --only ci 2>&1)
rc=$?
if ! { ((rc == 0)) && grep -q 'SKIP' <<<"$out"; }; then fail "ci_nogh: ci should SKIP without gh: rc=$rc $out"; fi
grep -q 'gh is not installed' "$tmp/logs/release-check/ci.log" || fail "ci_nogh: no reason in the log"

new_ci ci_noauth
run_ci ci_noauth FAKE_GH_NOAUTH=1
if ! { ((rc == 0)) && grep -q 'SKIP' <<<"$out"; }; then fail "ci_noauth: ci should SKIP when gh is not authenticated: rc=$rc $out"; fi

# run_ci_ref NAME REF: the ci check with --ci-ref REF and the fake gh first on PATH.
run_ci_ref() {
  local d=$tmp/$1
  : >"$d/gh/calls"
  out=$(PATH="$tmp/bin:$PATH" FAKE_GH_DIR="$d/gh" RELEASE_CHECK_ROOT="$d" CARGO_TARGET_DIR="$tmp/logs" \
    "$script" --only ci --ci-ref "$2" 2>&1)
  rc=$?
}

# --ci-ref (GUP-606): a pushed branch's commit is read, not origin/main's.
new_ci ci_ref_pushed
base=$(git -C "$tmp/ci_ref_pushed" rev-parse HEAD)
git -C "$tmp/ci_ref_pushed" commit -q --allow-empty -m topic
topic=$(git -C "$tmp/ci_ref_pushed" rev-parse HEAD)
git -C "$tmp/ci_ref_pushed" update-ref refs/remotes/origin/topic HEAD
run_ci_ref ci_ref_pushed origin/topic
((rc == 0)) || fail "ci_ref_pushed: ci should pass for a pushed branch: $out"
grep -q "run list --commit $topic " "$tmp/ci_ref_pushed/gh/calls" || fail "ci_ref_pushed: runs of $topic were not read: $(cat "$tmp/ci_ref_pushed/gh/calls")"
if grep -q "run list --commit $base " "$tmp/ci_ref_pushed/gh/calls"; then
  fail "ci_ref_pushed: origin/main's runs were read instead of the branch's"
fi

# --ci-ref HEAD on a commit no remote-tracking ref contains: SKIP, no gh run list.
new_ci ci_ref_unpushed
git -C "$tmp/ci_ref_unpushed" commit -q --allow-empty -m local
run_ci_ref ci_ref_unpushed HEAD
((rc == 0)) || fail "ci_ref_unpushed: ci should not fail for an unpushed commit: rc=$rc $out"
grep -q 'SKIP' <<<"$out" || fail "ci_ref_unpushed: ci should SKIP for an unpushed commit: $out"
grep -q 'was not pushed' "$tmp/logs/release-check/ci.log" || fail "ci_ref_unpushed: no reason in the log: $(cat "$tmp/logs/release-check/ci.log")"
if grep -q '^run list' "$tmp/ci_ref_unpushed/gh/calls"; then
  fail "ci_ref_unpushed: gh run list was asked for an unpushed commit"
fi

# A ref that is not a commit is a failure, not a SKIP.
new_ci ci_ref_unknown
run_ci_ref ci_ref_unknown no-such-ref
((rc != 0)) || fail "ci_ref_unknown: ci should fail for an unknown ref: $out"
grep -q 'no commit no-such-ref' "$tmp/logs/release-check/ci.log" || fail "ci_ref_unknown: wrong reason: $(cat "$tmp/logs/release-check/ci.log")"

# The script never changes a workflow run.
if grep -nE 'gh (run (rerun|cancel|watch|delete)|workflow|api)' "$script" | grep -v '^[0-9]*:#'; then
  fail "release_check.sh must only read workflow runs"
fi

# --- publish_check.sh (GUP-603): a build directory of its own ---

# A fake cargo records the build directory it is given and the arguments.
mkdir -p "$tmp/pbin" "$tmp/pt"
cat >"$tmp/pbin/cargo" <<'EOF'
#!/usr/bin/env bash
echo "build_dir=$CARGO_BUILD_BUILD_DIR" >"$FAKE_CARGO_OUT"
echo "args=$*" >>"$FAKE_CARGO_OUT"
# What a stale overlay would still be, if the script did not clear it.
ls "$CARGO_BUILD_BUILD_DIR/package" >>"$FAKE_CARGO_OUT" 2>&1
EOF
chmod +x "$tmp/pbin/cargo"
shared=$tmp/shared-build
mkdir -p "$shared/package/tmp-registry/index" "$tmp/pt/publish-check/package/tmp-registry" "$tmp/pt/publish-check/package/tmp-crate"
touch "$shared/package/tmp-registry/stale" "$tmp/pt/publish-check/package/tmp-registry/stale"
PATH="$tmp/pbin:$PATH" CARGO_BUILD_BUILD_DIR=$shared CARGO_TARGET_DIR=$tmp/pt FAKE_CARGO_OUT=$tmp/pcargo.out \
  "$root/scripts/publish_check.sh" || fail "publish_check.sh failed"
grep -qx "build_dir=$tmp/pt/publish-check" "$tmp/pcargo.out" || fail "publish_check.sh did not use its own build directory: $(cat "$tmp/pcargo.out")"
grep -qx 'args=publish --workspace --dry-run --locked' "$tmp/pcargo.out" || fail "publish_check.sh arguments: $(cat "$tmp/pcargo.out")"
[[ -e $shared/package/tmp-registry/stale ]] || fail "publish_check.sh touched the shared package directory"
if grep -q tmp- "$tmp/pcargo.out"; then
  fail "publish_check.sh left a stale overlay in its own directory: $(cat "$tmp/pcargo.out")"
fi
# The mask task runs the script, and nothing else publishes.
grep -q '^./scripts/publish_check.sh$' "$root/maskfile.md" || fail "mask publish-check does not run scripts/publish_check.sh"
if grep -n 'cargo publish' "$root/scripts/publish_check.sh" | grep -v -e '--dry-run' -e '^[0-9]*:#'; then
  fail "publish_check.sh mentions cargo publish without --dry-run"
fi

# The script never publishes: its only mention of `cargo publish` must be a
# dry run, and it must not tag or push.
if grep -n 'cargo publish' "$script" | grep -v -e '--dry-run' -e '^[0-9]*:#'; then
  fail "release_check.sh mentions cargo publish without --dry-run"
fi
if grep -nE 'git (tag|push)|cargo login' "$script" | grep -v '^[0-9]*:#'; then
  fail "release_check.sh must not tag, push or log in"
fi

# The real repository must still be a work tree.
if [[ $(command git -C "$root" rev-parse --is-bare-repository) != false ]]; then
  fail "the real repository reports itself bare"
fi

if ((fails > 0)); then
  echo "test_release_check: $fails failure(s)" >&2
  exit 1
fi
echo "test_release_check: OK"
