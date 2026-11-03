#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# One command that says whether the tree is ready to publish (GUP-598). It runs
# every gate below, each regardless of the others' results, prints one table
# (check, result, seconds) and exits 0 only if no check failed. It never
# publishes, tags or pushes: the only publish it runs is the dry run inside
# `mask publish-check`. `mask release-check` runs this script.
#
#   scripts/release_check.sh [--only CHECK[,CHECK...]] [-h]
#
# Checks, in order:
#   all-check    mask all-check
#   tests        cargo test -- --test-threads=1, then the same for gup-core
#   merge-check  mask merge-check --dogfood
#   publish      mask publish-check
#   docs-rs      mask docs-rs
#   one-version  workspace version, every `=X` sibling requirement and the
#                first CHANGELOG release heading agree
#   seven        exactly the seven published crates have publish == null
#   categories   every category is a crates.io category slug (SKIP offline)
#   clean-tree   no uncommitted paths, and HEAD is on or above origin/main
#
# Expected time: about 45 minutes (EXPECTED_MINUTES below; re-record it after
# a release-day run). It is meant for release day, not for every commit. Needs
# the network, a GPU or lavapipe, Chromium and wasm-bindgen, like the gates it
# runs.
#
# For the seeded tests (scripts/test_release_check.sh) the consistency checks
# read RELEASE_CHECK_ROOT (default: this checkout), RELEASE_CHECK_METADATA (a
# `cargo metadata --no-deps` JSON file, default: run cargo metadata) and
# RELEASE_CHECK_SLUGS (a file of category slugs, default: fetch crates.io).

# The check_* functions are called through run_step.
# shellcheck disable=SC2329
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here" || exit 1
root=${RELEASE_CHECK_ROOT:-$here}

published=(gup gup-core gup-core-macros gup-wgsl gup-text gup-egui gup-bevy)
all_checks=(all-check tests merge-check publish docs-rs one-version seven categories clean-tree)
EXPECTED_MINUTES=45

usage() {
  cat <<'EOF'
usage: scripts/release_check.sh [--only CHECK[,CHECK...]] [-h]

  --only  run just these checks (all-check, tests, merge-check, publish,
          docs-rs, one-version, seven, categories, clean-tree)
  -h      print this usage
EOF
}

