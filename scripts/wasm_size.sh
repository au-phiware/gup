#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# gup-core's gzipped WASM cost (GUP-401, RFC-001 §12 risk 10), with its budget
# enforced (GUP-417). Builds the two `crates/gup-core/wasm-size` harnesses in
# release for wasm32-unknown-unknown: bare wgpu, and the reference scatter
# through gup-core. Each goes through `wasm-bindgen --target web` without name
# or producers sections, then `gzip -9`. Prints each size, gup-core's cost over
# bare wgpu and the bundled Inter subset, then `metric` lines for
# scripts/perf_budget.sh, and fails when gup-core costs more than the budget
# over bare wgpu: 400 KB gz, read as 400,000 B (the stricter reading of
# RFC-001's "≤ +400 KB gz"). `mask wasm-size` and the Visual regression
# workflow's browser job both run this script.
#
#   GUP_WASM_OVER_WGPU_MAX_GZ  the budget in bytes (default 400000); lower it
#                              to see the check fail
#
# Needs: the rust-toolchain.toml toolchain with its wasm32 target, and the
# wasm-bindgen CLI at the harnesses' Cargo.lock version. Delete
# $CARGO_TARGET_DIR/wasm32-unknown-unknown/release afterwards if disk is tight.

set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
harnesses="$root/crates/gup-core/wasm-size"
budget=${GUP_WASM_OVER_WGPU_MAX_GZ:-400000}
# The harnesses are their own workspaces: point them at this target directory.
target="${CARGO_TARGET_DIR:-target}"
[[ $target == /* ]] || target="$root/$target"
export CARGO_TARGET_DIR="$target"
release="$target/wasm32-unknown-unknown/release"

lock_version() {
  sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$1/Cargo.lock"
}
want=$(lock_version "$harnesses/scatter")
if [[ $(lock_version "$harnesses/baseline") != "$want" ]]; then
  echo "wasm-size: the harnesses' Cargo.lock files pin different wasm-bindgen versions" >&2
  exit 1
fi
have=$(wasm-bindgen --version 2>/dev/null | cut -d' ' -f2 || true)
if [[ $have != "$want" ]]; then
  echo "wasm-size: needs wasm-bindgen $want (the harnesses' Cargo.lock), found '${have:-none}':" >&2
  echo "  cargo install wasm-bindgen-cli --locked --version $want" >&2
  exit 1
fi

out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
measure() { # name, crate dir, crate file name
  local name=$1 dir=$2 file=$3
  (cd "$harnesses/$dir" && cargo build --quiet --release --target wasm32-unknown-unknown)
  wasm-bindgen --target web --remove-name-section --remove-producers-section \
    --out-dir "$out/$name" --out-name "$name" "$release/$file.wasm"
  local wasm="$out/$name/${name}_bg.wasm"
  raw=$(stat -c %s "$wasm")
  gz=$(gzip -9 -c "$wasm" | wc -c)
  printf '%-26s %10d B raw %9d B gz\n' "$name" "$raw" "$gz"
}
measure wgpu baseline gup_core_wasm_size_baseline
base_raw=$raw base_gz=$gz
measure gup-core-scatter scatter gup_core_wasm_size_scatter
over_raw=$((raw - base_raw)) over_gz=$((gz - base_gz))
printf '%-26s %10d B raw %9d B gz\n' "(gup-core over wgpu)" "$over_raw" "$over_gz"
font="$root/crates/gup-text/fonts/Inter-Regular-Subset.ttf"
printf '%-26s %10d B raw %9d B gz\n' "(Inter subset, bundled)" \
  "$(stat -c %s "$font")" "$(gzip -9 -c "$font" | wc -c)"

echo "metric wasm.wgpu.gz_bytes $base_gz"
echo "metric wasm.scatter.gz_bytes $gz"
echo "metric wasm.over_wgpu.gz_bytes $over_gz"

if ((over_gz > budget)); then
  echo "wasm-size: FAIL: gup-core costs $over_gz B gz over bare wgpu, over the $budget B budget (RFC-001 §12 risk 10)" >&2
  exit 1
fi
echo "wasm-size: pass: gup-core costs $over_gz B gz over bare wgpu (budget $budget B)"
