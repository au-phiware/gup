#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Dogfood suite: build every task binary against the parent gup checkout
# as an external crate, then run the suite runner. It executes each task,
# checks the PNG/SVG it produced, and reports PASS / XFAIL / FAIL / XPASS
# per check. Outputs and per-task logs go to /tmp/gup-dogfood/.
#
# Windowed tasks run in DOGFOOD_AUTO mode (scripted input + screenshot) and
# need a display. On a headless machine use `xvfb-run -a ./run_all.sh`, or
# skip them with `DOGFOOD_SKIP_WINDOWED=1 ./run_all.sh`.
# Run a subset with `DOGFOOD_ONLY=t1_timeseries,t2_bars ./run_all.sh`.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --bins
exec "${CARGO_TARGET_DIR:-target}/release/dogfood_check"
