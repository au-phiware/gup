#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Proportional pre-commit checks (GUP-398). `mask pre-commit` runs this and
# the git hook installed by flake.nix runs `mask pre-commit`.
#
# Usage:
#   scripts/pre_commit.sh                 # classify the staged files and run
#   scripts/pre_commit.sh --plan [PATH…]  # print the plan only
#   scripts/pre_commit.sh PATH…           # classify PATHs instead of the index
#
# Every staged path is classified, and the most demanding class wins:
#
#   full   Anything not recognised below: the workspace manifest, Cargo.lock,
#          rust-toolchain.toml, flake.nix/flake.lock, maskfile.md, this
#          script, root files outside the gup crate's directories, the parked
#          crates outside the workspace. Runs `mask all-check`, unscoped.
#   crate  A file inside a workspace member's directory (for the root `gup`
#          crate: src/, tests/, examples/, benches/, assets/, build.rs), or a
#          file some member reads at compile time (include_str!, include_bytes!,
#          include!, #[path]), e.g. docs/tutorials/*.md or assets/fonts/*.
#          rustfmt runs on the touched crates. Clippy (default features and
#          --all-features, every target, -D warnings) runs on the touched
#          crates plus every workspace member that depends on them, since a
#          change can break a dependent. validate-marks runs when `gup` is in
#          that set.
#   docs   *.md, docs/**, COPYING, LICENSE*: no Rust check.
#   dogfood
#          dogfood/** (a detached crate): its rustfmt check.
#
# Outside full mode, these always run: the trailing-whitespace check on every
# .rs file and the gallery sync check (both repo-wide and cheap), and
# prettier and mdl on the changed Markdown files. nixfmt and statix need not:
# a flake.nix change forces full mode.
#
# This is a local-hook optimisation only. CI never scopes: every workflow runs
# its full checks on every push. Like the rest of the hook, the checks read
# the working tree, not the index.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

plan_only=0
if [[ ${1:-} == --plan ]]; then
  plan_only=1
  shift
fi

