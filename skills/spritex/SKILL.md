---
name: spritex
description: Author exact pixel-art JSON and compile editable Pixelorama/Aseprite projects or rendered static images, animations, and optional FFmpeg video with the standalone spritex CLI. Use for layered sprites and explicit frame timelines; combine with separate art-direction skills.
---

# spritex

Use `spritex` to turn explicit pixel decisions into editable projects and rendered
exports. Read [the schema-1 reference](references/authoring-format.md) before
authoring JSON. Use [static](examples/static.json) and
[animated](examples/animated.json) as structural starting points; derive artwork
from the user's brief and chosen sprite-design skill.

1. Run `spritex --version`. These instructions target 0.2.0 and authoring schema 1.
   If unavailable, use the repository's locally built `target/release/spritex`
   (`spritex.exe` on Windows). `cargo build --release --locked` is sufficient;
   do not install globally or publish without the user's request.
2. Author palette-index grids with exact `#RRGGBBAA` colours. Name cels/layers,
   supply every timeline entry, and choose editing links deliberately. Reusing a
   grid stays independent unless timeline entries share a layer-local `link`.
3. Validate, then choose outputs:
   ```sh
   spritex validate sprite.json
   spritex build sprite.json --output assets/sprite --formats pxo,aseprite
   spritex export sprite.json --output assets/sprite --formats png,svg,gif,webp,apng
   ```
   `--output` is an extensionless directory/filename stem; the parent must exist.
   Use fresh paths or `--force` for authorized replacements. PNG is optional.
4. View a rendered PNG to check silhouette, alpha, layer order and placement.
   Revise JSON and rebuild/export until it matches the brief. PNG is an
   inspection output; keep the numeric grids as the authoring source.
5. For animations, inspect the native project and a contact sheet:
   ```sh
   spritex inspect assets/sprite.aseprite
   spritex render assets/sprite.pxo --contact-sheet --columns 8 --output sheet.png
   ```
   Use valid column counts (1..frame count). Static exports default to frame 1;
   GIF/WebP/APNG export every frame once. `--frame N` selects a still image and
   is one-based. Tags label ranges and never add/repeat frames.

Optional video uses `--formats mp4,webm`; only video requires FFmpeg. It needs at
least two source frames and rejects `--frame`. MP4 uses lossless H.264 RGB;
WebM uses lossless VP9 YUV444 with possible RGB conversion rounding. Neither
preserves alpha. Transparent content needs explicit opaque
`--background "#RRGGBBFF"` for video, JPEG, and BMP; GIF needs it for partial
alpha. A supplied background flattens alpha in every requested export, so use a
separate request when retaining transparency in PNG/WebP/APNG is required.
GIF quantizes palettes above 256 colours and timings to 10 ms; WebP/video use
1 ms, APNG normally 0.1 ms, all using cumulative endpoint rounding.

Use `spritex unpack existing.pxo --output editable/sprite.json` or the same
command with `.aseprite`/`.ase` input. Output identifies the JSON filename,
not a directory. Unsupported features cause errors; do not flatten or silently
remove them to force conversion. Aseprite requires whole-millisecond durations
1..65535, rounds opacity to a byte, and rejects unused cel definitions. PXO
supports the documented version-7 RGBA raster subset. `inspect` emits JSON;
PXO metadata inspection alone does not validate raster buffers, while Aseprite
inspection currently runs the full reader.

Report JSON/native/export paths, visual checks actually performed, and relevant
compatibility limits. The CLI owns validation, output preparation and resource
bounds; this skill supplies the authoring/inspection workflow without an art style.
