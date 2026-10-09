#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# gup-core's browser smoke test (GUP-401, GUP-408): build the
# `crates/gup-core/wasm-size/scatter` harness in release for wasm32, bind it
# with wasm-bindgen and render it in headless Chromium through WebGPU on
# SwiftShader (scripts/browser_smoke.mjs). `mask wasm-browser` and the Visual
# regression workflow's browser job both run this script.
#
# Needs: the rust-toolchain.toml toolchain (with its wasm32 target), the
# wasm-bindgen CLI at the harness's Cargo.lock version, Node 22+, and a
# Chromium ($GUP_CHROMIUM, default `chromium`). Writes the browser's pixels
# to $CARGO_TARGET_DIR/visual-regression/gup_core/browser_scatter.png.

set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
harness="$root/crates/gup-core/wasm-size/scatter"
# The harness is its own workspace: without CARGO_TARGET_DIR its build would
# land in the harness directory, not the repository's target directory.
target="${CARGO_TARGET_DIR:-target}"
[[ $target == /* ]] || target="$root/$target"
export CARGO_TARGET_DIR="$target"

want=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$harness/Cargo.lock")
have=$(wasm-bindgen --version 2>/dev/null | cut -d' ' -f2 || true)
if [[ $have != "$want" ]]; then
  echo "wasm-browser: needs wasm-bindgen $want (the harness's Cargo.lock), found '${have:-none}':" >&2
  echo "  cargo install wasm-bindgen-cli --locked --version $want" >&2
  exit 1
fi

web=$(mktemp -d)
trap 'rm -rf "$web"' EXIT
(cd "$harness" && cargo build --quiet --release --target wasm32-unknown-unknown)
wasm-bindgen --target web --out-dir "$web" --out-name scatter \
  "$target/wasm32-unknown-unknown/release/gup_core_wasm_size_scatter.wasm"
cp "$harness/index.html" "$web/"
node "$root/scripts/browser_smoke.mjs" "$web" \
  "$target/visual-regression/gup_core/browser_scatter.png"
