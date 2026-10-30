# Pixen project files

A `.pixen` file is Pixen's compressed, editable project. It stores the canvas, palette, layers, animation frames, cel pixels, editing links, and metadata. Saving a project keeps this information so it can be edited again; exporting an image or video renders the artwork into a delivery format.

"Container" means the ZIP-based packaging inside a `.pixen` file. It is not a separate application feature or a video format. The same artwork can also be stored as readable `.pixen.json` authoring source.

## Pack and unpack

```sh
pixen-cli pack artwork.pixen.json --output artwork.pixen
pixen-cli unpack artwork.pixen --output artwork.pixen.json
```

The parent directory must exist. Existing outputs require `--force`. Packing preserves the document's encodings by default; `pack --optimize` optimizes them first. See [JSON optimization](optimizer.md) and the [export overview](../export/README.md).

## Container layout version 1

The container layout version is independent of authoring versions 1 and 2. Released readers must read older released container versions; unsupported versions and nonempty required_features are errors. No implicit field reinterpretation.

`manifest.json` is exactly `{ "format":"pixen", "container_version":1, "document":"document.json", "required_features":[] }` (all fields required).

`document.json` uses the existing authoring Document schema and defaults, except `cels` contains exactly `{ "id":ID, "payload":"cels/ID.json" }` references. Readers resolve explicit references, not ordering or ID-derived filenames. Every reference must be unique and point to a JSON file directly inside `cels/`. All frames, layers, timelines, annotations, tags, palette and native extras stay in this document. No semantic model is forked: structural fields deserialize through the existing Document type. Unknown semantic fields are rejected.

Each payload is exactly the existing CelDefinition schema minus `id`; version 1 uses `{ "pixels":... }`, version 2 uses `{ "encoding":..., ... }`, including placement, dimensions, defaults and local dictionaries under their existing rules. The structural ID is reattached before typed decoding. Storage references never imply edit links. Unused definitions are retained and fully validated.

Only the manifest, document and referenced payload entries are allowed; directory entries, previews, external resources and other entries are rejected. ZIP stored and deflate entries are accepted; encryption, ZIP64, unsafe paths, duplicate names and special files are rejected. Reads never extract files.

Limits: 512 MiB compressed, 32770 entries (32768 cels plus two), 64 MiB per entry, 256 MiB total expanded JSON. JSON uses existing streaming complexity and 128-level serde recursion limits. Existing canvas, aggregate decoded pixel and dictionary budgets apply to full validation. Metadata inspection checks structure and references without reading payloads; individual cel access reads only that entry. Full validation reads all payloads including unused definitions.

Writers use compact JSON, manifest then document then ID-sorted payload entries, fixed ZIP timestamps and deflate level 6. Cel order inside document is preserved. Default packing preserves encodings; --optimize invokes the existing exact-index optimizer before splitting. JSON minimization does not guarantee smallest ZIP. Staged output is reopened and fully validated before replacement. Input is never modified. Standalone optimize rejects containers: unpack, optimize JSON, then pack (or pack JSON with --optimize). JSON input continues accepting legacy filenames.

Legacy containers with the `pixloom` or `spritex` format identifiers remain readable, including genuine `.pixloom` and `.spritex` projects. New containers use the `pixen` identifier and `.pixen` extension. A new Pixen container renamed to `.pixloom` is rejected; renaming it does not convert it to a legacy format.
