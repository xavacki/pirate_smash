#!/usr/bin/env python3
"""Serve the editor and game assets on the local machine."""
import argparse
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

EDITOR = Path(__file__).resolve().parent
ASSETS = EDITOR.parent / "assets"


class EditorHandler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

    def translate_path(self, path):
        requested = unquote(urlsplit(path).path).lstrip("/")
        if requested.startswith("assets/"):
            root, requested = ASSETS, requested[len("assets/"):]
        else:
            root = EDITOR
        target = (root / (requested or "index.html")).resolve()
        return str(target if target.is_relative_to(root) and target.is_file() else root / ".missing")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8050)
    args = parser.parse_args()
    if not (EDITOR / "vendor/filament.wasm").is_file():
        parser.exit(1, "Run bash web-editor/setup-filament.sh before starting the server.\n")
    with ThreadingHTTPServer(("127.0.0.1", args.port), EditorHandler) as server:
        print(f"Smash Land editor: http://localhost:{server.server_port}/", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
