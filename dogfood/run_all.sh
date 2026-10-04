#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Dogfood regression run: builds every task binary against the parent gup
# checkout as an external crate and writes outputs to /tmp/gup-dogfood/.
# Windowed tasks run in DOGFOOD_AUTO mode (scripted input + screenshot).
set -u
cd "$(dirname "$0")"
mkdir -p /tmp/gup-dogfood
cargo build --release --bins || exit 1
B=target/release
run() { echo "== $*"; "$@" 2>&1 | grep -v -E '^\s*$|wgpu_hal|vkCreate' | tail -5; echo "   exit=${PIPESTATUS[0]}"; }
run $B/gen_data
run $B/t0_smoke
run $B/t1_timeseries
run $B/t2_bars
run $B/t3_scatter_png
run env RAW_MACRO=1 $B/t6_wgsl   # expected: panic (duplicate uniforms struct)
run $B/t6_wgsl
run $B/t5_stream                 # expected: blank PNGs (stream never render-ready)
DOGFOOD_AUTO=1 run $B/t3_scatter_window
DOGFOOD_AUTO=1 run $B/t4_linked
DOGFOOD_AUTO=1 run $B/t5_live
ls -1 /tmp/gup-dogfood/*.png
