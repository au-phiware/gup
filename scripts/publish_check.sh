#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later

# The publish dry run behind `mask publish-check` (GUP-589, GUP-603). It never
# publishes: `--dry-run` is the point of it.
#
# Cargo keeps the packages it verifies, and the local registry overlay of the
# members not yet on crates.io (`package/tmp-registry`), under the build
# directory (`CARGO_BUILD_BUILD_DIR`). That directory is shared by every
# checkout on a machine (GUP-411), and the overlay's path is the same in all of
# them, so one checkout's dry run of version X could meet another's. A failure
# (`no hash listed for gup-core-macros`) was seen after such a run. This step
# therefore uses a build directory of its own, inside this checkout's target
# directory, so `package/` is never shared.
#
# Cost: the verification builds (Bevy included) are not shared with other
# checkouts or with `mask build`, so the first run in a checkout compiles them
# cold (about 5 minutes); later runs reuse the checkout's own directory and
# take about a minute. The overlay itself is cleared before every run.
#
#   scripts/publish_check.sh
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here"

dir="${CARGO_TARGET_DIR:-$here/target}/publish-check"
mkdir -p "$dir"
rm -rf "$dir/package/tmp-registry" "$dir/package/tmp-crate"

CARGO_BUILD_BUILD_DIR=$dir exec cargo publish --workspace --dry-run --locked
