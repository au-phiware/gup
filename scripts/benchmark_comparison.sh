#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# benchmark_comparison.sh — Generate a side-by-side native vs WASM performance
# comparison report from two JSON benchmark result files.
#
# Usage:
#   ./scripts/benchmark_comparison.sh <native.json> <wasm.json> [output.md]
#
# Both input files must conform to the BenchSuite JSON schema produced by
# wasm_bench_native and the WASM benchmark runner.

set -euo pipefail

NATIVE_JSON="${1:?Usage: $0 <native.json> <wasm.json> [output.md]}"
WASM_JSON="${2:?Usage: $0 <native.json> <wasm.json> [output.md]}"
OUTPUT="${3:-/dev/stdout}"

PYTHON=""
for p in python3 python; do
  if command -v "$p" &>/dev/null; then
    PYTHON="$p"
    break
  fi
done

# Fallback: search common nix store locations
if [ -z "$PYTHON" ]; then
  for p in /nix/store/*/bin/python3; do
    if [ -x "$p" ]; then
      PYTHON="$p"
      break
    fi
  done
fi

if [ -z "$PYTHON" ]; then
  echo "Error: python3 or python is required for report generation" >&2
  exit 1
fi

"$PYTHON" - "$NATIVE_JSON" "$WASM_JSON" "$OUTPUT" <<'PYTHON_SCRIPT'
import json
import sys
from pathlib import Path

native_path = sys.argv[1]
wasm_path   = sys.argv[2]
output_path = sys.argv[3]

with open(native_path) as f:
    native = json.load(f)
with open(wasm_path) as f:
    wasm = json.load(f)

# Index results by name
native_by_name = {r["name"]: r for r in native["results"]}
wasm_by_name   = {r["name"]: r for r in wasm["results"]}

all_names = sorted(set(list(native_by_name.keys()) + list(wasm_by_name.keys())))

lines = []
lines.append("# Gup Performance Comparison: Native vs WASM")
lines.append("")
lines.append(f"**Generated**: {native.get('timestamp', 'N/A')}")
lines.append(f"**Native platform**: {native.get('platform', 'N/A')}")
lines.append(f"**WASM platform**: {wasm.get('platform', 'N/A')}")
if wasm.get("user_agent"):
    lines.append(f"**Browser**: {wasm['user_agent']}")
lines.append("")

# Group benchmarks by category
categories = {}
for name in all_names:
    cat = name.split("/")[0] if "/" in name else "other"
    categories.setdefault(cat, []).append(name)

for cat, names in sorted(categories.items()):
    title = cat.replace("_", " ").title()
    lines.append(f"## {title}")
    lines.append("")
    lines.append("| Benchmark | Native Mean (ms) | WASM Mean (ms) | Ratio (WASM/Native) | Delta |")
    lines.append("|-----------|------------------:|---------------:|--------------------:|------:|")

    for name in names:
        nr = native_by_name.get(name)
        wr = wasm_by_name.get(name)
        short = name.split("/", 1)[1] if "/" in name else name

        n_mean = f"{nr['mean_ms']:.3f}" if nr else "—"
        w_mean = f"{wr['mean_ms']:.3f}" if wr else "—"

        if nr and wr and nr["mean_ms"] > 0:
            ratio = wr["mean_ms"] / nr["mean_ms"]
            delta_pct = (wr["mean_ms"] - nr["mean_ms"]) / nr["mean_ms"] * 100
            ratio_str = f"{ratio:.2f}x"
            delta_str = f"{delta_pct:+.1f}%"
        else:
            ratio_str = "—"
            delta_str = "—"

        lines.append(f"| {short} | {n_mean} | {w_mean} | {ratio_str} | {delta_str} |")

    lines.append("")

# Summary statistics
if native_by_name and wasm_by_name:
    common = set(native_by_name.keys()) & set(wasm_by_name.keys())
    if common:
        ratios = []
        for name in common:
            nm = native_by_name[name]["mean_ms"]
            wm = wasm_by_name[name]["mean_ms"]
            if nm > 0:
                ratios.append(wm / nm)

        if ratios:
            avg_ratio = sum(ratios) / len(ratios)
            min_ratio = min(ratios)
            max_ratio = max(ratios)

            lines.append("## Summary")
            lines.append("")
            lines.append(f"- **Benchmarks compared**: {len(common)}")
            lines.append(f"- **Average WASM/Native ratio**: {avg_ratio:.2f}x")
            lines.append(f"- **Best case (closest to native)**: {min_ratio:.2f}x")
            lines.append(f"- **Worst case**: {max_ratio:.2f}x")
            lines.append("")

            if avg_ratio <= 1.5:
                lines.append("> WASM performance is **excellent** — within 1.5x of native.")
            elif avg_ratio <= 3.0:
                lines.append("> WASM performance is **good** — within 3x of native.")
            elif avg_ratio <= 5.0:
                lines.append("> WASM performance is **acceptable** — within 5x of native.")
            else:
                lines.append("> WASM performance **needs investigation** — more than 5x slower than native.")
            lines.append("")

report = "\n".join(lines) + "\n"

if output_path == "/dev/stdout":
    print(report, end="")
else:
    Path(output_path).write_text(report)
    print(f"Report written to {output_path}", file=sys.stderr)
PYTHON_SCRIPT
