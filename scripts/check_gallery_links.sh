#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Validate source-file links in the generated gallery HTML.
#
# For each <a href="..."> pointing to a source file on GitHub, this
# script verifies that the corresponding local file exists.  This
# catches broken links before deployment.
#
# Usage:
#   scripts/check_gallery_links.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GALLERY="$REPO_ROOT/docs/gallery/index.html"

if [[ ! -f "$GALLERY" ]]; then
  echo "ERROR: Gallery HTML not found at $GALLERY"
  echo "Run scripts/generate_gallery_html.sh first."
  exit 1
fi

# Extract all href values that point to example source files.
# Links look like: href="https://github.com/.../blob/main/examples/foo.rs"
BROKEN=0
TOTAL=0

while IFS= read -r link; do
  # Strip the GitHub prefix to get a repo-relative path.
  REL_PATH=$(echo "$link" | sed 's|.*blob/main/||')
  TOTAL=$((TOTAL + 1))

  if [[ ! -f "$REPO_ROOT/$REL_PATH" ]]; then
    echo "BROKEN: $link"
    echo "        File not found: $REL_PATH"
    BROKEN=$((BROKEN + 1))
  fi
done < <(grep -oP 'href="[^"]*examples/[^"]*\.rs"' "$GALLERY" | sed 's/href="//; s/"$//' | sort -u)

echo ""
echo "Checked $TOTAL source links, $BROKEN broken."

if [[ $BROKEN -gt 0 ]]; then
  exit 1
fi

exit 0
