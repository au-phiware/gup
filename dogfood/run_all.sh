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
# need a display. By default the suite runs headless through
# ../scripts/headless_env.sh --run (lavapipe, no WAYLAND_DISPLAY, a 1920x1080
# Xvfb screen). `DOGFOOD_DISPLAY=1` uses the real display instead, and
# `DOGFOOD_SKIP_WINDOWED=1` skips the windowed tasks (and the wrapper); an
# environment that is already headless (`GUP_HEADLESS=1`) is not wrapped twice.
# Run a subset with `DOGFOOD_ONLY=t1_timeseries,t2_bars ./run_all.sh`.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --bins
check="${CARGO_TARGET_DIR:-target}/release/dogfood_check"
if [[ ${DOGFOOD_SKIP_WINDOWED:-} == 1 || ${DOGFOOD_DISPLAY:-} == 1 || ${GUP_HEADLESS:-} == 1 ]]; then
  exec "$check"
fi
exec ../scripts/headless_env.sh --run "$check"
