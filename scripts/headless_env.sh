#!/usr/bin/env bash
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The headless environment for windowed checks (GUP-501):
#
#   eval "$(scripts/headless_env.sh)"          export the settings
#   scripts/headless_env.sh --run CMD [ARGS]   run CMD under xvfb-run
#   scripts/headless_env.sh --check            report what it resolved
#
# Settings: Mesa's lavapipe Vulkan driver (never the host GPU, for parity with
# CI), WGPU_BACKEND=vulkan, no WAYLAND_DISPLAY (else winit opens a real window
# that a tiling compositor resizes) and an Xvfb screen of 1920x1080x24.
# LD_LIBRARY_PATH is deliberately untouched (see scripts/wasm_browser.sh).
set -euo pipefail

screen=${GUP_XVFB_SCREEN:-1920x1080x24}

# Candidate lavapipe ICD paths, in resolution order.
candidates=()
[[ -z ${GUP_LAVAPIPE_ICD:-} ]] || candidates+=("$GUP_LAVAPIPE_ICD")
[[ -z ${LIBGL_DRIVERS_PATH:-} ]] ||
  candidates+=("$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json")
candidates+=(
  /run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json
  /usr/share/vulkan/icd.d/lvp_icd.x86_64.json
)

icd=
for c in "${candidates[@]}"; do
  if [[ -f $c ]]; then
    icd=$c
    break
  fi
done

case ${1:-} in
--run)
  shift
  [[ $# -gt 0 ]] || {
    echo "headless_env: --run needs a command" >&2
    exit 2
  }
  if [[ -z $icd ]]; then
    echo "headless_env: no lavapipe ICD" >&2
    exit 1
  fi
  command -v xvfb-run >/dev/null || {
    echo "headless_env: xvfb-run not found" >&2
    exit 1
  }
  export VK_ICD_FILENAMES=$icd WGPU_BACKEND=vulkan GUP_XVFB_SCREEN=$screen
  unset WAYLAND_DISPLAY
  export GUP_HEADLESS=1
  exec xvfb-run -a -s "-screen 0 $screen" "$@"
  ;;
--check)
  status=0
  if [[ -n $icd ]]; then
    echo "lavapipe ICD: $icd"
  else
    echo "lavapipe ICD: missing; tried:"
    printf '  %s\n' "${candidates[@]}"
    status=1
  fi
  if xv=$(command -v xvfb-run); then
    echo "xvfb-run: $xv"
  else
    echo "xvfb-run: missing"
    status=1
  fi
  echo "WGPU_BACKEND: vulkan"
  echo "screen: $screen"
  exit $status
  ;;
"")
  if [[ -z $icd ]]; then
    echo 'echo "headless_env: no lavapipe ICD" >&2; false'
    exit 0
  fi
  printf 'export VK_ICD_FILENAMES=%q WGPU_BACKEND=vulkan GUP_XVFB_SCREEN=%q\n' "$icd" "$screen"
  echo 'unset WAYLAND_DISPLAY'
  ;;
*)
  echo "usage: headless_env.sh [--run CMD...|--check]" >&2
  exit 2
  ;;
esac
