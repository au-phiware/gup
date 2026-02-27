#!/usr/bin/env bash
# Copyright (C) 2025 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# Performance alert script for CI/CD pipelines.
#
# Reads perf-thresholds.toml, runs mask perf-check and criterion benchmarks,
# compares results against stored baselines, and outputs a unified Markdown
# report suitable for PR comments.
#
# Usage:
#   ./scripts/perf_alert.sh [options]
#
# Options:
#   --baseline <name>     Baseline name to compare against (default: main)
#   --save-baseline <name> Save current results as a named baseline
#   --report <path>       Output report path (default: performance_report.md)
#   --json <path>         Output JSON path (default: performance_report.json)
#   --config <path>       Threshold config path (default: perf-thresholds.toml)
#   --skip-benchmarks     Skip criterion benchmarks, only run threshold tests
#   --skip-threshold-tests Skip threshold tests, only run criterion benchmarks
#   --fail-on-regression  Exit with code 1 if regressions are detected
#   --help                Show this help message

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Defaults
BASELINE_NAME="main"
SAVE_BASELINE=""
REPORT_PATH="performance_report.md"
JSON_PATH="performance_report.json"
CONFIG_PATH="${PROJECT_ROOT}/perf-thresholds.toml"
SKIP_BENCHMARKS=false
SKIP_THRESHOLD_TESTS=false
FAIL_ON_REGRESSION=false

# Parse options
while [[ $# -gt 0 ]]; do
    case "$1" in
        --baseline)
            BASELINE_NAME="$2"
            shift 2
            ;;
        --save-baseline)
            SAVE_BASELINE="$2"
            shift 2
            ;;
        --report)
            REPORT_PATH="$2"
            shift 2
            ;;
        --json)
            JSON_PATH="$2"
            shift 2
            ;;
        --config)
            CONFIG_PATH="$2"
            shift 2
            ;;
        --skip-benchmarks)
            SKIP_BENCHMARKS=true
            shift
            ;;
        --skip-threshold-tests)
            SKIP_THRESHOLD_TESTS=true
            shift
            ;;
        --fail-on-regression)
            FAIL_ON_REGRESSION=true
            shift
            ;;
        --help)
            sed -n '/^# Performance alert/,/^[^#]/p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

cd "${PROJECT_ROOT}"

# --- Threshold config parsing ---

