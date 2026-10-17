# spritex

Compile exact pixel-art JSON into editable projects, rendered images, or animation
video. The Rust CLI authors directly from palette-index grids; PNG is optional.
Image exports and native projects need no editor, MCP server, or image service.
Only MP4 and WebM require an optional FFmpeg installation.

## Use

```sh
spritex validate skills/spritex/examples/animated.json
spritex build skills/spritex/examples/animated.json --output heart --formats pxo,aseprite
spritex export skills/spritex/examples/animated.json --output heart --formats png,svg,gif,webp,apng
spritex export skills/spritex/examples/animated.json --output heart-video --formats mp4,webm --background "#203040FF"
spritex export skills/spritex/examples/animated.json --output selected --formats png,jpeg --frame 3 --background "#FFFFFFFF"
spritex inspect heart.aseprite
spritex render heart.pxo --contact-sheet --columns 2 --output sheet.png
spritex unpack heart.aseprite --output editable.json
spritex build editable.json --output rebuilt --formats pxo,aseprite
```

For **export/build**, `--output` is an **extensionless path containing the
destination directory and filename stem**: `assets/heart` produces
`assets/heart.png`, `assets/heart.svg`, etc. The parent directory must exist.
Do not pass `heart.png` or a directory alone. The defaults are `png` for
`export` and `pxo` for `build`. Format lists ignore case, accept aliases
`jpg`, `tif`, and `ase`, and deduplicate selections. Output extensions are
canonical: `.jpeg`, `.tiff`, and `.aseprite`.

For **unpack**, `--output` is the JSON **filename**, such as
`editable/heart.json`. For **render**, it is the PNG filename.
Frame numbers start at **1**.

Static formats use frame 1. GIF, WebP, and APNG use the complete timeline once.
An explicit `--frame N` produces a still image for every image choice.
Single-frame sources produce still images. Video requires at least two source
frames and rejects `--frame`. In a mixed `png,gif,mp4` request, PNG uses frame
1 and GIF/MP4 use every frame; tags never repeat or add frames.

## Capabilities

“Read” means native-project input for `inspect`, `render`, or `unpack`.
All writes below consume schema-1 authoring JSON. Image/video writes flatten
visible layers; native writes retain supported editable structure.

| Format | Read | Write | Layers | Animation | Alpha | Lossless pixels / limits |
| --- | --- | --- | --- | --- | --- | --- |
| Pixelorama PXO 7 | Yes | Yes | Editable raster | Yes | RGBA | Exact raw cels; supported v1.2.3 subset |
| Aseprite / ASE input | Yes | Yes | Editable raster | Yes | RGBA | Exact raw cels; opacity rounds to a byte; integer ms timing |
| PNG | No | Yes | Flattened | Frame 1 | RGBA | Exact rendered RGBA |
| SVG | No | Yes | Flattened | Frame 1 | Opacity | Pixel-aligned runs; no embedded PNG or tracing |
| JPEG / JPG | No | Yes | Flattened | Frame 1 | None | Lossy, quality 95 |
| BMP | No | Yes | Flattened | Frame 1 | None | Exact rendered RGB |
| TIFF / TIF | No | Yes | Flattened | Frame 1 | RGBA | Exact rendered RGBA; single image |
| GIF | No | Yes | Flattened | Full timeline | Binary | Local palettes; quantization above 256 colours |
| WebP | No | Yes | Flattened | Full timeline | RGBA | Lossless VP8L, including animation |
| APNG | No | Yes | Flattened | Full timeline | RGBA | Exact rendered RGBA; separate from still `png` |
| MP4 | No | Yes | Flattened | Full timeline | None | H.264 `libx264rgb`, CRF 0, RGB 4:4:4 |
| WebM | No | Yes | Flattened | Full timeline | None | VP9 `libvpx-vp9`, lossless YUV 4:4:4; RGB conversion rounds |

JPEG, BMP, MP4, and WebM require an explicit opaque `--background "#RRGGBB"`
or `"#RRGGBBFF"` whenever rendered pixels contain transparency. GIF accepts
binary alpha but requires a background for partial alpha. **Supplying a
background composites it into every requested image/video**, including formats
that otherwise preserve alpha. Omit it to keep alpha in PNG/TIFF/WebP/APNG/SVG.
The CLI does not resize, pad, smooth, interpolate, or antialias pixels.

Animated images encode one pass: GIF omits repeat metadata; WebP/APNG use a
play count of one. Timing rounds cumulative endpoints, bounding total error to
half a format tick: GIF 10 ms, WebP/video 1 ms, and APNG normally 0.1 ms.
APNG lowers precision for long frames; zero-tick or overflowing delays are
errors. Video preserves variable timing and the original dimensions, including
odd sizes. The chosen RGB/4:4:4 codecs prioritize pixel detail; player and
hardware compatibility varies. Neither video configuration preserves alpha.

See the [image encoder policies](docs/image-encoders.md),
[video codecs and limits](docs/video-export.md), and
[Aseprite subset](docs/aseprite-format.md).

## Author exact pixels

The [version-1 reference](skills/spritex/references/authoring-format.md) defines
the canonical JSON. Start with the [static](skills/spritex/examples/static.json)
or [animated](skills/spritex/examples/animated.json) example.

