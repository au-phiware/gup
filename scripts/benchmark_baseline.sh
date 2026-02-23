#!/usr/bin/env bash
# Copyright (C) 2025 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# Benchmark baseline management script for GUP-162
# Provides commands to save, compare, and reset Criterion baselines

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CRITERION_DIR="${PROJECT_ROOT}/target/criterion"

usage() {
    cat << EOF
Usage: $0 <command> [options]

Commands:
    save <name>         Save current benchmark results as a named baseline
    compare <baseline>  Compare current results against a named baseline
    list                List all available baselines
    delete <name>       Delete a named baseline
    reset               Delete all baselines and start fresh

Examples:
    $0 save main                     # Save current results as 'main' baseline
    $0 compare main                  # Compare against 'main' baseline
    $0 save feature-branch           # Save baseline for feature branch
    $0 list                          # Show all saved baselines
    $0 delete old-baseline           # Remove old baseline
    $0 reset                         # Clear all baselines

Pattern Benchmarks Only:
    $0 save-pattern main             # Save only pattern benchmarks
    $0 compare-pattern main          # Compare only pattern benchmarks

EOF
    exit 1
}

save_baseline() {
    local name="$1"
    echo "📊 Saving benchmark baseline: ${name}"
    cargo bench --all-features -- --save-baseline "${name}"
    echo "✅ Baseline '${name}' saved to ${CRITERION_DIR}"
}

save_pattern_baseline() {
    local name="$1"
    echo "🎨 Saving pattern benchmark baseline: ${name}"
    cargo bench --bench pattern_performance_benchmarks -- --save-baseline "${name}"
    echo "✅ Pattern baseline '${name}' saved"
}

compare_baseline() {
    local baseline="$1"
    if [ ! -d "${CRITERION_DIR}/${baseline}" ]; then
        echo "❌ Baseline '${baseline}' not found. Available baselines:"
        list_baselines
        exit 1
    fi
    
    echo "📊 Comparing against baseline: ${baseline}"
    cargo bench --all-features -- --baseline "${baseline}"
}

compare_pattern_baseline() {
    local baseline="$1"
    echo "🎨 Comparing pattern benchmarks against baseline: ${baseline}"
    cargo bench --bench pattern_performance_benchmarks -- --baseline "${baseline}"
}

list_baselines() {
    if [ ! -d "${CRITERION_DIR}" ]; then
        echo "No baselines found (${CRITERION_DIR} doesn't exist)"
        return
    fi
    
    echo "📊 Available baselines:"
    # List directories in criterion folder (each benchmark group)
    for benchmark_dir in "${CRITERION_DIR}"/*; do
        if [ -d "${benchmark_dir}" ]; then
            benchmark_name=$(basename "${benchmark_dir}")
            echo ""
            echo "  ${benchmark_name}:"
            # List baseline subdirectories
            if [ -d "${benchmark_dir}/base" ]; then
                echo "    - base (default baseline)"
            fi
            for baseline_dir in "${benchmark_dir}"/*; do
                if [ -d "${baseline_dir}" ] && [ "$(basename "${baseline_dir}")" != "base" ]; then
                    baseline_name=$(basename "${baseline_dir}")
                    echo "    - ${baseline_name}"
                fi
            done
        fi
    done
}

delete_baseline() {
    local name="$1"
    echo "🗑️  Deleting baseline: ${name}"
    
    deleted=0
    for benchmark_dir in "${CRITERION_DIR}"/*; do
        if [ -d "${benchmark_dir}/${name}" ]; then
            rm -rf "${benchmark_dir}/${name}"
            deleted=$((deleted + 1))
        fi
    done
    
    if [ ${deleted} -gt 0 ]; then
        echo "✅ Deleted baseline '${name}' from ${deleted} benchmark(s)"
    else
        echo "⚠️  Baseline '${name}' not found"
    fi
}

reset_baselines() {
    echo "⚠️  This will delete ALL benchmark baselines."
    read -p "Are you sure? (yes/no): " confirm
    
    if [ "${confirm}" = "yes" ]; then
        if [ -d "${CRITERION_DIR}" ]; then
            rm -rf "${CRITERION_DIR}"
            echo "✅ All baselines cleared"
        else
            echo "No baselines to clear"
        fi
    else
        echo "❌ Reset cancelled"
    fi
}

main() {
    cd "${PROJECT_ROOT}"
    
    if [ $# -eq 0 ]; then
        usage
    fi
    
    command="$1"
    shift
    
    case "${command}" in
        save)
            if [ $# -ne 1 ]; then
                echo "Error: 'save' requires a baseline name"
                usage
            fi
            save_baseline "$1"
            ;;
        save-pattern)
            if [ $# -ne 1 ]; then
                echo "Error: 'save-pattern' requires a baseline name"
                usage
            fi
            save_pattern_baseline "$1"
            ;;
        compare)
            if [ $# -ne 1 ]; then
                echo "Error: 'compare' requires a baseline name"
                usage
            fi
            compare_baseline "$1"
            ;;
        compare-pattern)
            if [ $# -ne 1 ]; then
                echo "Error: 'compare-pattern' requires a baseline name"
                usage
            fi
            compare_pattern_baseline "$1"
            ;;
        list)
            list_baselines
            ;;
        delete)
            if [ $# -ne 1 ]; then
                echo "Error: 'delete' requires a baseline name"
                usage
            fi
            delete_baseline "$1"
            ;;
        reset)
            reset_baselines
            ;;
        *)
            echo "Unknown command: ${command}"
            usage
            ;;
    esac
}

main "$@"
