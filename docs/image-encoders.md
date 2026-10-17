# Rendered image encoders

`export::write` renders directly from the validated `Sprite`. It never requires a
PNG intermediate. Rendered exports flatten visible layers with the existing
source-over compositor; native editable projects use `build` instead.

| Choice | Default frames | Pixels and alpha | Encoding |
| --- | --- | --- | --- |
| `png` | Frame 1 | Exact rendered RGBA | PNG, 8-bit RGBA |
| `svg` | Frame 1 | Pixel-aligned horizontal runs; partial alpha as opacity | XML rectangles, no embedded raster |
| `jpeg` / `jpg` | Frame 1 | Lossy RGB; transparent content requires an opaque background | JPEG quality 95 |
| `bmp` | Frame 1 | Exact rendered RGB; transparent content requires an opaque background | 24-bit BMP |
| `tiff` / `tif` | Frame 1 | Exact rendered RGBA | Single-image TIFF |
| `gif` | Entire timeline | Palette colours and binary transparency | Per-frame palettes; NeuQuant if more than 256 colours |
| `webp` | Entire timeline | Exact rendered RGBA | Lossless VP8L, including animation |
| `apng` | Entire timeline | Exact rendered RGBA | Animated PNG, 8-bit RGBA |

`--frame N` is one-based and selects one still image for every image choice.
Single-frame sources also produce still images, including when `apng`, `gif`, or
`webp` is selected. `png` always produces a still PNG. SVG and TIFF remain static.
An explicit opaque background is composited onto every selected frame before
encoding, including for formats that otherwise preserve alpha. The compositor
uses straight 8-bit channels with nearest-integer rounding.

## Animation

GIF, animated WebP, and APNG encode the source timeline once, in order. Tags do
not add frames. GIF omits its repeat extension; WebP's loop count and APNG's play
count are one. Frames cover the entire canvas. GIF clears the previous frame to
transparent background; APNG and WebP replace the full canvas, so transparent
pixels cannot retain artwork from the previous frame. Some viewers impose their
own minimum display delays; file timing does not override viewer behaviour.

Timing conversion rounds cumulative timeline endpoints to the nearest target
tick and subtracts consecutive endpoints. This bounds total duration error to
half a tick, instead of allowing independent rounding error to accumulate.
Durations that round to zero or exceed the format's delay field are rejected
before any bytes are written.

| Format | Tick | Maximum encoded duration per frame |
| --- | --- | --- |
| GIF | 10 ms | 655,350 ms |
| WebP | 1 ms | 16,777,215 ms |
| APNG | 0.1 ms normally; see below | 65,535 seconds |

APNG uses one common delay denominator per export: 10,000, 1,000, 100, 10, or 1
ticks per second. The highest precision whose 16-bit numerator can hold the
longest frame is selected. Thus a frame longer than 6.5535 seconds lowers
precision to 1 ms; beyond 65.535 seconds to 10 ms; beyond 655.35 seconds to
100 ms; beyond 6,553.5 seconds to 1 second. The same cumulative-endpoint policy
applies at that precision. Still images have no encoded playback duration.

## GIF policy

GIF preserves fully transparent and fully opaque pixels. Partial alpha is
rejected unless the caller supplies an explicit opaque background. There is no
implicit threshold or black background. Up to 256 distinct RGBA colours per
frame use an exact local palette, counting the transparent entry. Larger
palettes use the `gif` crate's NeuQuant quantizer at sampling speed 10, without
dithering. Quantized colours are approximate. The transparent index is reserved
strictly for transparent pixels; any opaque collision is moved to the nearest
other palette colour by squared RGB distance. GIF does not promise exact RGBA
fidelity for general artwork.

## Resource limits and dependencies

Existing canvas validation applies before rendering: maximum 4,096 pixels per
side and 4,194,304 pixels total. Animation renders and encodes one frame at a time;
it does not retain all rendered frames. A WebP frame temporarily stores one
lossless bitstream for RIFF framing. Encoded outputs are capped at 256 MiB. SVG
also has a 100,000 visible-run limit and a 16 MiB byte limit. An overly complex
SVG is rejected before writing. Encoded byte-limit or I/O failures can occur
during encoding; CLI output preparation prevents incomplete prepared files from
being installed as finished outputs. Cross-file installation is not transactional.

The encoder uses maintained image-rs `image` 0.25.10 codecs, `png` 0.17 for
PNG/APNG, and `gif` 0.14 for explicit palette, delay, and disposal handling.
Image exports need no FFmpeg or native codec library. Animated WebP wraps the
image-rs lossless VP8L frame payloads in the official
[WebP RIFF animation container](https://developers.google.com/speed/webp/docs/riff_container)
using `VP8X`, `ANIM`, and `ANMF` chunks.

Dependency review checked the upstream [image-rs project](https://github.com/image-rs/image)
and its [0.25.10 release](https://github.com/image-rs/image/releases/tag/v0.25.10),
including the Rust 1.88 minimum, and the same organization's
[GIF](https://github.com/image-rs/image-gif) and
[WebP](https://github.com/image-rs/image-webp) codec repositories. Only required
format features are enabled. Actual write support is verified by the encoder
round-trip tests rather than inferred from decoder support.

Integration tests decode generated raster outputs using image-rs, inspect loop
headers, and verify dimensions, pixels, frame order, unequal/fractional timing,
transparency, backgrounds, selection, and preflight limits. This does not certify
every external viewer or editor.
