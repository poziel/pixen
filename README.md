# spritec

Compile exact, agent-authored pixel art into editable Pixelorama projects and PNG
previews. A single standalone Rust CLI; no Pixelorama process, GUI, MCP server,
image-generation service, or PNG authoring input is needed at runtime.

Targets **Pixelorama v1.2.3-stable, PXO version 7**, for the supported RGBA raster
subset. Layers, frame timing, tags and explicit edit links survive conversion.
This is not a full-fidelity Pixelorama archive editor.

## Use

```sh
spritec --version
spritec validate skills/spritec/examples/static.json
spritec build skills/spritec/examples/static.json --output spark.pxo --preview spark.png
spritec inspect spark.pxo
spritec render spark.pxo --frame 1 --output frame.png
spritec render animation.pxo --contact-sheet --columns 8 --output sheet.png
spritec unpack spark.pxo --output editable
spritec build editable/sprite.json --output rebuilt.pxo --preview rebuilt.png
```

Preview and frame numbers start at **1**. `build` previews frame 1 at the original
canvas resolution. `render` decodes actual cel buffers and composites them; it
does not read the embedded thumbnail. Contact sheets contain every frame once,
left to right, then top to bottom, with transparent unused cells. The default
column count is the ceiling of the square root of the frame count.

Outputs require an existing parent directory. Existing files/directories are
refused unless `--force` is supplied. `unpack --force` replaces only `sprite.json`
inside an existing real directory and retains unrelated files. Input aliases,
colliding outputs and output symlinks are rejected. Windows output filenames
ending in dot/space or containing alternate-stream colons are rejected. Both build files are prepared
before either is installed; each install is atomic, but the pair is not a
filesystem transaction. A crash or concurrent output creation between installs
can leave only the first output installed.

`unpack` writes compact JSON through buffered output so large pixel grids stay
within the JSON budget. Whitespace is immaterial to the authoring format; the
small supplied examples use readable indentation.

`inspect` emits JSON with dimensions, layer order, visibility/opacity, timing,
tags, native zero-based link membership, and unsupported features. It reads
metadata without decompressing raster buffers; use `render`/`unpack` to validate
the actual raster data. An unsupported-feature report exits successfully;
malformed archives and command failures exit 1, argument errors exit 2.

## Author exact pixels

The [version-1 format reference](skills/spritec/references/authoring-format.md)
defines the canonical JSON representation. Start with the
[static](skills/spritec/examples/static.json) or
[animated](skills/spritec/examples/animated.json) example.

- Palette entries are `#RRGGBBAA`; numeric indices may exceed 255.
- Each named cel is a full-canvas, row-major 2D grid of palette indices.
- Layers are ordered bottom to top. Every layer supplies every frame explicitly.
- Reusing a cel definition creates **independent editing**. Only a named
  layer-local `link` group shares editing. Equal pixels never imply links.
- Frame durations are explicit milliseconds. There is no implicit hold,
  repetition, timeline expansion, or automatic frame padding.

The thin [companion skill](skills/spritec/SKILL.md) describes the author → validate
→ build → inspect PNG → revise workflow. Art direction belongs in your existing
sprite-design skills. The CLI enforces correctness independently of any skill.

## Compatibility

Supported: RGBA8 raster layers, transparency, normal source-over compositing,
layer visibility, layer/cel opacity, frames, timing, inclusive tags, explicit
linked cels, locks/new-cel-link defaults, metadata/user-data, and saved native
RGBA swatches and author/license fields.

Rejected: other PXO versions/colour modes, groups/other layer types, hierarchy,
non-normal blends, effects, clipping, animated layer parameters, nonzero cel
z-index, brushes/reference images/tilesets/guides/vanishing points, and unknown
feature fields. ZIP64, multi-disk and encrypted archives are also unsupported.

Editor selection, UI colours, link-group hues, symmetry/tile-view settings,
export profiles and extra archive entries are not archived. Metadata retention
does not imply compatibility with extensions. The compositor uses straight
RGBA source-over in encoded colour space, high-precision intermediates and
rounding once at output; it is not colour-managed. Pixelorama may quantize
intermediate blends differently. The independently exported large reference
frame differed by **at most one channel level**, with identical alpha. Raw cel
pixels round-trip exactly.

Read [PXO format evidence](docs/pxo-v7.md) and the exact
[verification record and remaining checks](docs/verification.md).

## Install and distribute

Rust 1.85 or later is required to build from source:

```sh
cargo build --release --locked
# Local use: target/release/spritec (Windows: target/release/spritec.exe)

# Optional user-controlled installation; adds spritec to Cargo's bin directory:
cargo install --path . --locked
```

Ensure Cargo's bin directory is on `PATH` (`%CARGO_HOME%\bin` or
`%USERPROFILE%\.cargo\bin` on Windows, `$CARGO_HOME/bin` or `~/.cargo/bin` on Unix).
No global installation or release publication is performed by this project.

For versioned release binaries, build with `--release --locked` on each target,
package only the executable, README, format reference, skill and license, and
name the archives `spritec-v0.1.0-<target-triple>.zip` (Windows) or `.tar.gz`
(Unix). Publish SHA-256 checksums with them and verify `spritec --version` after
extracting to a directory on `PATH`. Release binaries are not included in source
control; no hosted binaries or published release are promised here.

## Development

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

`schema`, `model`, `pxo::v7`, `raster`, `archive`, `output`, and the CLI each have
one responsibility. Resource limits are enforced in Rust: 4096 per canvas side,
4 Mi pixels per canvas, 4096 frames, 256 layers, 32768 timeline slots, 256 MiB
authoring JSON/decoded unique cel data, 64 Mi grid pixels, 512 MiB compressed ZIP,
64 MiB per entry, 8 GiB expanded ZIP, 65534 entries, 1024-byte ZIP names, 16 MiB
PXO metadata, and 64 Mi pixels per contact sheet. Entry-count and expanded-size
limits can further restrict combinations of individually valid dimensions.

Cel buffers are reused in memory without inventing edit links. PXO reading checks
redundant linked buffers incrementally and writing streams one cel at a time.
JSON parsing/unpacking and decoded unique cel buffers remain in memory within
these limits; total process memory can exceed the raw-buffer limit.
