#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Run the `gup` lib tests and every integration test that requires the
# `debug` feature (GUP-398). `cargo test` without the feature skips those
# targets silently, so the list is read from Cargo.toml rather than kept by
# hand. Extra arguments go to cargo before `--` (e.g. `--no-run`). Set
# XVFB_RUN= (empty) to run without Xvfb.
set -euo pipefail

mapfile -t gated < <(
  cargo metadata --no-deps --format-version 1 |
    jq -r '.packages[] | select(.name == "gup") | .targets[]
      | select(.kind == ["test"])
      | select((.["required-features"] // []) | index("debug"))
      | .name'
)
if [[ ${#gated[@]} -eq 0 ]]; then
  echo "test_debug_feature: found no debug-gated test targets; check the jq filter" >&2
  exit 1
fi

args=(-p gup --features debug --lib)
for name in "${gated[@]}"; do
  args+=(--test "$name")
done

echo "debug-gated tests: ${gated[*]}"
${XVFB_RUN-xvfb-run -a} cargo test "${args[@]}" "$@" -- --test-threads=1
