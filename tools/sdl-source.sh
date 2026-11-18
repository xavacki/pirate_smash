#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
shopt -s nullglob
candidates=(
  .tools/cargo/registry/src/*/sdl3-src-3.4.14/SDL
  "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/sdl3-src-3.4.14/SDL
)
if [[ ${#candidates[@]} -eq 0 ]]; then
  echo 'SDL3 source missing. Run tools/setup-android.sh first.' >&2
  exit 1
fi
sdl="$(realpath "${candidates[0]}")"
if [[ $# -eq 0 ]]; then
  printf '%s\n' "$sdl"
else
  mkdir -p "$1"
  cp -R "$sdl/android-project/app/src/main/java/." "$1/"
fi
