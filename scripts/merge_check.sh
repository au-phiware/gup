#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# The post-merge checks that textual merges cannot catch (GUP-502). Runs, in
# order, gup-core's tests with all features, scripts/wasm_size.sh and
# scripts/wasm_browser.sh, and with --dogfood ./dogfood/run_all.sh. Every step
# runs even when an earlier one failed; each step's output goes to
# ${CARGO_TARGET_DIR:-target}/merge-check/<step>.log. Then it prints one table
# and exits 0 only if every step that ran passed. `mask merge-check` runs this
# script.
#
#   scripts/merge_check.sh [--dogfood] [-h]
#
#   --dogfood  also run the dogfood suite (otherwise its row says SKIP)
#   -h         print this usage and exit
#
# Steps run one at a time (they share the build directory lock and the GPU).
# Needs a GPU or lavapipe, Chromium and wasm-bindgen, like the steps it runs.

set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 1

usage() {
  cat <<'EOF'
usage: scripts/merge_check.sh [--dogfood] [-h]

  --dogfood  also run the dogfood suite (./dogfood/run_all.sh)
  -h         print this usage
EOF
}

dogfood=0
while (($#)); do
  case $1 in
  --dogfood)
    dogfood=1
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "merge_check.sh: unknown option: $1" >&2
    usage >&2
    exit 2
    ;;
  esac
done

logdir="${CARGO_TARGET_DIR:-target}/merge-check"
mkdir -p "$logdir"

echo "merge-check: HEAD: $(git log -1 --oneline)"
dirty=$(git status --porcelain)
if [[ -z $dirty ]]; then
  echo "merge-check: working tree is clean"
else
  echo "merge-check: working tree is NOT clean ($(printf '%s\n' "$dirty" | wc -l) paths)"
fi

# Parallel arrays describing each step, in run order.
names=()
results=()
seconds=()
logs=()
failed=0
ran=0

# run_step NAME COMMAND [ARGS...]: run one step, logging its output.
run_step() {
  local name=$1
  shift
  local log="$logdir/$name.log"
  echo "merge-check: running $name ..."
  SECONDS=0
  if "$@" >"$log" 2>&1; then
    results+=(PASS)
  else
    results+=(FAIL)
    failed=$((failed + 1))
  fi
  ran=$((ran + 1))
  names+=("$name")
  seconds+=("$SECONDS")
  logs+=("$log")
}

run_step gup-core cargo test -p gup-core --all-features -- --test-threads=1
run_step wasm-size ./scripts/wasm_size.sh
run_step wasm-browser ./scripts/wasm_browser.sh
if ((dogfood)); then
  run_step dogfood ./dogfood/run_all.sh
else
  names+=(dogfood)
  results+=(SKIP)
  seconds+=(-)
  logs+=(-)
fi

echo
printf '%-12s  %-6s  %7s  %s\n' step result seconds log
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
  echo "merge-check: PASS"
  exit 0
fi
echo "merge-check: FAIL ($failed of $ran)"
exit 1
