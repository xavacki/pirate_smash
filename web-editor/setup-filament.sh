#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
temp=$(mktemp -d)
trap 'rm -rf -- "$temp"' EXIT
curl -fLsS --retry 2 https://github.com/google/filament/releases/download/v1.70.0/filament-v1.70.0-web.tgz -o "$temp/filament.tgz"
echo "b4939b86f3f75cef643d8cbbbb95aefcba75ff6ab176952c43cb7e061ba4b2d2  $temp/filament.tgz" | sha256sum -c
tar xf "$temp/filament.tgz" -C "$temp" filament.js filament.wasm
install -Dm644 "$temp/filament.js" vendor/filament.js
install -Dm644 "$temp/filament.wasm" vendor/filament.wasm
