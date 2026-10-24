#!/usr/bin/env bash
# Copyright (C) 2026 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Story ID guard (GUP-504). Two stories given the same ID merge without a
# textual conflict, so git cannot catch it. This checks the story files and the
# index entries for IDs above the baseline GUP-315. The legacy duplicates at or
# below the baseline are archived history and are not renumbered.
#
#   scripts/check_story_ids.sh [--dir DIR]
#
# Reads DIR/GUP-*.md (default docs/planning/stories), the entry lines
# (^- [GUP-...](...)) of DIR/INDEX.md and, if present, DIR/INDEX_ARCHIVE.md.
# Prints one line per problem and exits 1 if there is any. Otherwise prints
# "story ids: OK (<n> stories)" and exits 0. Rules:
#   1. every entry's link names a file that exists in DIR;
#   2. each ID above the baseline has at most one story file;
#   3. each ID above the baseline has exactly one entry across both index files,
#      and that entry links a file with the same number;
#   4. each story file above the baseline has a first level-1 heading that
#      starts with "# GUP-NNN:" (the file's number);
#   5. no story file above the baseline has a letter suffix (GUP-285B style).
# Entry text other than the leading link (for example a link in the middle of a
# sentence) is not an entry and is not counted.

set -uo pipefail
shopt -s nullglob

baseline=315
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
dir=$root/docs/planning/stories

usage() {
  echo "usage: scripts/check_story_ids.sh [--dir DIR]"
}

while (($#)); do
  case $1 in
    --dir)
      if [[ $# -lt 2 ]]; then
        usage >&2
        exit 2
      fi
      dir=$2
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

# number_of TEXT: sets $n to the decimal number at the start of TEXT (400 for
# 400B_X.md). A variable, not a command substitution: a fork per file is slow.
number_of() {
  local digits=${1%%[!0-9]*}
  n=$((10#${digits:-0}))
}

problems=()
add() { problems+=("story ids: $*"); }

report() {
  if ((${#problems[@]})); then
    printf '%s\n' "${problems[@]}"
    exit 1
  fi
  echo "story ids: OK ($stories stories)"
  exit 0
}

# ── story files ─────────────────────────────────────────────────────
declare -A file_count file_names
stories=0
for f in "$dir"/GUP-*.md; do
  base=${f##*/}
  rest=${base#GUP-}
  digits=${rest%%[!0-9]*}
  number_of "$rest"
  num=$n
  stories=$((stories + 1))
  ((num > baseline)) || continue

  file_count[$num]=$((${file_count[$num]:-0} + 1))
  file_names[$num]="${file_names[$num]:-} $base"

  suffix=${rest:${#digits}:1}
  if [[ $suffix == [A-Za-z] ]]; then
    add "rule 5: $base: letter suffix above the baseline GUP-$baseline"
  fi

  heading=$(grep -m1 '^# ' -- "$f")
  if [[ $heading != "# GUP-$digits:"* ]]; then
    add "rule 4: $base: first heading is '${heading:-<none>}', not '# GUP-$digits:'"
  fi
done

# ── index entries ───────────────────────────────────────────────────
re='^- \[GUP-([^]]*)\]\(([^)]*)\)'
declare -A entry_count entry_ok
index_files=(INDEX.md INDEX_ARCHIVE.md)
if [[ ! -f $dir/INDEX.md ]]; then
  add "cannot read $dir/INDEX.md"
  report
fi
for idx in "${index_files[@]}"; do
  path=$dir/$idx
  [[ -f $path ]] || continue
  while IFS= read -r line || [[ -n $line ]]; do
    [[ $line =~ $re ]] || continue
    token=${BASH_REMATCH[1]}
    link=${BASH_REMATCH[2]}
    number_of "$token"
    num=$n
    if [[ ! -f $dir/$link ]]; then
      add "rule 1: $idx: GUP-$token links missing file $link"
    fi
    ((num > baseline)) || continue
    entry_count[$num]=$((${entry_count[$num]:-0} + 1))
    linkbase=${link##*/}
    number_of "${linkbase#GUP-}"
    if [[ $linkbase == GUP-* && $n == "$num" ]]; then
      entry_ok[$num]=1
    fi
  done <"$path"
done

# ── rules 2 and 3, per ID above the baseline ────────────────────────
declare -A seen
for num in "${!file_count[@]}" "${!entry_count[@]}"; do
  [[ -z ${seen[$num]:-} ]] || continue
  seen[$num]=1
  fc=${file_count[$num]:-0}
  ec=${entry_count[$num]:-0}
  if ((fc > 1)); then
    add "rule 2: GUP-$num has $fc story files:${file_names[$num]}"
  fi
  if ((fc >= 1 && ec == 0)); then
    add "rule 3: GUP-$num has a story file${file_names[$num]} but no index entry"
  elif ((ec > 1)); then
    add "rule 3: GUP-$num has $ec index entries (expected 1 across INDEX.md and INDEX_ARCHIVE.md)"
  elif ((ec == 1 && fc >= 1 && ${entry_ok[$num]:-0} == 0)); then
    add "rule 3: GUP-$num's index entry does not link its story file"
  fi
done

report
