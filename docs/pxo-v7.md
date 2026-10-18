# Verified PXO 7 representation

Target: Pixelorama **v1.2.3-stable**, official Git tag `v1.2.3`, release commit
`da7b68f`. The checked `project.godot` sets version `v1.2.3-stable` and PXO 7.
The implementation was checked against official source and two independently
saved projects supplied as extracted directories. Those directories were read
without modification. Archives used for testing were repackaged from their
original files; their ZIP compression/order is consequently not native evidence.

## Source evidence

| Property | Official implementation and verified behaviour |
| --- | --- |
| Container | [OpenSave.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Autoload/OpenSave.gd), `save_pxo_file`/`open_pxo_file`: ZIP with `data.json`, `mimetype` equal to `application/x-pixelorama`, preview, and raw image data. Native writer emits data before mimetype; no OpenRaster-style first/stored mimetype requirement. |
| Raster paths | [Project.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Classes/Project.gd), `_load_image_from_pxo`: `image_data/frames/{frame+1}/layer_{layer+1}`. Full-canvas row-major RGBA8 bytes, colour mode 5. |
| Auxiliary indices | `Project._load_image_from_pxo` only applies these in indexed mode. [ImageExtended.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Classes/ImageExtended.gd) initializes a one-byte 1x1 R8 zero image in RGBA mode. Both references contain one-byte auxiliary files. Writer emits `[0]` for every raster cel; reader accepts missing indices in RGBA mode. |
| Timing | [Frame.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Classes/Frame.gd), `get_duration_in_seconds`: `duration / fps`. Native duration 1 at 8 FPS is 125 ms, not one second. |
| Layer order/opacity | `Project.ordered_layers` and [DrawingAlgos.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Autoload/DrawingAlgos.gd): increasing layer index is bottom-to-top when all cel z-indices are zero. Final opacity is layer opacity times cel opacity. |
| Links | [BaseLayer.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Classes/Layers/BaseLayer.gd), `serialize`/`deserialize`: layer-local `link_sets` with `cels` containing **zero-based frame indices**, plus display hue. First serialized member supplies shared content. Files are still written for every linked entry. |
| Cel attributes | [BaseCel.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Classes/Cels/BaseCel.gd): opacity and z-index are per-cel properties; editing links share image content, not opacity or annotations. |
| Tags | [AnimationTag.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Classes/AnimationTag.gd): `from`/`to` are one-based inclusive; colour is eight hex digits without `#`. |
| Preview | `OpenSave.save_pxo_file` composites the selected native frame and scales its longest side to 256. `spritec` deliberately embeds a full-resolution frame-1 preview and selects native frame 0; target Pixelorama successfully loaded these archives. |
| Native validation CLI | [Main.gd](https://github.com/Orama-Interactive/Pixelorama/blob/v1.2.3/src/Main.gd) accepts files and export options after `--`, with `--headless --quit` before it. |

`pixloom` writes raw full-canvas buffers for every timeline slot, including linked
duplicates. It never depends on an embedded thumbnail for rendering. Empty
authoring cels serialize as zero-filled raster images, because native raster
layers still require a cel at every frame.

## Scope

Reader/writer support RGBA8 raster data, zero z-index, ordinary bottom-to-top
normal blending, visibility, opacity, timing, tags, links, locks and new-cel-link
defaults. Metadata and user-data are carried explicitly. Native RGBA palette
swatches and author/license fields are preserved. Provenance naming hints use
reserved `_spritec*` metadata keys; actual pixel content and native link sets
take precedence after editing in Pixelorama.

Unsupported colour modes, layer types/hierarchy, blend modes, effects, animated
parameters, clipping and nonzero z-index are rejected, including on invisible
layers. Brushes, reference images, tilesets, guides and vanishing points are
rejected. Unknown feature fields are rejected conservatively; unknown extension
metadata is only data and does not establish extension support.

The reader is intentionally narrower than Pixelorama's archive reader: classic
single-disk ZIP only, no ZIP64/encryption/special files. It checks raw central
directory records before the ZIP library can hide duplicate names, limits entry
counts and sizes, checks decompression lengths/CRC for read entries, and rejects
unsafe paths. `unpack` writes newly generated authoring JSON to the requested
output filename; it does not extract
archive paths onto the filesystem. Unused thumbnails/extra files are not decoded
or CRC-verified and are not preserved.

Supported-subset round trips preserve pixels and supported editing behaviour;
they can change cel-definition IDs, grid deduplication, ZIP layout, preview size,
editor view/export settings, UI colours and link-group hues. Full-fidelity
archival round trips and compatibility with other Pixelorama releases are not
promised.
