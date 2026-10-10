#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# gup-core's performance budgets (GUP-417): run the wall-clock measurement
# tools and compare each `metric` line they print with the budget table in
# crates/gup-core/PERF_BUDGETS.md. Prints a pass/fail table and exits
# non-zero when any metric is over budget or missing. `mask perf-budget`
# runs this script.
#
#   scripts/perf_budget.sh [--budgets FILE] [--metrics FILE] [--skip-wasm]
#
#   --budgets FILE  compare against FILE's table instead of PERF_BUDGETS.md
#   --metrics FILE  compare FILE's `metric` lines instead of measuring
#   --skip-wasm     do not run scripts/wasm_size.sh (its rows then fail as
#                   not measured)
#
# Measuring needs a display and a real GPU: it runs
#   zoom_bench --present mailbox --uncapped   (release, fullscreen window)
#   pipeline_timings                          (release lib test)
#   scripts/wasm_size.sh                      (wasm32 release builds)
# On Linux with an i915 GPU it also samples the GPU clock during zoom_bench
# (GUP_GPU_FREQ_FILE overrides the sysfs file), because the driver's clock
# governor changes the GPU pass time by up to 2x (see PERF_BUDGETS.md).
#
# Budget table rows look like
#   | `zoom.gpu_pass.median_ms` | 3.89 | +20% | ... |
# with a check of: `exact` (the same value), `+N%` (at most N% over the
# recorded value), `<= X` (at most X) or `info` (printed, never fails).

set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
budgets="$root/crates/gup-core/PERF_BUDGETS.md"
metrics=""
wasm=1
while (($#)); do
  case $1 in
  --budgets)
    budgets=$2
    shift 2
    ;;
  --metrics)
    metrics=$2
    shift 2
    ;;
  --skip-wasm)
    wasm=0
    shift
    ;;
  *)
    echo "perf-budget: unknown option $1" >&2
    exit 2
    ;;
  esac
done

work=$(mktemp -d)
trap 'kill "${sampler:-}" 2>/dev/null || true; rm -rf "$work"' EXIT

if [[ -z $metrics ]]; then
  metrics="$work/metrics"
  : >"$metrics"
  cd "$root"

  echo "perf-budget: zoom_bench (100K points, Mailbox uncapped, fullscreen)" >&2
  cargo build --quiet -p gup-core --release --example zoom_bench
  freq=${GUP_GPU_FREQ_FILE:-}
  if [[ -z $freq ]]; then
    for f in /sys/class/drm/card*/gt_act_freq_mhz; do
      if [[ -r $f ]]; then
        freq=$f
        break
      fi
    done
  fi
  if [[ -n $freq ]]; then
    (while :; do
      cat "$freq"
      sleep 0.02
    done) >"$work/clock" 2>/dev/null &
    sampler=$!
  fi
  "${CARGO_TARGET_DIR:-$root/target}/release/examples/zoom_bench" \
    --present mailbox --uncapped | tee "$work/zoom" >&2
  if [[ -n ${sampler:-} ]]; then
    kill "$sampler"
    wait "$sampler" 2>/dev/null || true
    sampler=""
    sort -n "$work/clock" | awk '{v[NR] = $1} END {
      if (NR) printf "metric zoom.gpu_clock.median_mhz %d\n", v[int((NR + 1) / 2)]
    }' >>"$metrics"
  fi
  grep '^metric ' "$work/zoom" >>"$metrics"

  echo "perf-budget: pipeline_timings (release)" >&2
  cargo test --quiet -p gup-core --release --lib pipeline_timings \
    -- --ignored --nocapture >"$work/pipeline" 2>&1 || {
    cat "$work/pipeline" >&2
    exit 1
  }
  grep '^metric ' "$work/pipeline" >>"$metrics"

  if ((wasm)); then
    echo "perf-budget: wasm-size" >&2
    # Its own budget failure still prints the metrics: compare them below.
    "$root/scripts/wasm_size.sh" >"$work/wasm" 2>&1 || true
    cat "$work/wasm" >&2
    grep '^metric ' "$work/wasm" >>"$metrics" || true
  fi
fi

# The budget table: rows whose first cell is a backticked metric name.
awk -F'|' '
  $2 ~ /^ *`[a-z0-9_.]+` *$/ {
    key = $2; gsub(/[ `]/, "", key)
    rec = $3; gsub(/[ ,]/, "", rec)
    check = $4; gsub(/^ +| +$/, "", check); gsub(/`/, "", check)
    print key "\t" rec "\t" check
  }' "$budgets" >"$work/budgets"
if [[ ! -s $work/budgets ]]; then
  echo "perf-budget: no budget rows in $budgets" >&2
  exit 2
fi

awk -F'\t' -v metrics="$metrics" '
  BEGIN {
    while ((getline line < metrics) > 0) {
      n = split(line, f, " ")
      if (f[1] == "metric" && n >= 3) measured[f[2]] = f[3]
    }
    printf "\n%-34s %12s %8s %12s %12s  %s\n", "metric", "recorded", "check", "limit", "measured", "verdict"
  }
  {
    key = $1; rec = $2; check = $3; limit = "-"
    if (!(key in measured)) {
      verdict = "FAIL (not measured)"; m = "-"; failed++
    } else {
      m = measured[key]
      if (check == "info") {
        verdict = "info"
      } else if (check == "exact") {
        limit = rec
        verdict = (m == rec) ? "pass" : "FAIL"
      } else if (check ~ /^\+[0-9.]+%$/) {
        pct = substr(check, 2, length(check) - 2) / 100
        limit = sprintf("%.3f", rec * (1 + pct))
        if (m + 0 > rec * (1 + pct)) verdict = "FAIL"
        else if (m + 0 < rec * (1 - pct)) verdict = "pass (faster than recorded: re-record?)"
        else verdict = "pass"
      } else if (check ~ /^<= *[0-9.]+$/) {
        limit = check; sub(/^<= */, "", limit)
        verdict = (m + 0 <= limit + 0) ? "pass" : "FAIL"
      } else {
        verdict = "FAIL (unknown check " check ")"
      }
      if (verdict ~ /^FAIL/) failed++
    }
    printf "%-34s %12s %8s %12s %12s  %s\n", key, rec, check, limit, m, verdict
  }
  END {
    if ("zoom.gpu_clock.median_mhz" in measured && measured["zoom.gpu_clock.median_mhz"] >= 700)
      print "\nnote: the GPU clock was boosted for most of zoom_bench, so its timings are low and their checks weak; run again"
    if (failed) {
      printf "\nperf-budget: %d check(s) FAILED. A timing failure alone warrants a second run before treating it as real (PERF_BUDGETS.md).\n", failed
      exit 1
    }
    print "\nperf-budget: every check passed"
  }' "$work/budgets"
