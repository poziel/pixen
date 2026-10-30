# Exporting artwork

Choose a rendered output for a finished asset, or an editable project format to continue working in another editor. Pixen preserves the original canvas dimensions unless you explicitly choose scaling in the desktop export dialog.

| Output | Formats | Guide |
| --- | --- | --- |
| Still image or sprite sheet | PNG, SVG, JPEG, BMP, TIFF, GIF, WebP, APNG | [Image exports](image-encoders.md) |
| Animation | GIF, animated WebP, APNG | [Image exports](image-encoders.md#animation) |
| Video | MP4, WebM; FFmpeg required | [Video exports](video-export.md) |
| Editable Aseprite project | `.aseprite` | [Aseprite](aseprite.md) |
| Editable Pixelorama project | `.pxo`, PXO version 7 | [Pixelorama](pixelorama.md) |
| Pixen project or readable source | `.pixen`, `.pixen.json` | [Pixen project files](../formats/pixen-project.md) |

## Desktop export

Use **File > Export** (`Ctrl+Shift+E`). Select a shared name and output folder, then add entries for the current frame, a sprite sheet, animation, video, or editable Pixen JSON. Each entry has its own format and options. The dialog previews filenames and asks before replacing existing files. Exporting preserves the open project's save destination and dirty state.

The desktop export dialog currently offers Pixen JSON for editable project output. Use the CLI commands below for Aseprite and Pixelorama.

## Command-line export

Rendered images and video use `export`:

```sh
pixen-cli export artwork.pixen --output exports/artwork --formats png,webp
pixen-cli export artwork.pixen --output exports/frame --formats png --frame 3
pixen-cli export artwork.pixen --output exports/loop --formats gif,webp,apng --loop
pixen-cli export artwork.pixen --output exports/video --formats mp4,webm --background "#203040"
```

Editable projects for other editors use `build`:

```sh
pixen-cli build artwork.pixen --output exports/artwork --formats aseprite,pxo
```

The output is an extensionless base path; Pixen adds each format's extension. The parent folder must exist. Existing destinations require `--force`. CLI outputs are prepared before installation, with atomic replacement per file; installation across multiple files is not a transaction.

## Transparency and timing

PNG, WebP, TIFF, APNG, and SVG support transparency. JPEG, BMP, MP4, and WebM need an opaque background when the rendered artwork contains transparency. GIF uses binary alpha: values below 128 become transparent and values at least 128 become opaque. Supply a background when compositing is preferable to this threshold.

Animated images play the timeline once by default; `--loop` requests infinite looping. Saved frame durations determine export timing. Formats have different timing precision and color limits; consult their individual guides before choosing an output for delivery.
