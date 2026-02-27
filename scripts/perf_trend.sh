#!/usr/bin/env bash
# Copyright (C) 2025 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# Performance trend tracking for Gup.
#
# Records benchmark results over time and generates trend reports.
# Results are stored as JSON files in .benchmark-history/ keyed by
# date and git commit hash.
#
# Usage:
#   ./scripts/perf_trend.sh record          Record current results
#   ./scripts/perf_trend.sh report [N]      Generate trend report for last N runs (default 10)
#   ./scripts/perf_trend.sh list            List recorded data points
#   ./scripts/perf_trend.sh clean [N]       Keep only the last N data points (default 50)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
HISTORY_DIR="${GUP_BENCHMARK_HISTORY_DIR:-${PROJECT_ROOT}/.benchmark-history}"

cd "${PROJECT_ROOT}"

ensure_history_dir() {
    mkdir -p "$HISTORY_DIR"
}

# Record current benchmark results as a data point
record() {
    ensure_history_dir

    local commit
    commit=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    local branch
    branch=$(git branch --show-current 2>/dev/null || echo "unknown")
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    local date_key
    date_key=$(date -u +"%Y%m%d_%H%M%S")
    local filename="${date_key}_${commit}.json"

    echo "📊 Recording performance data point: ${filename}"

    # Run threshold tests and capture timings
    local test_output
    test_output=$(mktemp)
    cargo test --test interaction_performance_tests -- --test-threads=1 --nocapture \
        > "$test_output" 2>&1 || true

    # Extract timing data from test output
    local test_entries=""
    while IFS= read -r line; do
        local test_name=""
        local elapsed_ms=""

        # Match lines like: "Point query 1K: 63.378045ms (1 hits)"
        if echo "$line" | grep -qE '[0-9]+\.[0-9]+ms'; then
            test_name=$(echo "$line" | sed 's/^\(test [^ ]* \.\.\. \)\?//' | sed 's/:.*//')
            elapsed_ms=$(echo "$line" | grep -oE '[0-9]+\.[0-9]+ms' | head -1 | tr -d 'ms')

