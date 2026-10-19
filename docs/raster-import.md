# Raster import

Import raster pixels into one editable layer named `Artwork`. Static PNG,
JPEG/JPG, WebP and BMP produce one frame. TIFF loads its first image. GIF, APNG (including animated `.png`) and
animated WebP produce every displayed frame, composited with frame offsets,
blending and disposal. Detection uses file contents. No layer reconstruction,
AI interpretation, resizing, stretching or video import is performed.

```powershell
pixloom-cli import sprite.png --output "D:\Art\my-sprite.pixloom.json"
pixloom-cli import sprite.png --output "D:\Art\my-sprite.pixloom"
pixloom-cli import sprite.jpg --output "D:\Art\trimmed.pixloom" --transparent-color "#FFFFFF" --color-tolerance 20 --transparent-mode border --trim
pixloom-cli validate "D:\Art\my-sprite.pixloom"
pixloom-cli build "D:\Art\my-sprite.pixloom" --output "D:\Art\editable" --formats pxo
pixloom-cli export "D:\Art\my-sprite.pixloom.json" --output "D:\Art\preview" --formats png
pixloom-cli pack "D:\Art\my-sprite.pixloom.json" --output "D:\Art\packed.pixloom"
```

`--output` is the complete destination filename, including an arbitrary filename
and existing parent directory. Its suffix must be `.pixloom.json` (ordinary
readable authoring JSON, using the existing compact JSON writer) or `.pixloom`
(the existing compressed container). Both contain the same document. Existing
files require `--force`. Input aliases, including symbolic links and hard links,
are rejected even with `--force`. Output is staged, validated, and atomically
committed using the normal Pixloom output helpers.

Processing order is decode/composite, apply EXIF orientation to every displayed
frame, remove matching colours, then trim. Palette construction and document
validation follow. Decoded RGBA8 values, including partial alpha and RGB values
under transparent pixels, are retained by default. One deterministic palette is
shared by all frames; import never quantizes colours. Existing export formats
retain their own rendering and encoding rules.

- `--transparent-color "#RRGGBB"` sets matching pixels' alpha to zero, preserving
  their RGB values.
- `--color-tolerance N` is an integer from 0 through 255, default 0. A pixel
  matches exactly when `max(abs(R-target_R), abs(G-target_G), abs(B-target_B)) <= N`.
  Alpha does not affect matching. This can help select JPEG background artifacts;
  it does not reverse JPEG damage or remove matte halos.
- `--transparent-mode all|border` defaults to `all`. `all` removes every match.
  `border` removes matching regions reachable from any canvas boundary through
  matching or already fully transparent pixels, using four neighbours (no
  diagonal connections). Enclosed matching details remain. Each composited frame
  is processed independently. Both options require `--transparent-color`.
- `--trim` removes empty outer margins according to alpha > 0. Animations use the
  union of the bounds across all frames and the same crop for every frame, so
  alignment and motion are preserved. Completely transparent animations retain
  their original dimensions. No pixels are resized or stretched.

Limits and metadata:

- Each canvas side must be 1–4096 pixels, with at most 4,194,304 pixels per frame.
  At most 4096 frames and 67,108,864 total decoded pixels are allowed. Encoded
  input and authoring JSON are bounded to 256 MiB; compressed output uses the
  existing 512 MiB limit and 64 MiB per container JSON entry limit. Decoder
  allocation limits remain enabled; underlying codecs treat allocation limits
  as best effort. Dimension and total retained pixel limits are checked separately.
- At most 1,048,576 distinct exact RGBA colours are representable. Higher colour
  counts and inputs above 8 bits per channel fail explicitly. No silent precision
  reduction is performed.
- Animation delays become schema `f64` milliseconds; fractional rational delays
  may incur floating point rounding. The CLI reports this conversion. Zero-delay
  frames fail because the schema requires positive durations. Static images use
  a 100 ms duration because they have no source animation timing.
- Existing native/export timing restrictions still apply: Aseprite requires whole
  millisecond durations and rejects fractional animation durations.
  GIF exports use their existing centisecond timing conversion.
- The schema has no operational loop-count field. Decoder loop information is
  retained in `annotations.metadata.raster_import_loop_count` and reported as a
  warning; existing build/export workflows do not apply that annotation. GIF
  repeat counts are recorded as total plays (repeat count plus the first play).
- EXIF orientation is applied consistently. Other EXIF, colour profiles, comments,
  text and ancillary source metadata are not carried into the document; the CLI
  reports this limitation. Pixels are not colour-managed. WebP animation uses a
  transparent canvas and transparent disposal background, rather than its
  optional background-colour hint. APNG thumbnails excluded from the animation
  are not imported as displayed frames.


