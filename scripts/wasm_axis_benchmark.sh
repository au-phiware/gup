#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# wasm_axis_benchmark.sh — Run axis performance benchmarks in headless Chrome.
#
# Usage:
#   ./scripts/wasm_axis_benchmark.sh [output.json]
#
# Prerequisites:
#   - wasm-pack
#   - Chromium with WebGPU support (chromium-webgpu or equivalent)
#   - Python 3 (for HTTP server)
#
# Exit codes:
#   0 — All benchmarks within 2ms budget
#   1 — One or more benchmarks exceed budget
#   2 — Build or setup failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCH_DIR="$PROJECT_ROOT/benches/wasm"
RESULTS_DIR="$PROJECT_ROOT/target/bench-results"
OUTPUT="${1:-$RESULTS_DIR/wasm_axis_$(date +%Y%m%d_%H%M%S).json}"
BUDGET_MS=2.0

mkdir -p "$RESULTS_DIR"

echo "==> Building WASM package..."
cd "$PROJECT_ROOT"
wasm-pack build --target web --out-dir "$BENCH_DIR/pkg" --release 2>&1

echo "==> Starting HTTP server..."
cd "$BENCH_DIR"
PORT=8099
python3 -m http.server "$PORT" &>/dev/null &
SERVER_PID=$!
trap 'kill $SERVER_PID 2>/dev/null; rm -f /tmp/gup_axis_bench_*.js' EXIT
sleep 1

echo "==> Running axis benchmarks in headless Chromium..."
# Create a puppeteer-like script that loads the page and collects results
cat > /tmp/gup_axis_bench_runner.js << 'SCRIPT'
// This script is executed by chromium --headless to collect benchmark results.
// It relies on the ?autorun parameter triggering automatic benchmark execution.
// Results are collected via window.__gupAxisResults and printed to stdout as JSON.
SCRIPT

# Use chromium's built-in headless mode with JavaScript evaluation
RESULT_FILE="/tmp/gup_axis_bench_result_$$.json"

# Run benchmarks via chromium headless with virtual-time-budget
# The page auto-runs benchmarks when ?autorun is in the URL
# We give it 30 seconds of virtual time to complete
chromium --headless=new \
  --disable-gpu \
  --no-sandbox \
  --run-all-compositor-stages-before-draw \
  --virtual-time-budget=30000 \
  --dump-dom \
  "http://localhost:$PORT/axis_benchmarks.html?autorun" \
  2>/dev/null > /tmp/gup_axis_dom_$$.html

# Extract the JSON from the page's json-output element
RESULTS=$(python3 -c "
import sys, html.parser, json

class JSONExtractor(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.in_json = False
        self.json_data = ''
    def handle_starttag(self, tag, attrs):
        if tag == 'pre' and ('id', 'json-output') in attrs:
            self.in_json = True
    def handle_endtag(self, tag):
        if tag == 'pre' and self.in_json:
            self.in_json = False
    def handle_data(self, data):
        if self.in_json:
            self.json_data += data

with open('/tmp/gup_axis_dom_$$.html') as f:
    content = f.read()

parser = JSONExtractor()
parser.feed(content)

if parser.json_data.strip():
    # Validate it's valid JSON
    data = json.loads(parser.json_data)
    print(json.dumps(data, indent=2))
else:
    print('{}', file=sys.stderr)
    sys.exit(1)
" 2>&1)

if [ -z "$RESULTS" ] || [ "$RESULTS" = "{}" ]; then
    echo "❌ Failed to collect benchmark results from headless Chrome."
    echo "   The benchmarks may not have completed within the time budget."
    echo "   Try running manually: chromium-webgpu http://localhost:$PORT/axis_benchmarks.html"
    exit 2
fi

echo "$RESULTS" > "$OUTPUT"
echo "==> Results saved to $OUTPUT"
echo ""

# Parse and display results
python3 -c "
import json, sys

with open('$OUTPUT') as f:
    suite = json.load(f)

budget = $BUDGET_MS
violations = []

print('## WebAssembly Axis Performance Report')
print()
print(f'Platform: {suite[\"platform\"]}')
print(f'Timestamp: {suite[\"timestamp\"]}')
print(f'User Agent: {suite.get(\"user_agent\", \"N/A\")}')
print(f'Budget: {budget} ms')
print()
print(f'{\"Benchmark\":<30} {\"Median\":>10} {\"Mean\":>10} {\"Min\":>10} {\"Max\":>10} {\"Status\":>8}')
print('-' * 78)

for r in suite['results']:
    status = '✅' if r['median_ms'] < budget else '❌'
    if r['median_ms'] >= budget:
        violations.append(r['name'])
    print(f'{r[\"name\"]:<30} {r[\"median_ms\"]:>9.4f}ms {r[\"mean_ms\"]:>9.4f}ms {r[\"min_ms\"]:>9.4f}ms {r[\"max_ms\"]:>9.4f}ms {status:>8}')

print()
if violations:
    print(f'❌ {len(violations)} benchmark(s) exceed {budget}ms budget: {violations}')
    sys.exit(1)
else:
    print(f'✅ All {len(suite[\"results\"])} benchmarks within {budget}ms budget')
"
