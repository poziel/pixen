# Verification record

Verified locally on **2026-10-16**, Windows x86-64, Rust/Cargo 1.99.0.

## Automated checks

- `cargo test --locked`: **29 passed**, zero failed. Two external-reference tests
  are intentionally ignored by default, because the large private reference is
  not bundled. Four of the passing tests are unit tests; the others exercise
  real schema, archive, CLI or independent fixture boundaries.
- Both optional original-animation tests were explicitly run in release mode:
  **2 passed**. With the three bundled reference tests, that invocation reported
  five reference tests passed and zero ignored/failed.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo build --release --locked` passed.
- The companion skill passed `skill-creator/scripts/quick_validate.py`. Its
  missing PyYAML dependency was installed only into ignored local research data.
- Release CLI smoke checks passed for both examples. The full original animation
  also passed actual `unpack` → `build` → contact-sheet commands: compact
  `sprite.json` is 70,356,630 bytes, rebuilt PXO is 5,106,911 bytes, and the
  10240x3200 contact-sheet PNG is 7,502,128 bytes. Buffered compact export avoids
  expanding millions of grid indices into indented per-value filesystem writes.

The tests cover exact palette/RGBA decoding, indices above 255, arbitrary alpha,
malformed dimensions/grids/references/IDs, explicit timeline lengths, missing
cel entries, finite positive timing, tag ranges, links versus independent
content, layer order/visibility/opacity, contact-sheet ordering/padding,
supported PXO conversions, unsupported features, missing/short raster buffers,
optional RGBA index buffers, duplicate ZIP names, unsafe paths/symlinks,
central-directory size limits, output overwrite preflight, Windows filename-case
aliases, input aliases, deterministic output bytes and independent native edits
with stale provenance metadata.

An independent code review additionally reproduced Windows trailing-dot output
aliases and native linking between null/explicit all-zero cels. Both now have
regression tests: ambiguous Windows output filenames are rejected before any
write, and link validation compares actual RGBA pixels across empty-cel forms.

## Independent Pixelorama evidence

The downloaded `Pixelorama.exe`/`Pixelorama.pck` report **v1.2.3-stable**, built
on Godot 4.7.2. Generated projects were actually loaded and exported using this
binary, headlessly, with a separate process and `APPDATA` directed to an ignored
workspace directory. Runtime `spritec` commands never invoke Pixelorama.

1. The independently saved 32x32 reference has one layer/frame and 532 nonzero
   alpha pixels. The reader preserves its complete 4096-byte RGBA cel. Every
   rendered pixel matches the independently generated native 256px thumbnail
   after nearest-neighbour sampling. Its original raw-buffer hash is recorded
   in the fixture README.
2. The generated 4x4, four-frame animation was loaded in native Pixelorama and
   exported as a 16x4 sheet plus native JSON. The PNG pixels match exactly.
   Native JSON independently confirms durations 1,2,1,1 at 8 FPS, tag frames
   1-4, and a link set with members [0,2]. The PNG/JSON are bundled as fixtures.
3. The generated 2x1, three-layer opacity fixture was loaded and exported
   natively. Exact RGBA output `[223,0,32,255, 0,0,255,32]` confirms order,
   hidden-layer exclusion and combined layer/cel opacity. Its PNG is bundled.
4. The original large reference contains 640x400 pixels, 17 layers, 128 frames,
   8 FPS, one tag covering 1-128, 140 link sets (including singleton groups),
   and a saved DawnBringer palette. Unpack → model → PXO → read preserves raw
   cel pixels for **every timeline slot**, native swatches, timing and link
   memberships. Selected composited frames were also compared through the
   supported round trip.
5. Native Pixelorama independently loaded the original large reference and
   exported frame 24. Of 1,024,000 RGBA channel values, 23,804 differ from
   spritec by **one**; the maximum difference is one and alpha is identical.
   The source shows RGBA8 intermediate quantization in Pixelorama's headless
   `Image.blend_rect` path. Spritec uses high-precision source-over intermediates
   and rounds at the end. The optional test enforces the observed one-level
   tolerance, not byte identity.

Pixelorama's dummy headless renderer emitted four shader-compiler warnings on
startup. These occurred across validation runs; the exported results and native
serialized metadata were checked independently of the process exit code.
Logs/temporary native data remain under `.research/` and exports under `out/`.

## Reproduce optional checks

Repack the supplied extracted reference directory losslessly into a classic ZIP
named `animation.pxo`; retain every file and relative path. Set the environment
variables to that archive and to Pixelorama's independently exported frame-24
PNG, then run:

```powershell
$env:SPRITEC_ANIMATION_FIXTURE = 'path/to/animation.pxo'
$env:SPRITEC_NATIVE_REFERENCE_PNG = 'path/to/native-frame24.png'
cargo test --release --locked --test references -- --include-ignored --nocapture
```

To regenerate native fixtures, use the official v1.2.3 CLI, for example:

```text
Pixelorama.exe --headless --quit -- generated.pxo --export --spritesheet --frames 1-4 --json --output path/to/export.png
```

The native exporter may retain the project's saved basename; inspect the output
directory for its actual filename. Use a separate output directory and fresh
native data directory, since the native exporter can overwrite existing files.
`SPRITEC_NATIVE_DIR` optionally selects a freshly exported `animated.png` and
`animated.json` instead of the bundled golden fixtures.

## Limits and remaining checks

- Headless loading/export was tested. Interactive timeline UI, manually editing
  link groups in the editor, and GPU/canvas rendering have not been verified.
- Pixelorama's precise GPU blending and arbitrary opacity quantization are not
  reproduced byte-for-byte. Raw supported cel pixels remain exact.
- Other Pixelorama versions, indexed/other colour modes, groups, nonzero
  z-index, effects, clipping and non-normal blending are unsupported.
- Full-fidelity archive preservation, metadata-dependent extensions, GIF/APNG/
  video, sparse/offset cels and timeline shorthand are outside this version.
- This run built/tested Windows x86-64 with Rust 1.99.0. Rust 1.85 is the declared
  minimum; minimum-toolchain, Linux and macOS builds were not run.
- No global install, Git commit or release publication was performed. The
  supplied workspace had no Git repository; `target/`, `out/`, `.research/` and
  executables are excluded by `.gitignore` for a future repository.

The Windows execution sandbox denied filesystem canonicalization for the CLI
and directory reads for Cargo formatting/Clippy. The same checks passed under
normal local permissions; no production path-safety checks were removed to
accommodate the sandbox.