- Palette colours are `#RRGGBBAA`; indices may exceed 255.
- Named cels are full-canvas, row-major grids of palette indices.
- Layers are ordered bottom to top and explicitly supply every timeline entry.
- Reusing a cel definition creates independent editing. Only an explicit
  layer-local `link` group shares editing; equal pixels never imply links.
- Durations are explicit milliseconds. There are no implicit holds or repeats.

The schema remains **version 1** and existing authoring JSON stays valid.
Legacy `_spritec*` PXO provenance keys remain compatible. The renamed
[companion skill](skills/spritex/SKILL.md) provides the author → validate →
build/export → inspect → revise workflow; art direction belongs in separate
sprite-design skills.

## Native conversion and inspection

PXO targets Pixelorama **v1.2.3-stable, version 7**, for supported RGBA raster
layers, normal blends, visibility, opacity, locks, timing, tags, explicit edit
links, annotations, native swatches, and credits. Groups, effects, hierarchy,
clipping, nonzero cel z-index, non-RGBA modes and unsupported visual features
are rejected. Editor/UI/export settings and arbitrary ZIP entries are not
archived. See [PXO format evidence](docs/pxo-v7.md).

Aseprite supports root RGBA raster layers, native locks/link defaults, timing,
forward tags and explicit linked-cel records. Fractional or >65535 ms frame
durations, unsupported features, and unused authoring cel definitions are
rejected. Layer/cel opacity rounds to the nearest native byte. Cropped native
cels must fit the canvas; linked cels with differing offsets are rejected.
Supported annotations use standard user-data envelopes. These can appear as
JSON text in editors, and an editor that removes them loses that extra metadata.
Neither native format is a general archival round-trip guarantee.

Native inputs are detected by signature; extension/signature mismatches produce
an error. `.ase` is an accepted Aseprite input alias. `inspect` emits JSON:
PXO inspection reads metadata without decompressing raster data; Aseprite
inspection currently uses the full bounded decoder. Use `render`/`unpack`
for full supported-subset/raster validation. PXO unsupported-feature reports
may succeed; Aseprite unsupported features fail during decoding.

`render` stays PNG-only: frame 1 by default, `--frame N`, or a contact sheet.
Sheets contain each frame once, left to right then top to bottom, with
transparent unused cells. The default columns are ceil(sqrt(frame count)).
Rendering uses straight RGBA source-over in encoded colour space and rounds at
output. It is not colour-managed; Pixelorama intermediate blends can differ
by a channel level. Raw native cel bytes round-trip exactly.

## Output safety and resource limits

Existing outputs require `--force`. Output/input aliases, colliding paths,
output symlinks/directories, and ambiguous Windows filenames are rejected.
Every requested path is preflighted, and every file is prepared before any is
installed. Installation is atomic per file, **not a cross-file transaction**;
a crash or concurrent filesystem change between installs can leave a subset.
Encoding failures before installation leave no requested finished outputs.
`unpack` writes compact buffered JSON directly to the requested file.

Shared limits include 4096 per canvas side, 4 Mi canvas pixels, 4096 frames,
256 layers, 32768 timeline slots, 64 Mi definition pixels, and 256 MiB JSON/
resident unique cel data. PXO retains its ZIP limits: 512 MiB compressed,
64 MiB per entry, 8 GiB expanded, 65534 entries, 1024-byte names, and 16 MiB
metadata. Contact sheets allow 64 Mi pixels. Aseprite adds bounded files/chunks/
metadata; rendered images cap encoded output at 256 MiB, with SVG also capped
at 100000 runs/16 MiB. Video caps rendered temporary frames at 256 MiB and
accepted encoded output below 128 MiB, with cleanup on success/failure.
See individual codec documents for precise limits and timeouts. Total process
memory can exceed the unique-raster budget.

## Build and distribute

Rust **1.88 or later** is required:

```sh
cargo build --release --locked
# target/release/spritex (Windows: target/release/spritex.exe)
# Optional user-controlled installation:
cargo install --path . --locked
```

Put Cargo's bin directory on PATH if you choose to install it.
No global installation, release publication, or push is performed here.
FFmpeg is optional and only queried for video; use `--ffmpeg PATH` to select
a build containing the required encoders.

For versioned release binaries, build on each target and package the executable,
README, license, references, and skill. Name archives
`spritex-v0.2.0-<target-triple>.zip` (Windows) or `.tar.gz` (Unix), include
SHA-256 checksums, and verify `spritex --version` after extraction.
Hosted binaries are not included or promised.

## Development and verification

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
# Optional external FFmpeg/ffprobe checks:
cargo test --test video -- --ignored --nocapture
cargo test --test commands -- --ignored --nocapture
```

CLI parsing is separate from `operations`, the model/schema, native codecs,
rendering and encoders. Each multi-output request parses JSON and constructs
the model once, then dispatches directly without serializing through PNG.
Tests include independent native fixtures, lossless image decode comparisons,
timing, editing links, malformed inputs, cleanup and overwrite regressions.

See the [spritex verification record](docs/spritex-verification.md).
The original [spritec verification record](docs/verification.md) and research
fixtures remain historical evidence.
