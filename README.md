<div align="center">

<img src="assets/pixen.png" alt="Pixen logo" width="48" height="48">

# Pixen

**Weave pixels into projects.**

A desktop pixel-art editor with animation, AI-assisted drawing, and scriptable exports to images, animations, Aseprite, and Pixelorama.

[Get started](#get-started) · [Authoring reference](skills/pixen/references/authoring-format.md) · [Examples](#examples) · [Commands](#commands)

<img src="examples/floating-garden/floating-garden.gif" alt="A floating garden with a gently turning windmill, swaying flowers, and a flowing waterfall" width="480" height="384">

*A little garden in the sky. 160 × 128 pixels · 6 layers · a seamless 4-second loop. [Explore the example](examples/floating-garden/README.md).*

</div>

## What is Pixen?

Pixen combines a desktop editor with a Rust document engine, command-line tools,
and an MCP server. Draw by hand, animate layers and cels, or ask an AI assistant
to edit the artwork in the same document. Save editable `.pixloom` projects or
readable `.pixen.json` sources, then export assets for your game or application.

- **Desktop editing:** document tabs, drawing and selection tools, shape and text
  tools, color controls, layers, and recoverable undo/redo.
- **Animation:** a layer/frame timeline matrix, cel operations, editable frame
  durations, playback controls, and preview-speed adjustments.
- **AI art chat:** per-document conversations, image and text references, and
  installed provider tools connected to the current artwork through MCP.
- **Batch exports:** images, sprite sheets, animations, video, and editable native
  projects through the desktop export dialog or CLI.
- **Scriptable artwork:** explicit palette indices, structured JSON, validation,
  lossless source optimization, and revision-checked MCP editing.
- **Customizable workspace:** themes, persisted settings and keyboard shortcuts,
  plus default, portable, and semi-portable storage modes on Windows.

The desktop uses Frame/Vue in a Tauri host. The Rust CLI and MCP server can also
run headlessly. AI features require separately installed and authenticated
provider tools; video export requires FFmpeg. See [manual editing](docs/manual-editing.md),
[AI chat](docs/ai-chat.md), and [storage modes](docs/portable.md).

Windows is the current locally developed desktop target. GitHub workflows target
Windows, macOS, and Linux; desktop releases await published Frame dependencies
and platform verification. Application screen-color capture is Windows-only.
See [GitHub builds and release setup](docs/releases.md).

## Get started

### Install from source

You need **Rust 1.89 or later**, including Cargo, and a local checkout of this repository. Run these commands from the repository root:

```sh
cargo install --path . --locked
pixen-cli --version
pixen-cli --help
```

Cargo installs the headless `pixen-cli` and `pixen-mcp` executables in its bin directory, which must be on your `PATH`. With the default Cargo setup, that is `~/.cargo/bin` on Linux/macOS or `%USERPROFILE%\.cargo\bin` on Windows.

Build the desktop separately with **Bun**, Rust, and the
[Tauri native prerequisites](https://tauri.app/start/prerequisites/).
The current checkout also requires sibling [Frame](https://github.com/poziel/frame)
and Frame Native sources:

```text
1-codes/
  pixen/
  frame/          # packages/core and packages/tauri
  frame-native/   # agents Rust crate
```

From Frame, run `bun install --frozen-lockfile`, `bun run build:library`, and
`bun run --cwd packages/tauri build`. Then run the commands below from Pixen.
Versioned package dependencies are planned; manifests currently use local paths,
so cloning Pixen alone is sufficient only for the headless tools.

```sh
bun install --frozen-lockfile
bun run dev
# Production desktop, without creating an installer:
bun run build
```

Build the headless tools with `bun run build:tools` (or `cargo build --release --locked`) as well. Use `bun run build:cli` or `bun run build:mcp` to build either tool individually, without Tauri. The shared
target directory places all three executables under `target/release`. Distribute
them together so CLI/MCP preview requests can find `pixen` beside their own
executable. A Cargo-only installation does not install the desktop.
See [desktop architecture and verification](docs/frame-desktop.md).

To create a Windows ZIP containing the desktop, CLI, and MCP server, run:

```powershell
bun run release
```

This rebuilds Frame's library, refreshes Pixen's locked local dependencies, then
builds all three release executables. The folder and ZIP are written under
`target/portable/Pixen-<version>-<Rust-host-target>`. Keep the executables together
for MCP live previews. The destination computer needs WebView2; preferences still
use its application data folders by default. Create an extensionless `portable` file
beside the executables to use a local `data/` folder instead. Both AI skills ship
under `skills/`; see [portable storage and skill bundles](docs/portable.md). Configure your MCP client to launch the
extracted `pixen-mcp.exe` with `args = ["--viewer"]`.

To build without installing:

```sh
cargo build --release --locked
```

Then use `./target/release/pixen-cli` on Linux/macOS or `.\target\release\pixen-cli.exe` in PowerShell in place of `pixen-cli` below.

### Export your first artwork

From the repository root, validate the included animated example and export it:

```sh
pixen-cli validate skills/pixen/examples/animated.pixloom.json
pixen-cli export skills/pixen/examples/animated.pixloom.json --output heart --formats png,svg,gif
pixen-cli build skills/pixen/examples/animated.pixloom.json --output heart --formats pxo,aseprite
```

You now have:

| File | What it contains |
| --- | --- |
| `heart.png` | A transparent still of the first frame |
| `heart.svg` | Pixel-aligned vector artwork of the first frame |
| `heart.gif` | The full animation, encoded for one pass |
| `heart.pxo` | An editable Pixelorama project |
| `heart.aseprite` | An editable Aseprite project |

Open the native project in its editor to continue drawing, or use the exported images in your game or application.

**Output paths:** `build` and `export` take a filename stem without an extension. For example, `--output assets/heart` creates `assets/heart.png`. Create the destination directory first. Existing outputs require `--force` to overwrite.

## Make your own pixel art

Save this as `spark.pixen.json`:

```json
{
  "version": 2,
  "canvas": { "width": 3, "height": 3 },
  "palette": ["#00000000", "#F4C95DFF"],
  "cels": [
    {
      "id": "spark",
      "encoding": "grid",
      "pixels": [
        [0, 1, 0],
        [1, 1, 1],
        [0, 1, 0]
      ]
    }
  ],
  "frames": [{ "duration_ms": 125 }],
  "layers": [
    { "id": "art", "name": "Art", "timeline": [{ "cel": "spark" }] }
  ]
}
```

Each number selects a zero-based entry in `palette`. Here, `0` is transparent and `1` is gold. Colours use `#RRGGBBAA`; transparency comes from the alpha value, not a special palette index.

A **cel** holds pixel data. A layer's **timeline** selects a cel for each frame. To animate, add frames with durations in milliseconds, define the artwork for them, and add a matching timeline entry to every layer. Layers are ordered from bottom to top.

```sh
pixen-cli validate spark.pixen.json
pixen-cli export spark.pixen.json --output spark --formats png,svg
pixen-cli build spark.pixen.json --output spark --formats pxo,aseprite
```

Version 2 supports five pixel encodings: `grid`, `hybrid-rle`, `sparse-rows`, `row-table`, and `tiles`. Start with a grid and let the optimizer find a more compact representation later. Version 1 documents remain readable.

See the [complete authoring reference](skills/pixen/references/authoring-format.md) for placement, palettes, tags, annotations, and explicit editing links. Reusing a cel definition does not link edits; shared editing requires a layer-local `link` group.

## MCP live canvas

`pixen-mcp --viewer` lets an MCP client create, draw, inspect actual PNG previews,
and save pixel art while a separate read-only window displays its work. The
`open_preview` tool also opens existing static or animated projects, with local
playback and frame thumbnails. Open supported projects for editing with
`open_project`; `.pixloom` is the default save format, with explicit native/image
exports available. Revision-checked palette, region, layer, frame and drawing
tools share atomic batches and bounded undo/redo. `resize_canvas` changes canvas
bounds with revision checks and undo, cropping or padding without scaling pixels. Onion-skin, frame-comparison
and isolated-layer images help the AI review its work; inspection modes also
appear in the viewer menu and synchronize while Follow AI is enabled.
Use `pixen-mcp` for headless editing and image review.
See [setup, client configuration and tool examples](docs/mcp.md).
[Art-review APIs](docs/art-review.md) add review sheets, immutable checkpoints,
component/color measurements, explicit validation rules and animation audits.
The inspector displays their evidence without modifying artwork.

## Commands

Import existing raster artwork into an editable document:

```powershell
pixen-cli import sprite.png --output "D:\Art\my-sprite.pixen.json"
pixen-cli import sprite.png --output "D:\Art\my-sprite.pixloom"
pixen-cli import sprite.jpg --output "D:\Art\trimmed.pixloom" --transparent-color "#FFFFFF" --color-tolerance 20 --transparent-mode border --trim
```

PNG, JPEG, WebP, BMP and the first TIFF image, plus GIF, APNG and animated WebP, import as one
`Artwork` layer with exact decoded RGBA8 pixels and a shared palette. Optional
colour removal precedes trimming; animated trimming uses one shared crop.
See [raster import](docs/raster-import.md) for tolerance semantics, limits,
animation timing and loop metadata limitations.

Run `pixen-cli <command> --help` for all options.

| Command | Input | Purpose |
| --- | --- | --- |
| `import` | PNG, JPEG, GIF, APNG, WebP, BMP, or TIFF | Create editable `.pixen.json` or `.pixloom` |
| `validate` | Authoring JSON or `.pixloom` | Check the document, pixels, and editing links |
| `build` | Authoring JSON or `.pixloom` | Create editable `.pxo` and `.aseprite` projects |
| `export` | Authoring JSON or `.pixloom` | Export images, animations, or video |
| `pack` | Authoring JSON | Create a compressed `.pixloom` container |
| `unpack` | `.pixloom`, `.pxo`, or `.aseprite` | Write editable authoring JSON |
| `optimize` | Authoring JSON | Losslessly compact JSON pixel storage |
| `inspect` | `.pixloom`, `.pxo`, or `.aseprite` | Print structure or native metadata as JSON |
| `render` | `.pixloom`, `.pxo`, or `.aseprite` | Render a PNG frame or contact sheet |

### Inspect and convert native projects

```sh
pixen-cli inspect heart.aseprite
pixen-cli render heart.pxo --contact-sheet --columns 2 --output heart-sheet.png
pixen-cli unpack heart.aseprite --output editable.pixen.json
pixen-cli build editable.pixen.json --output converted --formats pxo
```

`render` and `unpack` take a complete output filename. Frame numbers start at **1**; use `render --frame N` to select a frame. `render` writes PNG only.

Native input supports a defined subset of each editor's format. Pixelorama targets **v1.2.3, PXO version 7**. Supported raster layers, animation timing, tags, and editing links are retained. Unsupported features such as groups, effects, and non-RGBA modes produce errors. Native projects are not a general archival round-trip format; see the [PXO](docs/pxo-v7.md) and [Aseprite](docs/aseprite-format.md) documentation for details.

### Optimize and package a source

```sh
pixen-cli optimize spark.pixen.json --output spark-small
pixen-cli pack spark-small.pixen.json --output spark.pixloom
pixen-cli validate spark.pixloom
pixen-cli unpack spark.pixloom --output restored.pixen.json
```

Separate optimizer outputs get the `.pixen.json` suffix. Use `--in-place` instead of `--output` to optimize the original file.

A `.pixloom` file is a ZIP container with independently compressed cel payloads. Packing preserves encodings by default; add `pack --optimize` to optimize first. Containers retain unused cels, palette indices, metadata, and explicit links. Use either authoring format with `build` and `export`.

Read more about [optimization](docs/optimizer.md) and [containers](docs/container-format.md).

## Export formats

| Format | Output | Transparency | Notes |
| --- | --- | --- | --- |
| PNG | Still | Full alpha | Exact rendered RGBA |
| SVG | Still | Opacity | Pixel-aligned shapes; no embedded PNG or tracing |
| JPEG | Still | None | Lossy, quality 95 |
| BMP | Still | None | Exact rendered RGB |
| TIFF | Still | Full alpha | Exact rendered RGBA |
| GIF | Animation | Binary alpha | Quantizes palettes above 256 colours |
| WebP | Still or animation | Full alpha | Lossless VP8L |
| APNG | Animation | Full alpha | Exact rendered RGBA |
| MP4 | Video | None | FFmpeg; H.264 RGB, CRF 0 |
| WebM | Video | None | FFmpeg; lossless VP9 with RGB/YUV conversion rounding |

Still outputs use frame 1 by default. GIF, WebP, and APNG use the full timeline once by default (`--loop` repeats indefinitely); single-frame sources produce still images. `export --frame N` selects a still frame for any image format. Video requires at least two frames and does not accept `--frame`.

JPEG, BMP, MP4, and WebM need an opaque `--background` when the artwork contains transparency. GIF also needs one for partial alpha. A supplied background is applied to **every image and video in that export request**. Leave it out to preserve transparency in formats that support it.

```sh
pixen-cli export skills/pixen/examples/animated.pixloom.json --output frame --formats png,svg --frame 3
pixen-cli export skills/pixen/examples/animated.pixloom.json --output opaque --formats jpeg,bmp --background "#203040"
```

Pixen preserves the canvas dimensions without resizing, smoothing, or antialiasing. Image animations encode one pass; a looping preview may have separate playback settings. See [image encoder policies](docs/image-encoders.md) for timing precision and format limits.

### Optional video export

Install FFmpeg with the `libx264rgb` encoder for MP4 or `libvpx-vp9` for WebM, then make it available on `PATH`:

```sh
pixen-cli export skills/pixen/examples/animated.pixloom.json --output heart-video --formats mp4,webm --background "#203040"
```

Use `--ffmpeg PATH` to select a specific executable. Video uses variable frame timing and the original canvas dimensions. See [video codecs and limits](docs/video-export.md) for codec details and playback compatibility.

## Examples

- [Floating garden](examples/floating-garden/README.md): the transparent README animation, with editable layers, native projects, and a light/dark preview.

- [Small static source](skills/pixen/examples/static.pixloom.json): a simple grid to edit by hand.
- [Small animated source](skills/pixen/examples/animated.pixloom.json): explicit frame timing and cel reuse.
- [All five encodings](skills/pixen/examples/encodings.pixloom.json): equivalent artwork stored in different ways.
- [Ember walk](examples/ember-walk/README.md): an editable animated character example.

## Output safety and compatibility

Pixen checks output paths before writing and requires `--force` to replace existing files. Input/output aliases, colliding destinations, and output symlinks are rejected. Files are staged before installation; installation is atomic per file, rather than a transaction across all outputs.

Canvas sizes, frame counts, expanded pixel data, archives, and encoded outputs have bounded resource limits. Rendering uses RGBA source-over compositing without colour management; editor blend rounding can differ slightly. Consult the individual format documents for precise bounds and supported features.

Pixen was previously named Pixloom and, before that, Spritex. The binary project extension remains `.pixloom`; editable JSON now uses `.pixen.json`. Container identifiers and metadata remain unchanged. Saved AI preferences fall back to the previous Pixloom settings directory; new preferences are written under Pixen. Existing `.spritex.json` documents, `.spritex` containers, and legacy Aseprite metadata envelopes remain readable. Reserved `_spritec*` provenance keys are retained for compatibility.

## Development

```sh
cargo test --locked
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

Optional video integration checks require FFmpeg and ffprobe:

```sh
cargo test --locked --test video --test commands -- --ignored --nocapture
```

The Rust library separates authoring/schema validation, rendering, native codecs, and export operations from CLI parsing. Tests cover independent editor fixtures, pixel comparisons, animation timing, editing links, malformed inputs, and output safety.

Technical references: [authoring](skills/pixen/references/authoring-format.md) · [Aseprite](docs/aseprite-format.md) · [Pixelorama](docs/pxo-v7.md) · [images](docs/image-encoders.md) · [video](docs/video-export.md) · [containers](docs/container-format.md) · [benchmarks](docs/container-benchmarks.md).


For multi-frame MCP authoring, labelled AI contact sheets and looping delivery,
see the [animation workflow](docs/mcp.md#authoring-and-delivering-animation) and
[reproducible bouncing-orb example](examples/mcp-animation/README.md).

## Desktop app and AI live view

Run `pixen` to open the editor, or `pixen "path/to/project.pixloom"` to load
artwork immediately. Open files through the application menu or drag and drop.
Documents use independent tabs, with layers, drawing tools, AI chat, and a
layer/frame timeline matrix sharing the active artwork.

Supported inputs include `.pixloom`, authoring JSON, Pixelorama `.pxo`, RGBA
Aseprite `.ase`/`.aseprite`, PNG/APNG, JPEG, GIF, WebP, BMP, and the first TIFF
image. Native format limitations still apply; see the format references above.
Use **File > Export** for batch outputs and [manual editing](docs/manual-editing.md)
for drawing, selection, and timeline behavior.

Configure an installed AI provider in Settings, then open AI chat for an artwork.
Connections start with the first message. Reference attachments and a fresh
artwork overview accompany requests. Conversations remain available while
switching document tabs during a run; transcripts are not restored after restart.
See [AI chat and provider setup](docs/ai-chat.md).

The same desktop also displays managed MCP previews. These are read-only for
manual tools and follow the MCP session. Keep the three executables together
when desktop previews are wanted:

- `pixen`: desktop editor and managed preview window.
- `pixen-cli`: file commands and conversions.
- `pixen-mcp`: stdio MCP server (`pixen-mcp --viewer`).

Windows executables have the `.exe` suffix. The MCP server runs only through
`pixen-mcp`; the CLI has no MCP subcommand.

On Windows, right-click a `.pixloom` file, choose **Open with → Choose another app**,
browse to `target/release/pixen.exe`, and choose **Always**. Update existing MCP
client configurations to point to `pixen-mcp.exe`, with `--viewer` as the optional argument. Pixen does not modify file
associations or MCP configurations automatically.

The menu switches animations between playback and a sprite-sheet view. Static
images enable animation view only when a regular square-cell strip or grid with
uniform gutters can be recognized; ordinary or ambiguous images leave the option
disabled. Detected image sheets play at 100 ms per frame by default because the
image contains no timing metadata. Conversions are read-only, use row-major order,
and retain the original image when switching back. Sprite-sheet rendering obeys
the existing canvas and pixel limits.

Below playback, the footer shows frame duration on the left, the filename in the
center, and frame position on the right. Errors replace the filename for eight
seconds and then disappear; long text truncates and is available on hover.

Interface icons come from Frame, Lucide, and inline SVG components bundled by Vite. No runtime `icons/` folder is required or shipped.

## Source layout

- `core/`: shared Rust engine and application services, with executable entry
  points at `core/bin/pixen-cli.rs` and `core/bin/pixen-mcp.rs`.
- `src/`: Vue and Frame frontend.
- `src-tauri/`: native desktop host, Tauri commands, dialogs and configuration.

The root `Cargo.toml` defines the shared library at `core/lib.rs` and both
headless binaries. The desktop has its own manifest and depends on that library.
`bun run dev` runs the desktop with frontend hot reload; standalone browser
execution and the former web bridge are no longer supported.

See [Code layout](docs/code-layout.md) for the source and test folder guide.