            if [[ -n "$test_name" && -n "$elapsed_ms" ]]; then
                if [[ -n "$test_entries" ]]; then
                    test_entries="${test_entries},"
                fi
                test_entries="${test_entries}
    {\"name\": \"${test_name}\", \"ms\": ${elapsed_ms}}"
            fi
        fi
    done < "$test_output"
    rm -f "$test_output"

    # Write data point (one test per line for easy grep parsing)
    cat > "${HISTORY_DIR}/${filename}" << EOF
{
  "timestamp": "${timestamp}",
  "commit": "${commit}",
  "branch": "${branch}",
  "test_results": [${test_entries}
  ]
}
EOF

    echo "✅ Data point saved: ${HISTORY_DIR}/${filename}"
}

# Generate a trend report from the last N data points
report() {
    ensure_history_dir

    local count="${1:-10}"
    local files
    files=$(ls -1 "${HISTORY_DIR}"/*.json 2>/dev/null | sort | tail -n "$count")

    if [[ -z "$files" ]]; then
        echo "No trend data available. Run '$0 record' first."
        return 1
    fi

    echo "## 📈 Performance Trend Report"
    echo ""
    echo "**Last ${count} data points**"
    echo ""

    # Header
    echo "| Date | Commit | Branch | Test Results |"
    echo "|------|--------|--------|-------------|"

    while IFS= read -r file; do
        local ts commit branch results
        ts=$(grep -oP '"timestamp":\s*"\K[^"]+' "$file" || echo "?")
        commit=$(grep -oP '"commit":\s*"\K[^"]+' "$file" || echo "?")
        branch=$(grep -oP '"branch":\s*"\K[^"]+' "$file" || echo "?")

        # Count test results and compute summary
        local result_count
        result_count=$(grep -c '"name"' "$file" 2>/dev/null || echo "0")

        # Extract timing summary
        local avg_ms="—"
        if [[ "$result_count" -gt 0 ]]; then
            avg_ms=$(grep -oP '"ms":\s*\K[0-9.]+' "$file" | awk '{sum+=$1; n++} END {if(n>0) printf "%.1f", sum/n; else print "—"}')
            avg_ms="${avg_ms}ms avg"
        fi

        local date_display
        date_display=$(echo "$ts" | cut -dT -f1)
        echo "| ${date_display} | \`${commit}\` | ${branch} | ${result_count} tests, ${avg_ms} |"
    done <<< "$files"

    echo ""
    echo "---"
    echo ""

    # Per-test trends (show last few data points for each test)
    echo "### Per-Test Timing Trends"
    echo ""

    # Collect all unique test names
    local test_names
    test_names=$(grep -hoP '"name":\s*"\K[^"]+' ${HISTORY_DIR}/*.json 2>/dev/null | sort -u || true)

    if [[ -n "$test_names" ]]; then
        echo "| Test | Oldest | Latest | Trend |"
        echo "|------|--------|--------|-------|"

        while IFS= read -r test_name; do
            local oldest_ms=""
            local latest_ms=""

            # Get oldest value (first file with this test)
            while IFS= read -r file; do
                local val
                # The name and ms may be on the same line or adjacent lines
                val=$(grep "\"name\": \"${test_name}\"" "$file" 2>/dev/null | grep -oP '"ms":\s*\K[0-9.]+' | head -1 || true)
                if [[ -z "$val" ]]; then
                    val=$(grep -A1 "\"name\": \"${test_name}\"" "$file" 2>/dev/null | grep -oP '"ms":\s*\K[0-9.]+' | head -1 || true)
                fi
                if [[ -n "$val" ]]; then
                    if [[ -z "$oldest_ms" ]]; then
                        oldest_ms="$val"
                    fi
                    latest_ms="$val"
                fi
            done <<< "$files"

            if [[ -n "$oldest_ms" && -n "$latest_ms" ]]; then
                local trend
                trend=$(awk "BEGIN {
                    d = $latest_ms - $oldest_ms;
                    pct = ($oldest_ms > 0) ? d / $oldest_ms * 100 : 0;
                    if (pct > 5) printf \"🔴 +%.1f%%\", pct;
                    else if (pct < -5) printf \"🟢 %.1f%%\", pct;
                    else printf \"⚪ %.1f%%\", pct;
                }")
                echo "| ${test_name} | ${oldest_ms}ms | ${latest_ms}ms | ${trend} |"
            fi
        done <<< "$test_names"
    else
        echo "_No per-test data available._"
    fi
}

# List recorded data points
list_data() {
    ensure_history_dir

    local files
    files=$(ls -1 "${HISTORY_DIR}"/*.json 2>/dev/null || true)

    if [[ -z "$files" ]]; then
        echo "No trend data recorded yet."
        return
    fi

    echo "📊 Recorded performance data points:"
    echo ""

    local count=0
    while IFS= read -r file; do
        local basename
        basename=$(basename "$file")
        local ts
        ts=$(grep -oP '"timestamp":\s*"\K[^"]+' "$file" || echo "?")
        local commit
        commit=$(grep -oP '"commit":\s*"\K[^"]+' "$file" || echo "?")
        echo "  ${basename}  (${ts}, commit ${commit})"
        count=$((count + 1))
    done <<< "$files"

    echo ""
    echo "Total: ${count} data points"
}

# Clean old data points, keeping the last N
clean() {
    ensure_history_dir

    local keep="${1:-50}"

    local files
    files=$(ls -1 "${HISTORY_DIR}"/*.json 2>/dev/null | sort || true)

    if [[ -z "$files" ]]; then
        echo "No data to clean."
        return
    fi

    local total
    total=$(echo "$files" | wc -l)

    if [[ "$total" -le "$keep" ]]; then
        echo "Only ${total} data points (keeping ${keep}). Nothing to clean."
        return
    fi

    local to_delete=$((total - keep))
    echo "🗑️  Removing ${to_delete} old data points (keeping ${keep})..."

    echo "$files" | head -n "$to_delete" | while IFS= read -r file; do
        rm -f "$file"
        echo "  Removed: $(basename "$file")"
    done

    echo "✅ Done. ${keep} data points remain."
}

# --- Main ---

usage() {
    echo "Usage: $0 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  record        Record current benchmark results"
    echo "  report [N]    Generate trend report (last N runs, default 10)"
    echo "  list          List recorded data points"
    echo "  clean [N]     Keep only the last N data points (default 50)"
    exit 1
}

if [[ $# -lt 1 ]]; then
    usage
fi

command="$1"
shift

case "$command" in
    record)
        record
        ;;
    report)
        report "${1:-10}"
        ;;
    list)
        list_data
        ;;
    clean)
        clean "${1:-50}"
        ;;
    *)
        echo "Unknown command: $command"
        usage
        ;;
esac