only=()
while (($#)); do
  case $1 in
  --only)
    [[ $# -ge 2 ]] || {
      usage >&2
      exit 2
    }
    IFS=, read -r -a only <<<"$2"
    shift 2
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "release_check.sh: unknown option: $1" >&2
    usage >&2
    exit 2
    ;;
  esac
done
for o in "${only[@]}"; do
  [[ " ${all_checks[*]} " == *" $o "* ]] || {
    echo "release_check.sh: unknown check: $o" >&2
    exit 2
  }
done

logdir="${CARGO_TARGET_DIR:-target}/release-check"
mkdir -p "$logdir"

metadata_json() {
  if [[ -n ${RELEASE_CHECK_METADATA:-} ]]; then
    cat "$RELEASE_CHECK_METADATA"
  else
    cargo metadata --format-version 1 --no-deps
  fi
}

# The published crate names as a JSON array, for jq.
published_json() {
  printf '%s\n' "${published[@]}" | jq -R . | jq -s .
}

# --- the consistency checks: print the reason, return non-zero ---

check_one_version() {
  local meta version names bad=0 line name dep req v heading
  meta=$(metadata_json) || return 1
  version=$(jq -r '.packages[] | select(.name == "gup") | .version' <<<"$meta")
  if [[ -z $version ]]; then
    echo "gup has no version in cargo metadata"
    return 1
  fi
  echo "version: $version"
  names=$(published_json)
  # Every published crate's own version, and its requirements on the others.
  while IFS=$'\t' read -r name v; do
    if [[ $v != "$version" ]]; then
      echo "$name has version $v, not $version"
      bad=1
    fi
  done < <(jq -r --argjson n "$names" '.packages[] | select(.name as $x | $n | index($x)) | [.name, .version] | @tsv' <<<"$meta")
  while IFS=$'\t' read -r name dep req; do
    if [[ $req != "=$version" ]]; then
      echo "$name requires $dep $req, not =$version"
      bad=1
    fi
  done < <(jq -r --argjson n "$names" '.packages[] | select(.name as $x | $n | index($x)) | .name as $p | .dependencies[] | select(.kind == null) | select(.name as $d | $n | index($d)) | [$p, .name, .req] | @tsv' <<<"$meta")
  # The manifest's own sibling requirements (workspace.dependencies and the
  # facade's gup-core line), and [workspace.package].
  while IFS= read -r line; do
    if [[ $line != *"\"=$version\""* ]]; then
      echo "Cargo.toml: $line (expected \"=$version\")"
      bad=1
    fi
  done < <(grep -E '^gup[a-z-]* *= *\{.*version *= *"' "$root/Cargo.toml")
  if ! grep -qE '^version *= *"'"$version"'"' "$root/Cargo.toml"; then
    echo "Cargo.toml: [workspace.package] version is not $version"
    bad=1
  fi
  # The first release heading in the changelog (skipping Unreleased).
  heading=$(grep -m1 -E '^## \[[0-9]' "$root/CHANGELOG.md" || true)
  if [[ $heading != "## [$version]"* ]]; then
    echo "CHANGELOG.md: first release heading is '${heading:-none}', expected '## [$version]'"
    bad=1
  fi
  return "$bad"
}

check_seven() {
  local meta got want
  meta=$(metadata_json) || return 1
  got=$(jq -r '.packages[] | select(.publish == null) | .name' <<<"$meta" | sort)
  want=$(printf '%s\n' "${published[@]}" | sort)
  if [[ $got != "$want" ]]; then
    echo "the publishable crates differ from the seven (< wanted, > found):"
    diff <(echo "$want") <(echo "$got") | sed 's/^/  /'
    return 1
  fi
}

# Exit 2 (SKIP) when the slug list cannot be fetched.
check_categories() {
  local meta slugs cats names bad=0 p c
  meta=$(metadata_json) || return 1
  if [[ -n ${RELEASE_CHECK_SLUGS:-} ]]; then
    slugs=$(cat "$RELEASE_CHECK_SLUGS")
  else
    slugs=$(curl -sf --max-time 30 -H "User-Agent: gup-release-check" -H "Accept: application/json" https://crates.io/api/v1/category_slugs) || {
      echo "skipped: could not fetch https://crates.io/api/v1/category_slugs"
      return 2
    }
    slugs=$(jq -r '.category_slugs[].slug' <<<"$slugs") || {
      echo "skipped: unexpected category_slugs response"
      return 2
    }
  fi
  names=$(published_json)
  cats=$(jq -r --argjson n "$names" '.packages[] | select(.name as $x | $n | index($x)) | .name as $p | .categories[] | [$p, .] | @tsv' <<<"$meta")
  while IFS=$'\t' read -r p c; do
    [[ -n $c ]] || continue
    if ! grep -qxF "$c" <<<"$slugs"; then
      echo "$p: '$c' is not a crates.io category slug"
      bad=1
    fi
  done <<<"$cats"
  return "$bad"
}

check_clean_tree() {
  local dirty
  dirty=$(git -C "$root" status --porcelain)
  if [[ -n $dirty ]]; then
    echo "working tree is not clean:"
    printf '%s\n' "$dirty" | head -20 | sed 's/^/  /'
    return 1
  fi
  if ! git -C "$root" rev-parse --verify -q origin/main >/dev/null; then
    echo "no origin/main to compare HEAD with"
    return 1
  fi
  if ! git -C "$root" merge-base --is-ancestor origin/main HEAD; then
    echo "HEAD ($(git -C "$root" rev-parse --short HEAD)) is not on or above origin/main"
    return 1
  fi
}

run_tests() {
  cargo test -- --test-threads=1 &&
    cargo test -p gup-core -- --test-threads=1
}

# --- the runner ---

names=()
results=()
seconds=()
logs=()
failed=0
ran=0

selected() {
  ((${#only[@]} == 0)) && return 0
  [[ " ${only[*]} " == *" $1 "* ]]
}

# run_step NAME COMMAND [ARGS...]: run one step, logging its output. A command
# that returns 2 is reported as SKIP.
run_step() {
  local name=$1 log rc
  shift
  selected "$name" || return 0
  log="$logdir/$name.log"
  echo "release-check: running $name ..."
  SECONDS=0
  "$@" >"$log" 2>&1
  rc=$?
  case $rc in
  0) results+=(PASS) ;;
  2) results+=(SKIP) ;;
  *)
    results+=(FAIL)
    failed=$((failed + 1))
    ;;
  esac
  ran=$((ran + 1))
  names+=("$name")
  seconds+=("$SECONDS")
  logs+=("$log")
}

echo "release-check: HEAD: $(git -C "$root" log -1 --oneline)"
if ((${#only[@]} == 0)); then
  echo "release-check: expected time: about $EXPECTED_MINUTES minutes"
fi

run_step all-check mask all-check
run_step tests run_tests
run_step merge-check mask merge-check --dogfood
run_step publish mask publish-check
run_step docs-rs mask docs-rs
run_step one-version check_one_version
run_step seven check_seven
run_step categories check_categories
run_step clean-tree check_clean_tree

echo
printf '%-12s  %-6s  %7s  %s\n' check result seconds log
for i in "${!names[@]}"; do
  printf '%-12s  %-6s  %7s  %s\n' "${names[$i]}" "${results[$i]}" "${seconds[$i]}" "${logs[$i]}"
done

for i in "${!names[@]}"; do
  if [[ ${results[$i]} == FAIL ]]; then
    echo
    echo "--- ${names[$i]}: last 20 lines of ${logs[$i]}"
    tail -n 20 "${logs[$i]}"
  fi
done

echo
if ((failed == 0)); then
  echo "release-check: PASS (ready to publish; the owner publishes, see docs/RELEASING.md)"
  exit 0
fi
echo "release-check: FAIL ($failed of $ran)"
exit 1
