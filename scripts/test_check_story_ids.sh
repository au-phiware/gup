#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Tests for scripts/check_story_ids.sh (GUP-504) in a throwaway directory. Each
# case starts from a valid set and breaks one rule.
set -uo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
check=$root/scripts/check_story_ids.sh

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fails=0
fail() {
  echo "FAIL: $*" >&2
  fails=$((fails + 1))
}

# A valid set: two stories above the baseline with one entry each, and a legacy
# pair GUP-013 with two entries (one in the archive).
base=$tmp/base
mkdir -p "$base"
printf '# GUP-013: Legacy A\n' >"$base/GUP-013_A.md"
printf '# GUP-013: Legacy B\n' >"$base/GUP-013_B.md"
printf '# GUP-400: Four hundred\n' >"$base/GUP-400_Four.md"
printf '# GUP-401: Four oh one\n' >"$base/GUP-401_Four_One.md"
cat >"$base/INDEX.md" <<'EOF'
# Stories

Some prose that mentions [GUP-013](GUP-013_A.md) is not an entry.

- [GUP-400](GUP-400_Four.md) ✅ — four hundred
- [GUP-401](GUP-401_Four_One.md) 📋 — four oh one
EOF
cat >"$base/INDEX_ARCHIVE.md" <<'EOF'
# Archive

- [GUP-013](GUP-013_A.md) ✅ — legacy A
- [GUP-013](GUP-013_B.md) ✅ — legacy B
EOF

# expect_pass NAME: the case in $tmp/NAME passes.
expect_pass() {
  local out
  if ! out=$("$check" --dir "$tmp/$1" 2>&1); then
    fail "$1 should pass: $out"
  fi
}

# expect_fail NAME RULE: the case fails with a message naming RULE.
expect_fail() {
  local out status
  out=$("$check" --dir "$tmp/$1" 2>&1)
  status=$?
  if ((status != 1)); then
    fail "$1 should exit 1, got $status"
  elif [[ $out != *"$2"* ]]; then
    fail "$1 should report '$2': $out"
  fi
}

# case_of NAME: a copy of the valid set to break.
case_of() {
  cp -r "$base" "$tmp/$1"
}

# A fresh copy of the valid set must pass with its count.
expect_pass base
if ! out=$("$check" --dir "$base" 2>&1) || [[ $out != "story ids: OK (4 stories)" ]]; then
  fail "valid set should print OK (4 stories): $out"
fi

# Rule 2: a second story file with an ID above the baseline.
case_of dup_file
printf '# GUP-400: Copy\n' >"$tmp/dup_file/GUP-400_Copy.md"
expect_fail dup_file "rule 2"

# Rule 3: a second entry for the same ID, in the archive.
case_of dup_entry
printf -- '- [GUP-400](GUP-400_Four.md) ✅ — again\n' >>"$tmp/dup_entry/INDEX_ARCHIVE.md"
expect_fail dup_entry "rule 3"

# Rule 3: a story file with no entry.
case_of no_entry
printf '# GUP-402: Orphan\n' >"$tmp/no_entry/GUP-402_Orphan.md"
expect_fail no_entry "rule 3"

# Rule 1: an entry linking a missing file.
case_of missing_link
printf -- '- [GUP-403](GUP-403_Gone.md) 📋 — gone\n' >>"$tmp/missing_link/INDEX.md"
expect_fail missing_link "rule 1"

# Rule 4: a heading whose number differs from the file name.
case_of heading
printf '# GUP-405: Wrong\n' >"$tmp/heading/GUP-404_Heading.md"
printf -- '- [GUP-404](GUP-404_Heading.md) 📋 — heading\n' >>"$tmp/heading/INDEX.md"
expect_fail heading "rule 4"

# Rule 5: a letter suffix above the baseline.
case_of suffix
mv "$tmp/suffix/GUP-400_Four.md" "$tmp/suffix/GUP-400B_X.md"
sed -i 's/GUP-400_Four\.md/GUP-400B_X.md/' "$tmp/suffix/INDEX.md"
expect_fail suffix "rule 5"

if ((fails)); then
  echo "test_check_story_ids: $fails failure(s)" >&2
  exit 1
fi
echo "test_check_story_ids: all cases pass"
