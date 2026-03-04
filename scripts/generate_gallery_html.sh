#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Generate the gallery index.html from the gallery config and thumbnails.
#
# Usage:
#   scripts/generate_gallery_html.sh
#
# Reads:  scripts/gallery_config.toml, docs/gallery/thumbs/*.png
# Writes: docs/gallery/index.html

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG="$SCRIPT_DIR/gallery_config.toml"
OUTPUT="$REPO_ROOT/docs/gallery/index.html"
GITHUB_BASE=$(grep '^github_base' "$CONFIG" | head -1 | sed 's/.*= *"//; s/"//')
GITHUB_BASE="${GITHUB_BASE:-https://github.com/au-phiware/gup/blob/main}"

# Parse all example entries from the config.
# Output: name|category|description|source|skip
parse_config() {
  awk '
    /^\[\[examples\]\]/ {
      if (name != "") print name "|" category "|" description "|" source "|" skip;
      name=""; category=""; description=""; source=""; skip="false"
    }
    /^name *= */ { gsub(/.*= *"|"/, ""); name=$0 }
    /^category *= */ { gsub(/.*= *"|"/, ""); category=$0 }
    /^description *= */ { gsub(/.*= *"|"/, ""); description=$0 }
    /^source *= */ { gsub(/.*= *"|"/, ""); source=$0 }
    /^skip *= *true/ { skip="true" }
    END { if (name != "") print name "|" category "|" description "|" source "|" skip }
  ' "$CONFIG"
}

# Collect ordered list of unique categories (preserving config order).
declare -a CATEGORIES=()
declare -A SEEN_CATS=()
while IFS='|' read -r _ cat _ _ _; do
  if [[ -z "${SEEN_CATS[$cat]:-}" ]]; then
    CATEGORIES+=("$cat")
    SEEN_CATS["$cat"]=1
  fi
done < <(parse_config)

# Count total and renderable examples.
TOTAL=0
RENDERABLE=0
while IFS='|' read -r _ _ _ _ eskip; do
  TOTAL=$((TOTAL + 1))
  [[ "$eskip" != "true" ]] && RENDERABLE=$((RENDERABLE + 1))
done < <(parse_config)

# Start generating HTML.
cat > "$OUTPUT" <<'HEADER'
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Gup Example Gallery</title>
  <link rel="stylesheet" href="gallery.css">
</head>
<body>
  <header>
    <h1>Gup Example Gallery</h1>
    <p class="subtitle">
      GPU-accelerated data visualisation examples.
      Each thumbnail links to the example's source code.
    </p>
    <nav aria-label="Category navigation">
      <ul class="category-nav">
HEADER

# Category navigation links.
for cat in "${CATEGORIES[@]}"; do
  slug=$(echo "$cat" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-')
  echo "        <li><a href=\"#${slug}\">${cat}</a></li>" >> "$OUTPUT"
done

cat >> "$OUTPUT" <<'NAV_END'
      </ul>
    </nav>
  </header>
  <main>
NAV_END

# Generate sections for each category.
for cat in "${CATEGORIES[@]}"; do
  slug=$(echo "$cat" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-')

  # Count examples in this category.
  CAT_COUNT=0
  while IFS='|' read -r _ ecat _ _ eskip; do
    [[ "$ecat" == "$cat" ]] && CAT_COUNT=$((CAT_COUNT + 1))
  done < <(parse_config)

  cat >> "$OUTPUT" <<SECTION_HEAD
    <section id="${slug}">
      <h2>${cat}</h2>
      <div class="gallery-grid">
SECTION_HEAD

  while IFS='|' read -r ename ecat edesc esource eskip; do
    [[ "$ecat" != "$cat" ]] && continue

    THUMB_FILE="thumbs/${ename}.png"
    THUMB_PATH="$REPO_ROOT/docs/gallery/thumbs/${ename}.png"
    SOURCE_URL="${GITHUB_BASE}/${esource}"

    if [[ "$eskip" == "true" ]]; then
      # Skipped example: show a placeholder card.
      cat >> "$OUTPUT" <<CARD
        <article class="gallery-card skipped">
          <div class="thumb-placeholder" aria-label="No thumbnail available">
            <span class="placeholder-label">${ename}</span>
            <span class="placeholder-note">Requires window</span>
          </div>
          <div class="card-info">
            <h3><a href="${SOURCE_URL}">${ename}</a></h3>
            <p>${edesc}</p>
          </div>
        </article>
CARD
    elif [[ -f "$THUMB_PATH" ]]; then
      # Has a real thumbnail.
      cat >> "$OUTPUT" <<CARD
        <article class="gallery-card">
          <a href="${SOURCE_URL}" class="thumb-link">
            <img src="${THUMB_FILE}" alt="Screenshot of ${ename} example"
                 loading="lazy" width="640" height="480">
          </a>
          <div class="card-info">
            <h3><a href="${SOURCE_URL}">${ename}</a></h3>
            <p>${edesc}</p>
          </div>
        </article>
CARD
    else
      # No thumbnail yet — show a pending placeholder.
      cat >> "$OUTPUT" <<CARD
        <article class="gallery-card pending">
          <div class="thumb-placeholder" aria-label="Thumbnail not yet generated">
            <span class="placeholder-label">${ename}</span>
            <span class="placeholder-note">Pending</span>
          </div>
          <div class="card-info">
            <h3><a href="${SOURCE_URL}">${ename}</a></h3>
            <p>${edesc}</p>
          </div>
        </article>
CARD
    fi
  done < <(parse_config)

  cat >> "$OUTPUT" <<SECTION_END
      </div>
    </section>
SECTION_END
done

# Footer with generation metadata.
GENERATED_DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)
cat >> "$OUTPUT" <<FOOTER
  </main>
  <footer>
    <p>
      Generated on ${GENERATED_DATE} &middot;
      ${RENDERABLE} renderable / ${TOTAL} total examples &middot;
      <a href="https://github.com/au-phiware/gup">Gup on GitHub</a>
    </p>
  </footer>
</body>
</html>
FOOTER

echo "Generated $OUTPUT ($TOTAL examples, $RENDERABLE renderable)"
