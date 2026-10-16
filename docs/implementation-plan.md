# spritec 0.1 plan

The supplied brief authorizes implementation in this session. This is a new
single-crate utility; no existing repository files or Git history were found.

## Design

`schema` owns strict version-1 JSON decoding/validation. `model` stores RGBA
buffers separately from edit-link identities. `raster` composites frames and
encodes PNG. `pxo::v7` owns the official Pixelorama v1.2.3 representation.
`archive` bounds ZIP input, and `output` installs files without accidental
overwrites. `main` exposes a small CLI.

The canonical input has canvas, palette, named cel grids, frame durations in
milliseconds, bottom-to-top layers with exact-length timelines, and one-based
inclusive tags. A repeated cel definition is independent unless entries name
the same layer-local link group. Null cels are explicit empty images. There is
no timeline expansion or implicit hold. PXO keeps FPS and maps milliseconds to
duration multipliers. Nonzero z-index is rejected initially.

## Stages and checks

1. Write schema/model/raster tests, demonstrate failure, implement static RGBA
   decoding and source-over compositing. Pin exact bytes, transparency, invalid
   grids, identifiers, references, dimensions, and opacities.
2. Write PXO tests against an independently saved 32px Pixelorama fixture.
   Implement version-7 archive reading/writing with size limits, required paths,
   and strict unsupported-feature rejection. Validate archive contents, paths,
   and traversal/duplicate/decompression errors.
3. Add multiple-layer and supported-subset round-trip checks. Preserve cel
   bytes, layer order, opacity, visibility, metadata and user-data where exposed.
4. Add animation/timing/tag/link tests. Exercise the original 640x400, 17-layer,
   128-frame reference locally, streaming redundant linked buffers rather than
   expanding all timeline data in memory.
5. Implement CLI overwrite protection, actual-cel render, inspect, unpack, and
   contact sheets. Document format, compatibility evidence and bounded resource
   limits. Add static/animated examples and thin companion skill.

## Review focus

- Link groups must not arise from equal pixels or reused definition names.
- PXO duration is not seconds; preserve FPS and effective duration.
- Reject effects/animated opacity even on currently invisible layers.
- Reject unsafe/duplicate ZIP paths and limit actual decompressed bytes.
- Refuse output aliases and preflight every output before writing either file.

## Verification

Run `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
and a release build. Run all commands on examples and reference archives. If the
target Pixelorama executable can be used locally, load generated files and
export comparison data in an isolated validation run. Record precisely which
interoperability checks pass in `docs/verification.md`.
