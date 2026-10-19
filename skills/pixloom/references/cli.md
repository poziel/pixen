# CLI workflow

Use `pixloom-cli` to turn explicit pixel decisions into editable projects and rendered
exports. Read [the authoring reference](authoring-format.md) before
authoring JSON. Use [static](../examples/static.pixloom.json) and
[animated](../examples/animated.pixloom.json) as structural starting points; derive artwork
from the user's brief and chosen sprite-design skill.

1. Run `pixloom-cli --version`. These instructions target 0.2.0 and authoring versions 1 (legacy grids) and 2 (explicit cel encodings).
   If unavailable, use the repository's locally built `target/release/pixloom-cli`
   (`pixloom-cli.exe` on Windows). `cargo build --release --locked` is sufficient;
   do not install globally or publish without the user's request.
2. Author version-2 `.pixloom.json` palette-index documents with exact `#RRGGBBAA` colours. Name cels/layers,
   supply every timeline entry, and choose editing links deliberately. Reusing a
   cel stays independent unless timeline entries share a layer-local `link`.
3. Validate, then choose outputs:
   ```sh
   pixloom-cli validate sprite.pixloom.json
   pixloom-cli build sprite.pixloom.json --output assets/sprite --formats pxo,aseprite
   pixloom-cli export sprite.pixloom.json --output assets/sprite --formats png,svg,gif,webp,apng
   ```
   `--output` is an extensionless directory/filename stem; the parent must exist.
   Use fresh paths or `--force` for authorized replacements. PNG is optional.
4. View a rendered PNG to check silhouette, alpha, layer order and placement.
   Revise JSON and rebuild/export until it matches the brief. PNG is an
   inspection output; keep the numeric document as the authoring source.
5. For animations, inspect the native project and a contact sheet:
   ```sh
   pixloom-cli inspect assets/sprite.aseprite
   pixloom-cli render assets/sprite.pxo --contact-sheet --columns 8 --output sheet.png
   ```
   Use valid column counts (1..frame count). Static exports default to frame 1;
   GIF/WebP/APNG export every frame once by default; add `--loop` to repeat indefinitely. `--frame N` selects a still image and
   is one-based. Tags label ranges and never add/repeat frames.

Choose encoding per cel: `grid` for easy explicit authoring, cropped `hybrid-rle`
for clustered sprites, `sparse-rows` for scattered content, `row-table` for repeated
complete rows, or `tiles` for repeated blocks. No encoding is universally smallest.
Cropping uses paired `offset`/`size`, never a separate identifier. Sparse gaps use
`default_index`, never previous-frame content. Dictionary references address local
rows/tiles; pixel tokens and `default_index` address the root colour palette.
Storage reuse never creates editing links. See the reference for exact payloads,
fill/padding rules, canonical writers and resource limits, and the
[all-encoding animation](../examples/encodings.pixloom.json) for equivalent cels.

Optional video uses `--formats mp4,webm`; only video requires FFmpeg. It needs at
least two source frames and rejects `--frame`. MP4 uses lossless H.264 RGB;
WebM uses lossless VP9 YUV444 with possible RGB conversion rounding. Neither
preserves alpha. Transparent content needs explicit opaque
`--background "#RRGGBBFF"` for video, JPEG, and BMP; GIF needs it for partial
alpha. A supplied background flattens alpha in every requested export, so use a
separate request when retaining transparency in PNG/WebP/APNG is required.
GIF quantizes palettes above 256 colours and timings to 10 ms; WebP/video use
1 ms, APNG normally 0.1 ms, all using cumulative endpoint rounding.

Use `pixloom-cli unpack existing.pxo --output editable/sprite.pixloom.json` or the same
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

## Optimize storage after authoring

Use any convenient valid encoding; do not predict the smallest representation.
Run `pixloom-cli optimize sprite.pixloom.json --in-place`, or use
`--output optimized` to generate `optimized.pixloom.json`. An explicit destination
is required; separate existing files require `--force`. Optimization preserves
exact palette indices, palette order, IDs and editing links and emits minified
JSON. The report separates formatting/normalization savings from encoding savings.
The bounded search is deterministic, byte-idempotent, and does not promise global
optimality. Version-1 files retain legacy grids. See the repository
[search and safety contract](../../../docs/optimizer.md) for candidate choices and limits.

## Compressed container workflow

Use `pixloom-cli pack sprite.pixloom.json --output sprite.pixloom` to preserve all
encoding choices, or add `--optimize` to run lossless JSON optimization first.
Containers accept the same validate/build/export workflows. Unpack with
`pixloom-cli unpack sprite.pixloom --output sprite.pixloom.json`; native unpack still
works. Never rename JSON to `.pixloom`: it is a ZIP format.

`inspect sprite.pixloom` checks structure/references without loading cel payloads;
`validate` checks all payloads, including unused definitions. Individual cel
loading is available through the repository Reader API. Build/export fully load
and validate the document. Shared storage never implies linked editing.
Standalone optimize rejects containers with unpack/repack guidance. Preserve the
source and use fresh destinations or explicitly authorized `--force` replacement.
See [container format](../../../docs/container-format.md) for exact schemas, versions,
strict entry policy and resource limits, and
[benchmarks](../../../docs/container-benchmarks.md) for compression tradeoffs.
