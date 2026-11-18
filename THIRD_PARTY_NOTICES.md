# Third-party rendering code

The OpenGL renderer, HDR framebuffer/MSAA handling, five-level bloom, ACES tone mapping, screenshot capture, and HUD triangle/glyph helpers are adapted from the user's local **Ion Bloom** example at `C:\3d_examples` (`/mnt/c/3d_examples`). The shaders and renderer were extended for the recovered game's textured OBJ models, per-body Bullet transforms, directional/hemisphere lighting, and existing gameplay camera.

Copyright (c) 2026 Ion Bloom contributors. The MIT license is preserved in [third_party/ion-bloom-LICENSE](third_party/ion-bloom-LICENSE). The example's separate Primary Ion Drive model is outside this adaptation; all game meshes and textures here come from the supplied Smash Land package.

Bullet 3.25 is built from its original source in `.tools/bullet3`; its license is included there. Rust dependency versions are recorded in Cargo.lock; their upstream license files accompany their packages in the Cargo registry cache.

SDL 3.4.14 is bundled from the Cargo-pinned `sdl3-src` package and statically linked via `sdl3-sys`. Its zlib license is preserved in [third_party/SDL-LICENSE.txt](third_party/SDL-LICENSE.txt).
