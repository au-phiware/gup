#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Runs a command with this checkout's cargo build environment (GUP-411):
#
#   scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1
#   scripts/with_cargo_env.sh mask perf-budget
#
# Same environment as `eval "$(scripts/cargo_env.sh)"`, as one plain command:
# no eval, no `cd`, no shell state carried between calls. Agents' shells start
# fresh for every command and inherit the environment of the checkout that
# started them, so a worktree agent calls this script by the worktree's path.
# The checkout is the one this script lives in, wherever it is called from.
set -euo pipefail
eval "$("$(dirname "${BASH_SOURCE[0]}")/cargo_env.sh")"
exec "$@"
