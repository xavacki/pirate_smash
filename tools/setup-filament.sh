#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=1.70.0
mkdir -p .tools/filament-download
archive=.tools/filament-download/android.tgz
if [[ ! -f "$archive" ]]; then
  curl -fLsS --retry 2 "https://github.com/google/filament/releases/download/v$version/filament-v$version-android-native.tgz" -o "$archive.part"
  mv "$archive.part" "$archive"
fi
printf '%s  %s\n' ffd143b27e79b5f79dfd6b3467b241d7eb7e99a0ff95123227371c9fc9fca0aa "$archive" | sha256sum -c
mkdir -p .tools/filament-download/sdk
tar xf "$archive" -C .tools/filament-download/sdk --wildcards 'filament/include/*' 'filament/lib/arm64-v8a/*' 'filament/lib/x86_64/*'
sdk=.tools/filament-download/sdk/filament
mkdir -p "$sdk/include/private/backend"
curl -fsSL "https://raw.githubusercontent.com/google/filament/v$version/filament/backend/include/private/backend/VirtualMachineEnv.h" \
  -o "$sdk/include/private/backend/VirtualMachineEnv.h"
ln -sfn filament-download/sdk/filament .tools/filament-sdk
