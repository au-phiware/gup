#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Generate gallery thumbnails for GUP examples.
#
# Usage:
#   scripts/generate_gallery.sh                 # all non-skipped examples
#   scripts/generate_gallery.sh export_png 03_line_chart  # specific examples
#
# Each eligible example is run with the GUP_SCREENSHOT_PATH environment
# variable pointing to its thumbnail output location.  Examples that detect
# this variable render a single frame offscreen and exit.  The script reads
# the skip list from scripts/gallery_config.toml.
#
# Requires: cargo, toml parsing (grep-based), timeout

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG="$SCRIPT_DIR/gallery_config.toml"

# Read gallery dimensions from config (defaults: 640x480).
THUMB_WIDTH=$(grep '^thumb_width' "$CONFIG" | head -1 | sed 's/.*= *//')
THUMB_HEIGHT=$(grep '^thumb_height' "$CONFIG" | head -1 | sed 's/.*= *//')
OUTPUT_DIR="$REPO_ROOT/$(grep '^output_dir' "$CONFIG" | head -1 | sed 's/.*= *"//; s/"//')"

THUMB_WIDTH="${THUMB_WIDTH:-640}"
THUMB_HEIGHT="${THUMB_HEIGHT:-480}"

mkdir -p "$OUTPUT_DIR"

# Parse example names and skip flags from TOML.
# Produces lines of: name|skip|source
parse_config() {
  awk '
    /^\[\[examples\]\]/ { name=""; skip="false"; source="" }
    /^name *= */ { gsub(/.*= *"|"/, ""); name=$0 }
    /^skip *= */ { gsub(/.*= */, ""); skip=$0 }
    /^source *= */ { gsub(/.*= *"|"/, ""); source=$0 }
    /^(skip_reason|description|category) *= */ { next }
    /^\[/ && name != "" { print name "|" skip "|" source; name="" }
    END { if (name != "") print name "|" skip "|" source }
  ' "$CONFIG"
}

# Build the list of examples to process.
declare -A SKIP_MAP
declare -A SOURCE_MAP
while IFS='|' read -r ename eskip esource; do
  SKIP_MAP["$ename"]="$eskip"
  SOURCE_MAP["$ename"]="$esource"
done < <(parse_config)

# If specific examples were requested on the command line, use those.
# Otherwise process all non-skipped examples from the config.
if [[ $# -gt 0 ]]; then
  EXAMPLES=("$@")
else
  EXAMPLES=()
  for ename in "${!SKIP_MAP[@]}"; do
    if [[ "${SKIP_MAP[$ename]}" != "true" ]]; then
      EXAMPLES+=("$ename")
    fi
  done
fi

# Sort for deterministic ordering.
IFS=$'\n' EXAMPLES=($(sort <<<"${EXAMPLES[*]}")); unset IFS

TOTAL=${#EXAMPLES[@]}
PASS=0
FAIL=0
SKIP=0
CACHED=0
FAILED_EXAMPLES=()

echo "=== GUP Gallery Thumbnail Generation ==="
echo "Output:     $OUTPUT_DIR"
echo "Dimensions: ${THUMB_WIDTH}×${THUMB_HEIGHT}"
echo "Examples:   $TOTAL"
echo ""

for ename in "${EXAMPLES[@]}"; do
  # Check skip status.
  if [[ "${SKIP_MAP[$ename]:-}" == "true" ]]; then
    echo "SKIP  $ename (in skip list)"
    SKIP=$((SKIP + 1))
    continue
  fi

  THUMB_PATH="$OUTPUT_DIR/${ename}.png"

  # Cache check: skip if thumbnail exists and is newer than source.
  SOURCE_FILE="$REPO_ROOT/${SOURCE_MAP[$ename]:-}"
  if [[ -f "$THUMB_PATH" && -n "$SOURCE_FILE" && -f "$SOURCE_FILE" ]]; then
    if [[ "$THUMB_PATH" -nt "$SOURCE_FILE" ]]; then
      echo "CACHE $ename"
      CACHED=$((CACHED + 1))
      PASS=$((PASS + 1))
      continue
    fi
  fi

  echo -n "RUN   $ename ... "

  # Run the example with the screenshot environment variable.
  if GUP_SCREENSHOT_PATH="$THUMB_PATH" \
     GUP_SCREENSHOT_WIDTH="$THUMB_WIDTH" \
     GUP_SCREENSHOT_HEIGHT="$THUMB_HEIGHT" \
     timeout 60 cargo run --release --example "$ename" 2>/tmp/gallery_"$ename".log; then
    if [[ -f "$THUMB_PATH" ]]; then
      SIZE=$(stat -c%s "$THUMB_PATH" 2>/dev/null || stat -f%z "$THUMB_PATH" 2>/dev/null || echo "?")
      echo "OK (${SIZE} bytes)"
      PASS=$((PASS + 1))
    else
      echo "FAIL (no thumbnail produced)"
      FAIL=$((FAIL + 1))
      FAILED_EXAMPLES+=("$ename")
    fi
  else
    echo "FAIL (exit code $?)"
    FAIL=$((FAIL + 1))
    FAILED_EXAMPLES+=("$ename")
    # Show last few lines of stderr for diagnosis.
    tail -5 /tmp/gallery_"$ename".log 2>/dev/null | sed 's/^/       /'
  fi
done

echo ""
echo "=== Summary ==="
echo "Pass:   $PASS"
echo "Fail:   $FAIL"
echo "Skip:   $SKIP"
echo "Cached: $CACHED"
echo "Total:  $TOTAL"

if [[ $FAIL -gt 0 ]]; then
  echo ""
  echo "Failed examples:"
  for fe in "${FAILED_EXAMPLES[@]}"; do
    echo "  - $fe"
  done
  exit 1
fi

exit 0
