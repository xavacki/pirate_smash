#!/usr/bin/env python3
"""Stage the runtime assets consumed by the Android activity."""
import hashlib
from pathlib import Path
import sys
import zipfile

root = Path(__file__).resolve().parents[1]
destination = Path(sys.argv[1])
destination.mkdir(parents=True, exist_ok=True)
files = {
    "assets/levels.json",
    "assets/audio/fire.wav",
    "assets/audio/fire2.wav",
    "assets/audio/fire3.wav",
    "assets/audio/fire4.wav",
    "assets/audio/fire5.wav",
    "assets/start-screen/atlas.png",
    "assets/recovered/meshes/CannonBottom.obj",
    "assets/recovered/meshes/Cannfon3.obj",
    "assets/recovered/textures/Cannon_1_65.png",
    "assets/objects/manifest.txt",
    "assets/objects/fragments.txt",
}
files.update(path.relative_to(root).as_posix()
             for path in (root / "assets/filament").iterdir() if path.is_file())
for name, first_asset_column in [("manifest.txt", 4), ("fragments.txt", 2)]:
    for line in (root / "assets/objects" / name).read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            files.update(line.split()[first_asset_column:first_asset_column + 2])

files.update(path.relative_to(root).as_posix()
             for path in (root / "assets/pirate").iterdir() if path.suffix in {".png", ".obj", ".json"})

archive = destination / "game-assets.zip"
with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as bundle:
    for path in sorted(files):
        bundle.write(root / path, path)
checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
(destination / "game-assets.sha256").write_text(checksum + "\n")
