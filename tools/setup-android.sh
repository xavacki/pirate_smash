#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
task_root="$PWD"
case "${SMASH_ANDROID_ABI:-arm64-v8a}" in
    arm64-v8a) task_target=aarch64-linux-android; task_suffix= ;;
    x86_64) task_target=x86_64-linux-android; task_suffix=-x86_64 ;;
    *) echo "SMASH_ANDROID_ABI must be arm64-v8a or x86_64" >&2; exit 1 ;;
esac
source tools/android-rust.sh
task_cache="$task_root/.tools/android"
mkdir -p "$task_cache"
fetch_android() {
  local filename="$1" url="$2" algorithm="$3" digest="$4"
  if [[ ! -f "$task_cache/$filename" ]]; then
    curl -L --fail --retry 2 --connect-timeout 20 "$url" -o "$task_cache/$filename"
  fi
  printf '%s  %s\n' "$digest" "$task_cache/$filename" | "$algorithm" -c -
}
if [[ ! -f "$task_cache/android-ndk-r27d/build/cmake/android.toolchain.cmake" ]]; then
  fetch_android android-ndk-r27d-linux.zip https://dl.google.com/android/repository/android-ndk-r27d-linux.zip sha1sum 22105e410cf29afcf163760cc95522b9fb981121
  unzip -q "$task_cache/android-ndk-r27d-linux.zip" -d "$task_cache"
fi
if command -v rustup >/dev/null 2>&1; then
  rustup target add "$task_target"
else
  # Use the compiler's own manifest so the Android std exactly matches the host compiler.
  python3 - "$(command -v rustc)" "$task_cache" "$task_target" "$task_suffix" <<'PY'
import json,subprocess,sys,tomllib
from pathlib import Path
sysroot=Path(subprocess.check_output([sys.argv[1],'--print','sysroot'],text=True).strip())
manifest=tomllib.loads((sysroot/'lib/rustlib/multirust-channel-manifest.toml').read_text())
entry=manifest['pkg']['rust-std']['target'][sys.argv[3]]
cache=Path(sys.argv[2]);(cache/f'rust-component{sys.argv[4]}.json').write_text(json.dumps(dict(url=entry['xz_url'],sha256=entry['xz_hash'])))
PY
  task_url="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["url"])' "$task_cache/rust-component$task_suffix.json")"
  task_hash="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["sha256"])' "$task_cache/rust-component$task_suffix.json")"
  if [[ ! -d "$task_cache/rust-sysroot$task_suffix/lib/rustlib/$task_target/lib" || ! -f "$task_cache/rust-component-installed$task_suffix" || "$(cat "$task_cache/rust-component-installed$task_suffix")" != "$task_hash" ]]; then
    task_archive="$(basename "$task_url")"
    fetch_android "$task_archive" "$task_url" sha256sum "$task_hash"
    tar -xf "$task_cache/$task_archive" -C "$task_cache"
    "$task_cache/${task_archive%.tar.xz}/install.sh" --prefix="$task_cache/rust-sysroot$task_suffix" --disable-ldconfig
    printf '%s\n' "$task_hash" > "$task_cache/rust-component-installed$task_suffix"
  fi
fi
if ! cargo fetch --locked --offline; then
  cargo fetch --locked
fi
printf 'Native Android tools and Rust dependencies are ready.\n' 