if [[ $# -gt 0 ]]; then
  paths=("$@")
else
  mapfile -t paths < <(git diff --cached --name-only --no-renames)
fi

# Workspace members: name and directory relative to the repository root.
declare -A member_dir=() dir_member=()
root_member=""
metadata=$(cargo metadata --no-deps --format-version 1 2>/dev/null || true)
full_reasons=()
if [[ -z $metadata ]]; then
  full_reasons+=("cargo metadata failed")
else
  top=$(pwd -P)
  while IFS=$'\t' read -r name manifest; do
    dir=$(dirname "$manifest")
    rel=${dir#"$top"}
    rel=${rel#/}
    [[ -z $rel ]] && rel=. && root_member=$name
    member_dir[$name]=$rel
    dir_member[$rel]=$name
  done < <(jq -r '.workspace_members as $m | .packages[]
      | select(.id as $id | $m | index($id)) | [.name, .manifest_path] | @tsv' \
    <<<"$metadata")
fi

# Reverse dependencies between members (path dependencies only).
declare -A rdeps=()
if [[ -n $metadata ]]; then
  while IFS=$'\t' read -r name dep; do
    [[ -n ${member_dir[$dep]:-} ]] && rdeps[$dep]+=" $name"
  done < <(jq -r '.workspace_members as $m | .packages[]
      | select(.id as $id | $m | index($id)) | .name as $n
      | .dependencies[] | select(.path != null) | [$n, .name] | @tsv' \
    <<<"$metadata")
fi

# The member owning a directory: the nearest ancestor that is a member
# directory. Prints nothing for the root crate's directory or above.
owning_member_dir() {
  local d=$1
  while [[ $d != . && $d != / ]]; do
    if [[ -n ${dir_member[$d]:-} ]]; then
      echo "$d"
      return
    fi
    d=$(dirname "$d")
  done
}

# The nearest ancestor below the root with a Cargo.toml that is not a member.
foreign_manifest_dir() {
  local d=$1
  while [[ $d != . && $d != / ]]; do
    if [[ -f $d/Cargo.toml && -z ${dir_member[$d]:-} ]]; then
      echo "$d"
      return
    fi
    d=$(dirname "$d")
  done
}

# Files read at compile time, mapped to the member that reads them.
declare -A included_by=()
while IFS=: read -r file _ rest; do
  while [[ $rest =~ (include(_str|_bytes)?!\(\"([^\"]+)\"|#\[path\ =\ \"([^\"]+)\") ]]; do
    rel=${BASH_REMATCH[3]:-${BASH_REMATCH[4]}}
    rest=${rest#*"${BASH_REMATCH[1]}"}
    target=$(realpath -m --relative-to=. "$(dirname "$file")/$rel")
    owner_dir=$(owning_member_dir "$(dirname "$file")")
    owner=${dir_member[${owner_dir:-.}]:-}
    if [[ -z $owner ]]; then
      # Not inside a member directory (dogfood, a parked crate): no member
      # reads it, so the plain path rules below apply.
      continue
    fi
    included_by[$target]+=" $owner"
  done
done < <(git grep -nE 'include(_str|_bytes)?!\("|#\[path = "' -- '*.rs' || true)

is_doc() {
  case $1 in
  *.md | docs/* | COPYING | LICENSE*) return 0 ;;
  *) return 1 ;;
  esac
}

declare -A touched=()
docs=0
dogfood=0
for p in "${paths[@]}"; do
  case $p in
  Cargo.toml | Cargo.lock | rust-toolchain.toml | flake.nix | flake.lock | \
    maskfile.md | scripts/pre_commit.sh | scripts/test_pre_commit.sh | \
    .cargo/* | clippy.toml | .clippy.toml | rustfmt.toml | .rustfmt.toml)
    full_reasons+=("$p is workspace-level")
    continue
    ;;
  esac
  if [[ -n ${included_by[$p]:-} ]]; then
    for m in ${included_by[$p]}; do touched[$m]=1; done
    continue
  fi
  if is_doc "$p"; then
    docs=1
    continue
  fi
  case $p in
  dogfood/*)
    dogfood=1
    continue
    ;;
  esac
  d=$(owning_member_dir "$(dirname "$p")")
  if [[ -n $d ]]; then
    touched[${dir_member[$d]}]=1
    continue
  fi
  if [[ -n $root_member ]]; then
    case $p in
    src/* | tests/* | examples/* | benches/* | assets/* | build.rs)
      # A crate outside the workspace can live inside these directories
      # (the parked Tauri app is under examples/).
      if [[ -n $(foreign_manifest_dir "$(dirname "$p")") ]]; then
        full_reasons+=("$p is in a crate outside the workspace")
      else
        touched[$root_member]=1
      fi
      continue
      ;;
    esac
  fi
  full_reasons+=("$p is outside every workspace crate")
done

# Close the touched set over reverse dependencies.
declare -A lint=()
queue=("${!touched[@]}")
while [[ ${#queue[@]} -gt 0 ]]; do
  m=${queue[0]}
  queue=("${queue[@]:1}")
  [[ -n ${lint[$m]:-} ]] && continue
  lint[$m]=1
  for r in ${rdeps[$m]:-}; do queue+=("$r"); done
done

if [[ ${#full_reasons[@]} -gt 0 ]]; then
  mode=full
elif [[ ${#lint[@]} -gt 0 || $dogfood -eq 1 ]]; then
  mode=scoped
elif [[ $docs -eq 1 ]]; then
  mode=docs
else
  mode=none
fi

sorted() { printf '%s\n' "$@" | sort | tr '\n' ' ' | sed 's/ $//'; }
fmt_crates=$(sorted "${!touched[@]}")
lint_crates=$(sorted "${!lint[@]}")
echo "pre-commit: mode=$mode"
if [[ $mode == full ]]; then
  printf 'pre-commit: full because %s\n' "${full_reasons[@]}"
fi
if [[ $mode == scoped ]]; then
  echo "pre-commit: fmt=[$fmt_crates] clippy=[$lint_crates] dogfood=$dogfood"
fi
[[ $plan_only -eq 1 ]] && exit 0

if [[ $mode == full ]]; then
  exec mask all-check
fi

checks=(
  'git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" -- "*.rs"; test $? -eq 1'
  './scripts/check_gallery_sync.sh'
)
names='\s,gallery'

# Markdown checks on the changed Markdown files (a markdown config change is
# outside every crate, so it already forced the full, repo-wide check).
md=()
for p in "${paths[@]}"; do
  [[ $p == *.md && -f $p ]] && md+=("$(printf '%q' "$p")")
done
if [[ ${#md[@]} -gt 0 ]]; then
  checks+=("prettier --log-level warn --check ${md[*]} && mdl ${md[*]}")
  names+=',md'
fi

if [[ $mode == scoped ]]; then
  rust=()
  pkgs=()
  for m in ${lint_crates}; do pkgs+=(-p "$m"); done
  fmt_pkgs=()
  for m in ${fmt_crates}; do fmt_pkgs+=(-p "$m"); done
  if [[ ${#fmt_pkgs[@]} -gt 0 ]]; then
    rust+=("cargo fmt ${fmt_pkgs[*]} -- --check")
  fi
  if [[ $dogfood -eq 1 ]]; then
    rust+=("cargo fmt --manifest-path dogfood/Cargo.toml -- --check")
  fi
  if [[ ${#pkgs[@]} -gt 0 ]]; then
    rust+=("cargo clippy ${pkgs[*]} --all-targets -- -D warnings")
    rust+=("cargo clippy ${pkgs[*]} --all-targets --all-features -- -D warnings")
  fi
  if [[ ${#rust[@]} -gt 0 ]]; then
    joined=$(printf ' && %s' "${rust[@]}")
    checks+=("${joined# && }")
    names+=',rs'
  fi
  if [[ -n ${lint[$root_member]:-} ]]; then
    checks+=('mask validate-marks')
    names+=',marks'
  fi
fi

exec concurrently --group --names "$names" "${checks[@]}"
