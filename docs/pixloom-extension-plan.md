# Pixloom 0.2 extension plan

The user's extension brief authorizes implementation in this checkout. Preserve
schema version 1, PXO 7 compatibility, exact raw pixels, explicit editing links,
resource limits and output protections. The checkout directory already has the
requested name; historical evidence and fixture bytes remain unchanged.

1. Rename package, binary, imports and companion skill. Update Cargo.lock through
   Cargo. Keep legacy `_spritec*` PXO provenance keys readable and writable for
   existing project compatibility; they are format metadata rather than branding.
2. Add small explicit format enums, extensionless output-base construction,
   parse/deduplicate aliases, and all-output preflight/preparation. Build writes
   native projects; export writes rendered images/video. Unpack targets a JSON
   filename. Render stays PNG-only and uses a shared signature-based reader.
3. Implement static PNG/SVG/JPEG/BMP/TIFF, animated GIF/APNG/WebP, exact static WebP,
   explicit frame selection and alpha/background rules. Use maintained image-rs
   codecs; use official RIFF WebP animation framing around lossless frame payloads.
   Preserve frame timing within documented precision and export one timeline pass.
4. Implement optional FFmpeg MP4/WebM export with structured arguments and bounded
   temporary frames. No FFmpeg dependency for image exports. Verify actual codec
   availability, odd dimensions, variable timing, alpha/background policy, missing
   dependency/error cleanup and full-timeline output using ffprobe/decode checks.
5. Implement Aseprite RGBA native reading/writing from the official format spec,
   with independently saved upstream fixtures. Preserve represented layers,
   durations, tags and explicit links; reject unsupported/unrepresentable values.
6. Update CLI, format matrix, examples, companion skill and verification report.
   Run regression tests, formatting, Clippy, release build, and independent review.

Image encoder, video, and Aseprite modules are independent delegated tasks. The
primary agent owns shared model, CLI, format dispatch/output safety and integration.
No pushes, publication, global installation or unrelated registry/config edits.

## Decisions

- Schema 1 and native PXO provenance remain compatible with spritec 0.1 files.
- Image-rs 0.25.10 is locally available and maintained; its MSRV is 1.88, so the
  package's declared minimum Rust version will move to 1.88 rather than misstate it.
- Looping image exports play once by default; static choices always select frame 1.
- Timeline quantization must use cumulative endpoints where a format needs ticks;
  unsupported sub-tick durations are errors rather than silent zero-delay frames.
- No cross-file transactional guarantee: all outputs are prepared first, then
  installed atomically one at a time with existing overwrite protection.
