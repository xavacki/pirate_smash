# Smash Land Android

Android Gradle project for the portrait Filament port of Smash Land. The `app` module contains the Android activity, manifest, and resources. Gradle builds the Rust game, SDL3, Bullet, and Filament and packages the level data, models, and textures.

## Build on WSL Ubuntu

Use JDK 21, the Android SDK Platform 35 and Build Tools 35.0.0, CMake, a C++ toolchain, Python 3.11 or newer, and the Gradle wrapper. Open the project root in Android Studio with a WSL/Linux build environment, or build directly from Ubuntu.

```sh
./gradlew :app:assembleDebug
```

The APK is at `app/build/outputs/apk/debug/app-debug.apk`. It includes ARM64 and x86-64 by default. To build only for a phone, pass `-PsmashAbis=arm64-v8a`. To build an Android App Bundle, run `./gradlew :app:bundleRelease` and configure release signing before distribution.

## Set up a new checkout

Install the Linux Android SDK packages `platforms;android-35` and `build-tools;35.0.0`. Set `ANDROID_HOME` or put `sdk.dir=/path/to/android-sdk` in `local.properties`. Install a Rust toolchain via Rustup, plus curl, tar, and unzip. Then run:

```sh
tools/setup-android.sh
SMASH_ANDROID_ABI=x86_64 tools/setup-android.sh
tools/setup-filament.sh
./gradlew :app:assembleDebug
```

The setup scripts provision the pinned NDK, Rust Android standard libraries, and Filament SDK in `.tools`. The first build obtains Bullet 3.25 if needed. For an ARM64-only build, skip the x86-64 setup and pass `-PsmashAbis=arm64-v8a`.

This WSL copy prefers `~/.android/debug.keystore`, with `.tools/android/debug.keystore` as a fallback. Updating an installed APK requires the same signing key and preserves app data. If the keys differ, use the isolated test application ID described in the pirate design notes.

## Gameplay

Tap **Materials** while in a level to edit a target or platform. **Save** keeps the selected appearance in private app storage. **Play** returns to the game.

## Level data

`assets/levels.json` stores the stages in play order. Each stage defines its level number, zero-based stage index, stage count, shot limit, vertical offset, and arrays of platforms and targets. The `position` and `half_extents` fields are XYZ, and `rotation` is an XYZW quaternion. A target's numeric `type` selects its visual and collider. The zero-based `support` points into the stage's platforms.

Platform `motion` retains the recovered 12-value order. The groups are rotation enable and speed, horizontal enable/min/max/direction/speed, and vertical enable/min/max/direction/speed. Static platforms may omit the field. Keep the relative order of existing targets and platforms to preserve saved material overrides.

## Browser level and shape editor

From the project root, run:

```sh
bash web-editor/setup-filament.sh  # One-time download of Filament Web 1.70.0
python3 web-editor/serve.py
```

Open **http://localhost:8050/** in a WebGL 2 browser. The editor loads the bundled levels and shape library when it opens.

- **Level** edits a stage's shot limit, targets, platforms, poses, and platform motion settings. Use **Open JSON** for an existing level file. **Export JSON** downloads the complete level set. To ship your version, replace `assets/levels.json` with the export and rebuild the Android APK.
- **Mesh** opens the bundled OBJ meshes or an OBJ from disk. Select a vertex in the preview or by number, edit its XYZ coordinates, then export the OBJ. Replace the game asset with that OBJ to ship the mesh change. Game meshes require face indices for positions, UVs, and normals. The vertex editor preserves the existing UVs, normals, and topology. Bullet collision shapes remain separate.

The browser uses the Android renderer's Filament version, shader packages, KTX environment, mesh variants, and textures. Its viewport is an orbitable authoring preview. Android handles gameplay and platform motion. Target and platform order matters for existing material overrides because the game saves them by placement index.

## Pirate redesign

The game now uses pirate cargo, a sunset harbor backdrop, modeled timber docks and cannon, and a brass-framed nautical HUD. The browser editor uses the same meshes, textures, and surface finishes. See [design and regeneration notes](docs/pirate/README.md) for the asset catalog, geometry budgets, rendering details, and collision-shape limitations.
