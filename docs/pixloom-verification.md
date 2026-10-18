# Pixloom: historical Spritex 0.2 verification

This record predates the Pixloom rename; historical commands and evidence paths are preserved.

Verified on **2026-10-16**, Windows x86-64, Rust/Cargo 1.99.0.
The declared minimum compiler is Rust 1.88; this run used 1.99.

## Commands and results

- `cargo test --offline --locked`: **69 passed**, zero failed; five optional
  external checks ignored by default.
- `cargo test --offline --locked --test video --test commands -- --ignored --nocapture`:
  **3 passed** using installed FFmpeg/ffprobe 9.0.2-full_build.
- `cargo test --release --offline --locked --test references -- --include-ignored --nocapture`:
  **5 passed**, including both optional large original-animation checks.
  The three bundled reference checks also run in the normal suite.
- In total, **all 74 distinct tests passed** when including optional checks.
- `cargo fmt --check`, `cargo clippy --offline --locked --all-targets -- -D warnings`,
  `cargo build --release --offline --locked`, and `git diff --check` passed.
- The renamed companion skill passed `skill-creator/scripts/quick_validate.py`
  using the previously installed, ignored local PyYAML dependency.
- Release CLI reported `spritex 0.2.0` and successfully exported all ten choices
  in one request, built both native formats, unpacked each to a JSON filename,
  and validated the resulting schema-1 JSON. Outputs are in ignored
  `out/spritex-smoke-0.2/`. No global installation, push, or publication occurred.

## Covered behavior

The suite retains PXO and authoring regression coverage and adds case/alias/
dedup parsing, extensionless output construction, native signature mismatch
errors, all-output preflight, force/input/collision protections, failure cleanup,
explicit one-based frame selection, every static/animated image encoder,
supported Aseprite features and malformed-input bounds.

PNG/TIFF/still and animated WebP/APNG decode to the exact expected rendered RGBA
bytes, including partial alpha and transparent replacement of previous frames.
JPEG/BMP alpha policies and explicit background compositing are checked.
GIF tests cover local palettes, quantization above 256 colours, binary alpha,
partial-alpha rejection, frame disposal, one pass, and unequal/fractional timing.
SVG checks actual pixel-run rectangles, colours, opacity, viewBox, no embedded
raster, and complexity limits.

The Aseprite codec has 16 integration tests plus two budget unit tests.
Independent upstream cropped/linked inputs match their independently exported
PNG golden images. Other checks exercise raw RGBA, forward links, independent
versus linked editing, named/unnamed empty slots, metadata/tags, opacity rounding,
integer-ms timing, stale hints after native edits, native/PXO interoperability,
unsupported features, chunk/file/zlib bounds, and exact decoded-raster budgets.
Fixture [provenance and licenses](../tests/fixtures/aseprite/README.md) identify
the pinned upstream commit; generated fixtures do not replace independent inputs.

Independent final code review found one budget mismatch: repeated independent
authoring definitions could emit more native raster sources than the Aseprite
reader's 256 MiB resident limit. The writer now preflights actual raster sources
without inferring edit links. Tests accept the exact limit, reject the next
independent source before writing, and preserve a 65-frame explicit link group.
The reviewer reran the original reproduction and confirmed a mixed PXO/Aseprite
failure installs neither file and leaves no prepared temporaries.

## Actual video encoding

Both installed encoders were exercised:

| Output | Encoder | Verified pixels | Verified timing/dimensions |
| --- | --- | --- | --- |
| MP4 | H.264 `libx264rgb`, CRF 0, RGB24 | Exact decoded rendered RGB | 3x3 odd canvas; three frames at 0/13/140 ms; delays 13/127/259 ms; total 399 ms |
| WebM | VP9 `libvpx-vp9`, lossless YUV444 | Tested RGB conversion error <=3 channel levels | Same frame count, dimensions, timestamps, delays, and duration |

Thirty frames at 1000/60 ms retain thirty frames and total exactly 500 ms in
both containers. Explicit opaque-background compositing and cleanup after
success are verified. The mixed CLI test exports PNG/GIF/WebP/APNG/MP4/WebM:
PNG matches frame 1, GIF contains all four frames, and both videos contain
four frames totaling 625 ms.

Regular dependency-free tests simulate encoder errors and size-limit exits and
check temporary cleanup, unchanged destinations, structured special-character
paths, missing FFmpeg/encoders, transparency in later/composited frames, timing
collapse, single-frame rejection, and storage limits. No image export
requires FFmpeg. See [video details](video-export.md) for resource-monitor
headroom and player compatibility limits.

## Independent native applications

Existing independently saved Pixelorama fixtures and golden exports remain
unchanged. Both original large-reference tests were rerun against
`.research/animation.pxo` and the existing native frame-24 export:

- 640x400, 17 layers, 128 frames, 8 FPS, 140 link sets, and native swatches survive
  the supported JSON/PXO round trip. Every timeline slot's raw pixels and link
  membership are checked.
- Frame 24 again differs from Pixelorama's native rendering in 23,804 of
  1,024,000 channel values, by at most one integer level; alpha is identical.
  Pixelorama quantizes intermediate blends; Spritex rounds at final output.

Pixelorama **v1.2.3-stable** was additionally run headlessly with its Aseprite
importer on the newly generated four-frame `animated.aseprite`, using separate
ignored APPDATA/output directories. Its exported spritesheet is byte-identical
to the verified native animation PNG:

```text
SHA-256 dd62634ec85607b4f5ea45e7491026de7b0f15a1d7644477284aa8167ca3e8f9
```

The native JSON independently confirms canvas 4x4, one layer, four frame durations
125/250/125/125 ms, edit-link members [0,2], and tag 1-4. Pixelorama uses FPS 1
and fractional native multipliers for this import; the actual millisecond
durations agree. Spritex's user-data envelopes remain visible as user text.
This check establishes native binary/raster/timeline import interoperability;
it does not establish that Pixelorama interprets Spritex metadata envelopes.
Evidence remains in ignored `out/spritex-native-ase/` and
`.research/spritex-ase-native.log`.

The Aseprite application itself, interactive editor save/edit workflows, and
external GUI/video-player compatibility were **not tested**. H.264 RGB and VP9
4:4:4 playback support varies; WebM is lossless in YUV rather than exact RGB.
No full-fidelity editor archive guarantee is made.

## Reproduce optional reference checks

```powershell
$env:SPRITEX_ANIMATION_FIXTURE = 'path/to/animation.pxo'
$env:SPRITEX_NATIVE_REFERENCE_PNG = 'path/to/native-frame24.png'
cargo test --release --locked --test references -- --include-ignored --nocapture
```

`SPRITEX_NATIVE_DIR` can select a newly exported native `animated.png` and
`animated.json`; otherwise bundled goldens are used. Historical spritec
research/verification records retain their original command and variable names.
