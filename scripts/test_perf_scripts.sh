#!/usr/bin/env bash
# Copyright (C) 2025 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# Tests for the performance alert and trend tracking scripts.
# Run with: bash scripts/test_perf_scripts.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PASS=0
FAIL=0

pass() {
    echo "  ✅ $1"
    PASS=$((PASS + 1))
}

fail() {
    echo "  ❌ $1: $2"
    FAIL=$((FAIL + 1))
}

# --- perf_alert.sh tests ---

echo "=== Testing perf_alert.sh ==="

# Test 1: Help output
if bash "${SCRIPT_DIR}/perf_alert.sh" --help 2>&1 | grep -q "Performance alert script"; then
    pass "Help output contains expected text"
else
    fail "Help output" "Missing expected text"
fi

# Test 2: Config parsing - groups are listed
output=$(
    # Source the parsing functions in a subshell
    CONFIG_PATH="${PROJECT_ROOT}/perf-thresholds.toml"
    # Use awk to list groups from the TOML
    awk '
        /^\[\[group\]\]/ { in_group=1; next }
        in_group && /^name[[:space:]]*=/ {
            gsub(/.*=[[:space:]]*"/, ""); gsub(/".*/, "");
            print; in_group=0
        }
    ' "$CONFIG_PATH"
)
if echo "$output" | grep -q "interaction"; then
    pass "Config parsing finds 'interaction' group"
else
    fail "Config parsing" "Missing 'interaction' group"
fi

if echo "$output" | grep -q "pattern"; then
    pass "Config parsing finds 'pattern' group"
else
    fail "Config parsing" "Missing 'pattern' group"
fi

# Test 3: Config parsing - threshold values
threshold=$(awk -v group="interaction" -v key="max_regression_percent" '
    BEGIN { in_group=0; found="" }
    /^\[\[group\]\]/ { in_group=0 }
    in_group && /^name[[:space:]]*=/ {
        gsub(/.*=[[:space:]]*"/, ""); gsub(/".*/, "");
        if ($0 == group) { in_group=2 }
        else { in_group=0 }
    }
    /^\[\[group\]\]/ { in_group=1 }
    in_group==2 && $0 ~ "^" key "[[:space:]]*=" {
        gsub(/.*=[[:space:]]*/, ""); found=$0
    }
    END { print found }
' "${PROJECT_ROOT}/perf-thresholds.toml")

if [ "$threshold" = "20.0" ]; then
    pass "Interaction max_regression_percent is 20.0"
else
    fail "Threshold parsing" "Expected 20.0, got '$threshold'"
fi

# Test 4: Report generation (skip both benchmarks and threshold tests)
report_file=$(mktemp)
json_file=$(mktemp)
if bash "${SCRIPT_DIR}/perf_alert.sh" \
    --skip-benchmarks --skip-threshold-tests \
    --report "$report_file" --json "$json_file" 2>&1 | grep -q "No performance regressions"; then
    pass "Report generation succeeds with all skipped"
else
    fail "Report generation" "Unexpected output"
fi

if grep -q "Performance Report" "$report_file"; then
    pass "Markdown report contains expected header"
else
    fail "Markdown report" "Missing expected header"
fi

if grep -q '"status"' "$json_file"; then
    pass "JSON report contains status field"
else
    fail "JSON report" "Missing status field"
fi

rm -f "$report_file" "$json_file"

# --- perf_trend.sh tests ---

echo ""
echo "=== Testing perf_trend.sh ==="

# Use a temporary directory for test history
TEST_HISTORY_DIR=$(mktemp -d)
export GUP_BENCHMARK_HISTORY_DIR="$TEST_HISTORY_DIR"

# Test 5: List with no data
if bash "${SCRIPT_DIR}/perf_trend.sh" list 2>&1 | grep -q "No trend data"; then
    pass "List with no data shows expected message"
else
    fail "Empty list" "Unexpected output"
fi

# Test 6: Clean with no data
if bash "${SCRIPT_DIR}/perf_trend.sh" clean 2>&1 | grep -q "No data to clean"; then
    pass "Clean with no data shows expected message"
else
    fail "Empty clean" "Unexpected output"
fi

# Test 7: Usage with no args
usage_output=$(bash "${SCRIPT_DIR}/perf_trend.sh" 2>&1 || true)
if echo "$usage_output" | grep -q "Usage"; then
    pass "No-arg usage shows help"
else
    fail "Usage" "Missing usage text"
fi

rm -rf "$TEST_HISTORY_DIR"

# --- Summary ---

echo ""
echo "================================"
echo "Results: ${PASS} passed, ${FAIL} failed"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
