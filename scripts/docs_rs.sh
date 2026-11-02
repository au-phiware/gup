#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Emulate the docs.rs build of every published crate (GUP-589).
#
# docs.rs reads `[package.metadata.docs.rs]`: it builds the listed targets,
# with all features when `all-features = true`, passing `rustdoc-args` to
# rustdoc and setting `DOCS_RS=1`. This script does the same on the pinned
# stable toolchain (docs.rs uses a nightly rustdoc; no code here uses
# nightly-only doc attributes) and adds `-D warnings`, so a broken intra-doc
# link fails here before it fails there. It exits non-zero on the first
# failure and names the crate and target.
#
# Needs the network for the registry (CI's cache may be cold); docs.rs itself
# builds offline, and the build scripts read only their own package's files.
#
# Usage: scripts/docs_rs.sh [CRATE...]   (default: every published crate)
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

crates=("$@")
if [[ ${#crates[@]} -eq 0 ]]; then
  crates=(gup gup-core gup-core-macros gup-text gup-wgsl gup-egui gup-bevy)
fi

metadata=$(cargo metadata --format-version 1 --no-deps)

for crate in "${crates[@]}"; do
  table=$(jq -c --arg n "$crate" \
    '.packages[] | select(.name == $n) | .metadata.docs.rs // empty' \
    <<<"$metadata")
  if [[ -z $table ]]; then
    echo "docs_rs: $crate has no [package.metadata.docs.rs] table" >&2
    exit 1
  fi
  all_features=$(jq -r '."all-features" // false' <<<"$table")
  mapfile -t targets < <(jq -r '.targets // [] | .[]' <<<"$table")
  mapfile -t rustdoc_args < <(jq -r '."rustdoc-args" // [] | .[]' <<<"$table")
  if [[ ${#targets[@]} -eq 0 ]]; then
    echo "docs_rs: $crate lists no docs.rs targets" >&2
    exit 1
  fi
  flags=()
  if [[ $all_features == true ]]; then
    flags+=(--all-features)
  fi
  for target in "${targets[@]}"; do
    echo "docs_rs: $crate ($target ${flags[*]:-})"
    if ! DOCS_RS=1 RUSTDOCFLAGS="${rustdoc_args[*]} -D warnings" \
      cargo doc --no-deps -p "$crate" --target "$target" ${flags[@]+"${flags[@]}"}; then
      echo "docs_rs: FAILED: $crate for $target" >&2
      exit 1
    fi
  done
done
echo "docs_rs: all crates documented"