# Parse a TOML value for a given group and key.
# Falls back to [defaults] if the group has no override.
parse_threshold() {
    local group_name="$1"
    local key="$2"
    local default_val="$3"

    # Try group-specific value first
    local val
    val=$(awk -v group="$group_name" -v key="$key" '
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
    ' "$CONFIG_PATH" 2>/dev/null || true)

    if [[ -n "$val" ]]; then
        echo "$val"
        return
    fi

    # Fall back to [defaults]
    val=$(awk -v key="$key" '
        BEGIN { in_defaults=0; found="" }
        /^\[defaults\]/ { in_defaults=1; next }
        /^\[/ { in_defaults=0 }
        in_defaults && $0 ~ "^" key "[[:space:]]*=" {
            gsub(/.*=[[:space:]]*/, ""); found=$0
        }
        END { print found }
    ' "$CONFIG_PATH" 2>/dev/null || true)

    if [[ -n "$val" ]]; then
        echo "$val"
    else
        echo "$default_val"
    fi
}

# Collect all group names from the config
list_groups() {
    awk '
        /^\[\[group\]\]/ { in_group=1; next }
        in_group && /^name[[:space:]]*=/ {
            gsub(/.*=[[:space:]]*"/, ""); gsub(/".*/, "");
            print; in_group=0
        }
    ' "$CONFIG_PATH" 2>/dev/null || true
}

# --- Result collection ---

THRESHOLD_RESULTS=""
BENCHMARK_RESULTS=""
REGRESSIONS_DETECTED=0
WARNINGS_DETECTED=0
TIMESTAMP=$(date -u +"%Y-%m-%d %H:%M:%S UTC")

# Run threshold-based performance tests (mask perf-check)
run_threshold_tests() {
    echo "🔍 Running threshold performance tests..."

    local output_file
    output_file=$(mktemp)
    local exit_code=0

    cargo test --test interaction_performance_tests -- --test-threads=1 --nocapture \
        > "$output_file" 2>&1 || exit_code=$?

    if [[ $exit_code -ne 0 ]]; then
        REGRESSIONS_DETECTED=1
        THRESHOLD_RESULTS="❌ Threshold tests FAILED (exit code $exit_code)"
    else
        THRESHOLD_RESULTS="✅ All threshold tests passed"
    fi

    # Extract only test result lines (skip compiler warnings)
    local filtered
    filtered=$(grep -E '(^test |^running |^  test |^ok |^FAILED|^test result)' "$output_file" || true)

    if [[ -n "$filtered" ]]; then
        THRESHOLD_RESULTS="${THRESHOLD_RESULTS}
\`\`\`
${filtered}
\`\`\`"
    fi

    rm -f "$output_file"
    return $exit_code
}

# Run criterion benchmarks and compare against baseline
run_criterion_benchmarks() {
    local baseline="$1"
    local save_as="$2"

    echo "📊 Running criterion benchmarks..."

    local output_file
    output_file=$(mktemp)
    local exit_code=0

    local bench_args="--all-features"
    if [[ -n "$save_as" ]]; then
        bench_args="${bench_args} -- --save-baseline ${save_as}"
    elif [[ -n "$baseline" ]]; then
        bench_args="${bench_args} -- --baseline ${baseline}"
    fi

    # shellcheck disable=SC2086
    cargo bench ${bench_args} > "$output_file" 2>&1 || exit_code=$?

    BENCHMARK_RESULTS=$(cat "$output_file")
    rm -f "$output_file"

    return 0
}

# Analyse criterion output for regressions per group
analyse_criterion_output() {
    local output="$1"

    # Parse criterion lines that show changes
    # Format: "benchmark_name  time: [lower upper] change: [lower upper] (p = ...)"
    local regression_lines=""
    local warning_lines=""
    local improvement_lines=""

    while IFS= read -r line; do
        # Look for lines with percentage changes
        if echo "$line" | grep -qE 'change:.*[+-][0-9]+\.[0-9]+%'; then
            local change_pct
            change_pct=$(echo "$line" | grep -oE '[+-][0-9]+\.[0-9]+%' | head -1 | tr -d '%+')

            if [[ -z "$change_pct" ]]; then
                continue
            fi

            # Determine which group this benchmark belongs to
            local bench_name
            bench_name=$(echo "$line" | awk '{print $1}')
            local group="default"

            for g in $(list_groups); do
                if echo "$bench_name" | grep -qi "$g"; then
                    group="$g"
                    break
                fi
            done

            local max_regression
            max_regression=$(parse_threshold "$group" "max_regression_percent" "15.0")
            local warn_threshold
            warn_threshold=$(parse_threshold "$group" "warning_percent" "10.0")
            local improvement_threshold
            improvement_threshold=$(parse_threshold "$group" "min_improvement_percent" "5.0")

            # Compare (positive = regression, negative = improvement)
            if awk "BEGIN {exit !($change_pct > $max_regression)}"; then
                regression_lines="${regression_lines}
| ${bench_name} | ${group} | ${change_pct}% | ${max_regression}% | ❌ Regression |"
                REGRESSIONS_DETECTED=1
            elif awk "BEGIN {exit !($change_pct > $warn_threshold)}"; then
                warning_lines="${warning_lines}
| ${bench_name} | ${group} | ${change_pct}% | ${max_regression}% | ⚠️ Warning |"
                WARNINGS_DETECTED=1
            elif awk "BEGIN {exit !($change_pct < -$improvement_threshold)}"; then
                improvement_lines="${improvement_lines}
| ${bench_name} | ${group} | ${change_pct}% | — | ✅ Improved |"
            fi
        fi
    done <<< "$output"

    # Return the analysis table
    if [[ -n "$regression_lines" || -n "$warning_lines" || -n "$improvement_lines" ]]; then
        echo "| Benchmark | Group | Change | Threshold | Status |"
        echo "|-----------|-------|--------|-----------|--------|"
        echo "$regression_lines"
        echo "$warning_lines"
        echo "$improvement_lines"
    else
        echo "_No significant changes detected._"
    fi
}

# --- Report generation ---

generate_report() {
    local analysis="$1"

    local status_icon="✅"
    local status_text="No Regressions"
    if [[ $REGRESSIONS_DETECTED -gt 0 ]]; then
        status_icon="❌"
        status_text="Regressions Detected"
    elif [[ $WARNINGS_DETECTED -gt 0 ]]; then
        status_icon="⚠️"
        status_text="Warnings"
    fi

    cat > "$REPORT_PATH" << EOF
## ${status_icon} Performance Report — ${status_text}

**Timestamp**: ${TIMESTAMP}
**Baseline**: \`${BASELINE_NAME}\`
**Commit**: \`$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")\`
**Branch**: \`$(git branch --show-current 2>/dev/null || echo "unknown")\`

### Threshold Configuration

Thresholds are defined in [\`perf-thresholds.toml\`](perf-thresholds.toml).

| Group | Max Regression | Warning | Min Improvement |
|-------|---------------|---------|-----------------|
EOF

    for g in $(list_groups); do
        local max_r
        max_r=$(parse_threshold "$g" "max_regression_percent" "15.0")
        local warn_t
        warn_t=$(parse_threshold "$g" "warning_percent" "10.0")
        local min_i
        min_i=$(parse_threshold "$g" "min_improvement_percent" "5.0")
        echo "| ${g} | ${max_r}% | ${warn_t}% | ${min_i}% |" >> "$REPORT_PATH"
    done

    cat >> "$REPORT_PATH" << EOF

### Threshold Test Results

${THRESHOLD_RESULTS}

### Benchmark Comparison

${analysis}

EOF

    if [[ $REGRESSIONS_DETECTED -gt 0 ]]; then
        cat >> "$REPORT_PATH" << 'EOF'
### ❌ Action Required

Performance regressions exceed configured thresholds. Please review
the changes and either optimise the code or update the thresholds
in `perf-thresholds.toml` with justification in the PR description.

EOF
    fi

    cat >> "$REPORT_PATH" << EOF
---

<details>
<summary>📊 Full Benchmark Output</summary>

\`\`\`
$(echo "$BENCHMARK_RESULTS" | head -200)
\`\`\`

</details>

> Generated by [perf_alert.sh](scripts/perf_alert.sh) — see
> [perf-thresholds.toml](perf-thresholds.toml) for configuration.
EOF

    echo "📄 Report written to ${REPORT_PATH}"
}

# Generate JSON report for programmatic consumption
generate_json_report() {
    local analysis_status="pass"
    if [[ $REGRESSIONS_DETECTED -gt 0 ]]; then
        analysis_status="fail"
    elif [[ $WARNINGS_DETECTED -gt 0 ]]; then
        analysis_status="warn"
    fi

    cat > "$JSON_PATH" << EOF
{
  "timestamp": "${TIMESTAMP}",
  "baseline": "${BASELINE_NAME}",
  "commit": "$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")",
  "branch": "$(git branch --show-current 2>/dev/null || echo "unknown")",
  "status": "${analysis_status}",
  "regressions_detected": ${REGRESSIONS_DETECTED},
  "warnings_detected": ${WARNINGS_DETECTED},
  "config_path": "${CONFIG_PATH}",
  "groups": [
$(for g in $(list_groups); do
    local max_r
    max_r=$(parse_threshold "$g" "max_regression_percent" "15.0")
    local warn_t
    warn_t=$(parse_threshold "$g" "warning_percent" "10.0")
    echo "    {\"name\": \"${g}\", \"max_regression_percent\": ${max_r}, \"warning_percent\": ${warn_t}},"
done | sed '$ s/,$//')
  ]
}
EOF

    echo "📄 JSON report written to ${JSON_PATH}"
}

# --- Main ---

main() {
    echo "🚀 Gup Performance Alert System"
    echo "================================"
    echo ""

    if [[ ! -f "$CONFIG_PATH" ]]; then
        echo "⚠️  Config file not found: ${CONFIG_PATH}"
        echo "   Using built-in defaults."
    fi

    local threshold_exit=0
    local analysis="_Benchmarks skipped._"

    # Step 1: Run threshold tests
    if [[ "$SKIP_THRESHOLD_TESTS" != "true" ]]; then
        run_threshold_tests || threshold_exit=$?
    else
        THRESHOLD_RESULTS="_Threshold tests skipped._"
    fi

    # Step 2: Run criterion benchmarks
    if [[ "$SKIP_BENCHMARKS" != "true" ]]; then
        run_criterion_benchmarks "$BASELINE_NAME" "$SAVE_BASELINE" || true
        analysis=$(analyse_criterion_output "$BENCHMARK_RESULTS")
    fi

    # Step 3: Generate reports
    generate_report "$analysis"
    generate_json_report

    # Step 4: Summary
    echo ""
    echo "📋 Summary"
    echo "----------"
    if [[ $REGRESSIONS_DETECTED -gt 0 ]]; then
        echo "❌ Performance regressions detected!"
        if [[ "$FAIL_ON_REGRESSION" == "true" ]]; then
            echo "   Failing build as --fail-on-regression is set."
            exit 1
        fi
    elif [[ $WARNINGS_DETECTED -gt 0 ]]; then
        echo "⚠️  Performance warnings detected (within tolerance)."
    else
        echo "✅ No performance regressions."
    fi
}

main
