# Aseprite subset

Pixloom reads and writes `.aseprite` binary projects directly. `.ase` is an accepted input alias. No Aseprite installation, MCP server, or PNG intermediate is required. The implementation follows the [official Aseprite file specification](https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md).

| Feature | Read | Write | Policy |
| --- | --- | --- | --- |
| RGBA raster pixels | Yes | Yes | 32-bit straight RGBA, including RGB values under zero alpha |
| Layer names/order/visibility | Yes | Yes | Root normal raster layers, normal blend mode |
| Layer opacity | Yes | Yes | Native byte divided by 255 when reading; nearest byte when writing |
| Layer locks | Yes | Yes | Native editable flag maps to the inverse of `locked` |
| Prefer linked new cels | Yes | Yes | Native layer flag 16 |
| Frames and durations | Yes | Yes | Whole milliseconds, 1..65535 per frame |
| Tags | Yes | Yes | Inclusive frame ranges, forward direction, repeat count 0 |
| Tag color and user text | Yes | Yes | Standard user-data chunks preserve RGBA tag color |
| Explicit linked cels | Yes | Yes | Native link records, never inferred from identical pixels |
| Cropped/offset cels | Yes | Full canvas | Cropped rectangles must fit completely inside the canvas |
| Native user text/color | Yes | Yes | Text in annotations; color in `annotations.metadata.aseprite_native.color` |
| Editing palette and ordinary sRGB/no profile | Yes | Yes | Original chunks are carried as native metadata |
| Arbitrary Pixloom annotations and Pixelorama extras | Yes | Yes | Standard user-data text envelopes, described below |
| Groups, nested layers, non-normal blends, tilemaps, slices | No | No | Rejected |
| Indexed/grayscale, ICC/custom gamma, pixel aspect ratios other than 1:1 | No | No | Rejected |
| Cel extras, nonzero cel z-index, external resources, unknown chunks | No | No | Rejected |
| Background/reference layers, movement locks, collapsed groups, UUIDs | No | No | Rejected |
| Typed native user properties | No | No | Rejected |

## Timing and precision

The authoring schema supports fractional milliseconds. Aseprite stores a 16-bit integer millisecond duration, so builds reject fractional values and durations above 65535. Timing is never silently rounded. Aseprite has no authoritative global FPS field: Pixloom saves the authoring `fps` in project user data. Independently saved projects without this metadata use `1000 / first_frame_duration_ms` as their authoring FPS; every native frame duration remains authoritative.

Cel and layer opacity are converted with `round(opacity * 255)`. A value of `0.5` becomes byte 128 and reads back as `128 / 255`; `0.1` becomes byte 26. Pixels themselves retain their original RGBA bytes. Rendering before and after opacity conversion can differ by a rounding unit. Unlinked empty-slot opacity has no native cel field and is preserved in the layer metadata envelope. Linked empty-slot provenance can restore the original float only while the native opacity byte still matches its quantized value.

## Editing links and metadata

Each explicitly named nonempty link group writes one compressed raster cel followed by native linked-cel records. Independent timeline entries each write their own raster cel even if they use the same authoring definition or contain identical pixels. The reader resolves forward and backward references, rejects cycles, missing targets, and out-of-range frames, and groups only the actual native links.

Linked cels with different native offsets cannot be represented by the full-canvas authoring model while keeping editing links, so import rejects them. Native cels extending outside the canvas are also rejected instead of clipping their pixels. Builds reject unused cel definitions, since native Aseprite cannot carry orphan authoring pixel grids. Named groups of `cel: null` entries write one transparent native raster cel with actual linked members, including empty singleton groups. Unlinked null slots remain absent native cels. Null provenance is restored only while the entire native group retains its empty hints and all RGBA bytes remain zero; native pixel edits take precedence while preserving the actual editing links.

