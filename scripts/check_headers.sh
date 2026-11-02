#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Licence header guard (GUP-588). Every code file carries
# `SPDX-License-Identifier: GPL-3.0-or-later` in its first 5 lines.
#
#   scripts/check_headers.sh [FILE...]
#
# Without arguments it checks the git-tracked files matching
# *.rs *.wgsl *.sh *.mjs *.js *.css *.html. With arguments it checks exactly
# those files (the seeded run in the story uses this). Exempt: the generated
# test fixtures crates/gup-core/tests/fixtures/*.wgsl, which the tests compare
# byte for byte, and the parked, frozen crates/gup-culling-lod/.
# Prints one line per file lacking the header and exits 1; otherwise prints
# "headers: OK (<n> files)" and exits 0.
set -euo pipefail

files=()
if [[ $# -gt 0 ]]; then
  files=("$@")
else
  cd "$(git rev-parse --show-toplevel)"
  while IFS= read -r -d '' f; do
    case "$f" in
      crates/gup-core/tests/fixtures/*.wgsl | crates/gup-culling-lod/*) continue ;;
    esac
    [[ -f "$f" ]] && files+=("$f")
  done < <(git ls-files -z -- '*.rs' '*.wgsl' '*.sh' '*.mjs' '*.js' '*.css' '*.html')
fi

bad=0
for f in "${files[@]}"; do
  if ! head -n 5 -- "$f" | grep -q 'SPDX-License-Identifier: GPL-3.0-or-later'; then
    echo "missing licence header: $f"
    bad=1
  fi
done

if [[ $bad -ne 0 ]]; then
  exit 1
fi
echo "headers: OK (${#files[@]} files)"
