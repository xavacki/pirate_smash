#!/usr/bin/env bash
set -euo pipefail
output="$1"
shift
cd "$(dirname "$0")/.."
task_root="$PWD"
source tools/android-rust.sh
task_sdl="$(bash tools/sdl-source.sh)"

for task_abi in "$@"; do
  case "$task_abi" in
    arm64-v8a) task_target=aarch64-linux-android; task_env=AARCH64_LINUX_ANDROID; task_suffix= ;;
    x86_64) task_target=x86_64-linux-android; task_env=X86_64_LINUX_ANDROID; task_suffix=-x86_64 ;;
    *) echo "Unsupported Android ABI: $task_abi" >&2; exit 1 ;;
  esac
  task_filament="${FILAMENT_SDK:-$HOME/soft/android/filament/$task_abi}"
  if [[ ! -f "$task_filament/lib/libfilament.a" && ! -f "$task_filament/lib/$task_abi/libfilament.a" ]]; then
    if [[ -d "$task_root/.tools/filament-sdk" ]]; then
      task_filament="$task_root/.tools/filament-sdk"
    else
      echo "Filament SDK missing. Run tools/setup-filament.sh first." >&2
      exit 1
    fi
  fi
  if [[ -d "$HOME/soft/android-ndk-r27d" ]]; then
    task_ndk="$HOME/soft/android-ndk-r27d"
  else
    task_ndk="$task_root/.tools/android/android-ndk-r27d"
  fi
  if [[ ! -f "$task_ndk/build/cmake/android.toolchain.cmake" ]]; then
    echo "Android NDK missing at $task_ndk. Run SMASH_ANDROID_ABI=$task_abi tools/setup-android.sh." >&2
    exit 1
  fi
  if ! [[ -d "$(rustc --target "$task_target" --print target-libdir 2>/dev/null)" || -d ".tools/android/rust-sysroot$task_suffix/lib/rustlib/$task_target/lib" ]]; then
    echo "Android Rust standard library missing. Run rustup target add $task_target or SMASH_ANDROID_ABI=$task_abi tools/setup-android.sh." >&2
    exit 1
  fi
  task_bullet="$HOME/soft/android/bullet3/$task_abi"
  if [[ ! -f "$task_bullet/lib/libBulletDynamics.a" ]]; then
    cmake -S .tools/bullet3 -B "build/native/bullet-build$task_suffix" \
      -DCMAKE_TOOLCHAIN_FILE="$task_ndk/build/cmake/android.toolchain.cmake" \
      -DANDROID_ABI="$task_abi" -DANDROID_PLATFORM=android-26 -DANDROID_STL=c++_static \
      -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DBUILD_BULLET2_DEMOS=OFF \
      -DBUILD_UNIT_TESTS=OFF -DBUILD_EXTRAS=OFF -DBUILD_CPU_DEMOS=OFF \
      -DBUILD_OPENGL3_DEMOS=OFF -DBUILD_BULLET3=OFF -DINSTALL_LIBS=OFF
    cmake --build "build/native/bullet-build$task_suffix" --target BulletDynamics BulletCollision LinearMath -j "${SMASH_BUILD_JOBS:-6}"
  fi
  cmake -S "$task_sdl" -B "build/native/sdl-build$task_suffix" \
    -DCMAKE_TOOLCHAIN_FILE="$task_ndk/build/cmake/android.toolchain.cmake" \
    -DANDROID_ABI="$task_abi" -DANDROID_PLATFORM=android-26 -DANDROID_STL=c++_static \
    -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON -DCMAKE_BUILD_TYPE=Release \
    -DSDL_SHARED=ON -DSDL_STATIC=OFF -DSDL_TESTS=OFF -DSDL_EXAMPLES=OFF
  cmake --build "build/native/sdl-build$task_suffix" -j "${SMASH_BUILD_JOBS:-6}"
  export SMASH_FILAMENT_NATIVE="$PWD/build/native/filament-build$task_suffix"
  cmake -S native/filament -B "$SMASH_FILAMENT_NATIVE" \
    -DCMAKE_TOOLCHAIN_FILE="$task_ndk/build/cmake/android.toolchain.cmake" \
    -DANDROID_ABI="$task_abi" -DANDROID_PLATFORM=android-26 -DANDROID_STL=c++_static \
    -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON -DCMAKE_BUILD_TYPE=Release -DFILAMENT_SDK="$task_filament"
  cmake --build "$SMASH_FILAMENT_NATIVE" -j "${SMASH_BUILD_JOBS:-6}"
  export "CARGO_TARGET_${task_env}_LINKER=$task_ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/${task_target}26-clang++"
  task_rustflags="-C link-arg=-Wl,-z,max-page-size=16384"
  if [[ -d "$task_root/.tools/android/rust-sysroot$task_suffix/lib/rustlib/$task_target/lib" ]]; then
    task_rustflags="--sysroot=$task_root/.tools/android/rust-sysroot$task_suffix $task_rustflags"
  fi
  export "CARGO_TARGET_${task_env}_RUSTFLAGS=$task_rustflags"
  cargo build --lib --release --locked --target "$task_target"
  abi_output="$output/$task_abi"
  mkdir -p "$abi_output"
  install -m 644 "target/$task_target/release/libsmash_land_port.so" "$abi_output/libmain.so"
  install -m 644 "build/native/sdl-build$task_suffix/libSDL3.so" "$abi_output/libSDL3.so"
  install -m 644 "$SMASH_FILAMENT_NATIVE/libsmash_filament.so" "$abi_output/libsmash_filament.so"
done