Pixloom reserves user-data text beginning `pixloom-aseprite-v1:` for a JSON envelope. It carries annotations, frame annotations, FPS, Pixelorama swatches/credits, authoring IDs, names for native link groups, named singleton hints, and empty-slot annotations. These are ordinary Aseprite user-data chunks and may be visible as JSON text in the editor. The format's STRING field allows at most 65535 UTF-8 bytes per envelope; builds reject larger annotations. Existing native user text and colors are preserved, while typed native property maps are rejected.

Native image/link records determine the imported editing groups. A stale multi-member link-name hint cannot turn newly independent native cels back into a group. A named singleton hint is used only for a native singleton that was explicitly marked as such. Conflicting hints are rejected when preserving them would assign one name to distinct native groups. Authoring image IDs are reused when their raster content still agrees; independent editing still depends solely on the native link records. Layer IDs are preserved when available; otherwise IDs are generated.

Native nonvisual header fields and palette/profile chunks are stored under `annotations.metadata.aseprite_native` as hex-encoded data, with the original frame numbers for chunks. They are emitted again during a build. The reader takes these fields from the native file, so stale copies inside an envelope cannot override native changes. Unknown visual or editing chunks are rejected instead of silently dropped. Editing a project in another application can remove Pixloom envelopes; arbitrary authoring metadata then cannot be reconstructed. Changes to frame counts with stale frame-annotation envelopes produce an explicit mismatch error.

## Bounds and inspection

The decoder checks actual file/frame/chunk bounds, UTF-8 strings, supported palette bounds, linked-cel references, canvas dimensions, and actual zlib output bytes. A truncated, oversized, or trailing compressed stream is an error. It uses the shared limits: at most 4096 pixels per side, 4 million canvas pixels, 4096 frames, 256 layers, 32768 timeline slots, and 256 MiB resident raster data. Files are capped at 256 MiB, individual chunks at 32 MiB, and total native/user metadata payloads at 16 MiB. The decoder streams chunks and expands one raster cel at a time; linked cels share their source buffer.

Before writing any bytes, the builder checks the same 256 MiB raster budget against the native sources it will emit: each independent cel occurrence counts as one full-canvas buffer, each explicit link group counts once per layer, and named empty groups count their transparent source. Identical pixels and reused authoring IDs do not reduce this count. At 1024×1024, 64 independent raster sources reach the budget exactly; a 65th is rejected. Both reader and writer count the same native/user chunk payloads toward the 16 MiB metadata budget and apply the same file and individual-chunk byte limits.

`inspect` reports structural metadata, chunk counts, and unsupported features. It does **not** decompress compressed raster payloads or resolve linked-cel graphs, and returns `raster_validated: false`. A successful metadata inspection is not a claim that rendering or unpacking the file will succeed.

## Verification and dependency choice

The integration tests read independently saved [asefile upstream fixtures](../tests/fixtures/aseprite/README.md) and compare the rendered pixels with the upstream PNG exports. Additional tests check binary headers/chunks, explicit versus independent links, forward references, authored metadata/tag round trips, PXO-to-Aseprite model interoperability, raw RGBA, byte precision, malformed files, actual decompression size, stale hints, and unsupported features.

The [asefile crate](https://docs.rs/asefile/0.3.8/asefile/) provides decoding, not writing. [aseprite-io](https://github.com/spebern/aseprite-io) advertises read/write support and was also investigated. Pixloom uses its own subset codec to keep shared resource limits and rejection policies explicit, with the existing `flate2` dependency for zlib.

Pixelorama v1.2.3-stable's independent Aseprite importer was run headlessly on
the generated four-frame example. Its spritesheet matched the previously
verified native PNG byte-for-byte; native JSON confirmed 125/250/125/125 ms,
the [0,2] editing link set and inclusive tag 1-4. The Aseprite application itself
and interactive editor save/edit workflows were not tested. See the
[verification record](pixloom-verification.md) for exact evidence and limits.

The reader also accepts legacy `spritex-aseprite-v1:` metadata envelopes.
