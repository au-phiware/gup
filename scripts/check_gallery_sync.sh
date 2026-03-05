#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Check that gallery_config.toml, examples/INDEX.md, and Cargo.toml
# [[example]] entries are in sync.
#
# Compares the three sources and reports examples present in one but
# missing from another.  Exits non-zero if any drift is detected.
#
# Usage:
#   scripts/check_gallery_sync.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG="$SCRIPT_DIR/gallery_config.toml"
INDEX="$REPO_ROOT/examples/INDEX.md"
CARGO_TOML="$REPO_ROOT/Cargo.toml"

# ── helpers ──────────────────────────────────────────────────────────

# Print a sorted, deduplicated list to a temp file.
to_sorted_file() {
  sort -u > "$1"
}

# ── 1. Cargo examples ───────────────────────────────────────────────
# The effective set is the union of:
#   a) auto-discovered  examples/*.rs  (file stems)
#   b) explicit [[example]] entries in Cargo.toml  (covers subdirs)
CARGO_EXAMPLES=$(mktemp)
{
  # (a) auto-discovered top-level examples
  find "$REPO_ROOT/examples" -maxdepth 1 -name '*.rs' -printf '%f\n' \
    | sed 's/\.rs$//'

  # (b) explicit [[example]] names from Cargo.toml
  grep -A1 '^\[\[example\]\]' "$CARGO_TOML" \
    | grep 'name *= *"' \
    | sed 's/.*name *= *"//; s/".*//'
} | to_sorted_file "$CARGO_EXAMPLES"

# ── 2. gallery_config.toml names ────────────────────────────────────
GALLERY_EXAMPLES=$(mktemp)
grep '^name *= *"' "$CONFIG" \
  | sed 's/.*name *= *"//; s/".*//' \
  | to_sorted_file "$GALLERY_EXAMPLES"

# ── 3. examples/INDEX.md names ──────────────────────────────────────
# Names appear in the first column of markdown tables as `name`.
INDEX_EXAMPLES=$(mktemp)
grep -oP '(?<=\| `)[^`]+(?=` )' "$INDEX" \
  | to_sorted_file "$INDEX_EXAMPLES"

# ── compare ──────────────────────────────────────────────────────────
DRIFT=0

report_diff() {
  local label="$1" file_a="$2" file_b="$3"
  local diff_items
  diff_items=$(comm -23 "$file_a" "$file_b")
  if [[ -n "$diff_items" ]]; then
    echo "$label"
    echo "$diff_items" | sed 's/^/  - /'
    echo ""
    DRIFT=$((DRIFT + $(echo "$diff_items" | wc -l)))
  fi
}

echo "=== Gallery Config Sync Check ==="
echo ""

report_diff \
  "In Cargo examples but NOT in gallery_config.toml:" \
  "$CARGO_EXAMPLES" "$GALLERY_EXAMPLES"

report_diff \
  "In gallery_config.toml but NOT a Cargo example:" \
  "$GALLERY_EXAMPLES" "$CARGO_EXAMPLES"

report_diff \
  "In Cargo examples but NOT in examples/INDEX.md:" \
  "$CARGO_EXAMPLES" "$INDEX_EXAMPLES"

report_diff \
  "In examples/INDEX.md but NOT a Cargo example:" \
  "$INDEX_EXAMPLES" "$CARGO_EXAMPLES"

report_diff \
  "In gallery_config.toml but NOT in examples/INDEX.md:" \
  "$GALLERY_EXAMPLES" "$INDEX_EXAMPLES"

report_diff \
  "In examples/INDEX.md but NOT in gallery_config.toml:" \
  "$INDEX_EXAMPLES" "$GALLERY_EXAMPLES"

# ── cleanup ──────────────────────────────────────────────────────────
rm -f "$CARGO_EXAMPLES" "$GALLERY_EXAMPLES" "$INDEX_EXAMPLES"

# ── summary ──────────────────────────────────────────────────────────
if [[ $DRIFT -eq 0 ]]; then
  echo "All three sources are in sync."
  exit 0
else
  echo "Drift detected: $DRIFT total inconsistencies."
  exit 1
fi
