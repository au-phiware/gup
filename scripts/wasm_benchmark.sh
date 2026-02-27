#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# wasm_benchmark.sh — Build, run, and compare WASM benchmarks.
#
# Modes:
#   build    Compile the library to WASM and copy to bench runner
#   native   Run native benchmarks and save results to JSON
#   serve    Build WASM and start a local server for the browser runner
#   compare  Generate a comparison report from native + WASM result files
#   all      Run native benchmarks, build WASM, and remind to collect WASM data
#
# Usage:
#   ./scripts/wasm_benchmark.sh build
#   ./scripts/wasm_benchmark.sh native [output.json]
#   ./scripts/wasm_benchmark.sh serve  [port]
#   ./scripts/wasm_benchmark.sh compare <native.json> <wasm.json> [report.md]
#   ./scripts/wasm_benchmark.sh all

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCH_DIR="$PROJECT_ROOT/benches/wasm"
RESULTS_DIR="$PROJECT_ROOT/target/bench-results"

mkdir -p "$RESULTS_DIR"

cmd_build() {
    echo "==> Building WASM package with wasm-pack..."
    cd "$PROJECT_ROOT"
    wasm-pack build --target web --out-dir "$BENCH_DIR/pkg" --release

    echo "==> WASM package built to $BENCH_DIR/pkg/"
    echo "    Open $BENCH_DIR/index.html in a WebGPU-enabled browser to run benchmarks."
}

cmd_native() {
    local output="${1:-$RESULTS_DIR/native_$(date +%Y%m%d_%H%M%S).json}"
    echo "==> Running native benchmarks (release build)..."
    cd "$PROJECT_ROOT"
    cargo run --release --bin wasm_bench_native > "$output"
    echo "==> Native results saved to $output"
}

cmd_serve() {
    local port="${1:-8081}"
    cmd_build

    echo "==> Serving benchmark runner at http://localhost:$port"
    echo "    Open in a WebGPU-enabled browser (e.g. chromium-webgpu)"
    echo "    After running benchmarks, download the JSON results."
    echo ""
    echo "    Press Ctrl+C to stop the server."

    if command -v miniserve &>/dev/null; then
        miniserve --index index.html --port "$port" "$BENCH_DIR"
    elif command -v python3 &>/dev/null; then
        cd "$BENCH_DIR"
        python3 -m http.server "$port"
    else
        echo "Error: no suitable HTTP server found (miniserve or python3)" >&2
        exit 1
    fi
}

cmd_compare() {
    local native_json="${1:?Usage: $0 compare <native.json> <wasm.json> [report.md]}"
    local wasm_json="${2:?Usage: $0 compare <native.json> <wasm.json> [report.md]}"
    local output="${3:-$RESULTS_DIR/comparison_$(date +%Y%m%d_%H%M%S).md}"
    "$SCRIPT_DIR/benchmark_comparison.sh" "$native_json" "$wasm_json" "$output"
}

cmd_all() {
    echo "=== Gup WASM Performance Benchmark Suite ==="
    echo ""

    # Step 1: Native benchmarks
    local native_output="$RESULTS_DIR/native_$(date +%Y%m%d_%H%M%S).json"
    cmd_native "$native_output"
    echo ""

    # Step 2: Build WASM
    cmd_build
    echo ""

    # Step 3: Instructions
    echo "=== Next Steps ==="
    echo ""
    echo "1. Open the benchmark runner in a WebGPU browser:"
    echo "   chromium-webgpu --app=file://$BENCH_DIR/index.html"
    echo ""
    echo "2. Click 'Run Benchmarks' and wait for completion."
    echo "3. Click 'Download JSON' to save the WASM results."
    echo "4. Generate comparison report:"
    echo "   $0 compare $native_output <wasm_results.json>"
    echo ""
}

cmd_help() {
    echo "Usage: $0 <command> [args...]"
    echo ""
    echo "Commands:"
    echo "  build              Build WASM package for benchmark runner"
    echo "  native [out.json]  Run native benchmarks, save JSON results"
    echo "  serve  [port]      Build WASM and serve the benchmark runner"
    echo "  compare <n> <w>    Compare native (n) and WASM (w) JSON results"
    echo "  all                Run native, build WASM, and show next steps"
    echo "  help               Show this help message"
}

case "${1:-help}" in
    build)   cmd_build ;;
    native)  shift; cmd_native "${@:-}" ;;
    serve)   shift; cmd_serve "${@:-}" ;;
    compare) shift; cmd_compare "$@" ;;
    all)     cmd_all ;;
    help|*)  cmd_help ;;
esac
