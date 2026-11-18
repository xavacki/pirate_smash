#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .tools
if [[ ! -f .tools/bullet3/src/btBulletDynamicsCommon.h ]]; then
  archive=.tools/bullet-3.25.tar.gz
  if [[ ! -f "$archive" ]]; then
    curl -L --fail https://github.com/bulletphysics/bullet3/archive/refs/tags/3.25.tar.gz -o "$archive"
  fi
  mkdir -p .tools/bullet3
  tar xzf "$archive" --strip-components=1 -C .tools/bullet3
fi
if [[ "${1:-}" == --source-only ]]; then
  exit 0
fi
cmake -S .tools/bullet3 -B .tools/bullet-build \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DBUILD_BULLET2_DEMOS=OFF -DBUILD_BULLET3=OFF \
  -DBUILD_CPU_DEMOS=OFF -DBUILD_OPENGL3_DEMOS=OFF \
  -DBUILD_UNIT_TESTS=OFF -DBUILD_EXTRAS=OFF -DBUILD_PYBULLET=OFF \
  -DINSTALL_LIBS=OFF
cmake --build .tools/bullet-build --target BulletDynamics BulletCollision LinearMath --parallel 2
