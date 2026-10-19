# Authoring formats 1 and 2

UTF-8 JSON. Unknown fields are errors. The CLI never requires Pixelorama's
internal project metadata for new artwork.

| Field | Meaning |
| --- | --- |
| `version` | Required integer `1` (legacy) or `2` (explicit cel encoding). Other versions are rejected. |
| `canvas` | Required `width` and `height`, positive integers. |
| `palette` | Required array of `#RRGGBBAA` strings; indices are zero-based integers. |
| `cels` | Required array of cel definitions. Version 1 uses `{ "id": "name", "pixels": [[0,1], ...] }`; version 2 uses the explicit encodings below. May be empty for all-empty timelines. |
| `frames` | Required nonempty array of `{ "duration_ms": 125 }`. Every duration is finite and positive. |
| `layers` | Required nonempty ordered array of layers, **bottom to top**. |
| `tags` | Optional array of named inclusive frame ranges; defaults to `[]`. |
| `fps` | Optional finite positive export FPS; defaults to `12`. Durations still use milliseconds. |
| `annotations` | Optional project metadata/user-data; see below. |
| `pixelorama` | Optional native swatches/credits preserved by `unpack`; normally omit for new artwork. |

Each version-1 cel's `pixels` contains exactly `height` rows of exactly `width` numeric
indices. Index zero has no special meaning. Any palette entry may be transparent
or partially transparent. Hex case is immaterial. Palette entries are not
limited to 256: index `299` is valid with at least 300 entries. A palette contains
1..1048576 colours; palette indices are unsigned 32-bit integers.

Cel and layer IDs must be unique within their respective arrays. IDs and link
names contain 1..128 ASCII letters, digits, underscores or hyphens, with no
spaces or filesystem paths. A cel definition may be reused across layers or
frames. Unused definitions are valid, but a PXO round-trip may remove or rename
them because Pixelorama stores actual cels rather than reusable definitions.
Aseprite builds reject unused definitions to avoid dropping orphan pixel data.

## Version-2 pixel storage

Use `.pixloom.json` for new authoring filenames. Legacy `.json` filenames remain
readable; the reader does not select a schema by extension. Explicit CLI output
filenames are honored. Existing version-1 documents require no modification.
Version 1 forbids `encoding`; version 2 requires it on **every cel definition**.
Encoding is never a layer or document property. Cels in the same layer or on
one frame may use different encodings. All decode to full-canvas palette indices.

Pixels, hybrid literals/run indices, and `default_index` are unsigned 32-bit
**direct root-palette indices**. Explicit zero always means palette entry zero;
it is not a default placeholder. Transparency depends on palette alpha, at any
index. Duplicate RGBA entries retain their distinct indices in authoring
read/write and encode/decode. Dictionary/map references instead address local
row/tile definitions, never the colour palette. There are no reference chains.

### Placement and default fill

All five encodings accept paired optional `"offset": [x,y]` and
`"size": [width,height]`. Supply both or neither; omission means the full canvas
at `[0,0]`. Offset is nonnegative, size positive, both integer pairs. Coordinates
start at zero with x rightward and y downward. The entire region must fit the
canvas; overflow and clipping are errors. Sparse coordinates are region-relative.
A zero-sized region is invalid. Explicit null is not omission.

`default_index` fills outside a smaller region, and is required for that purpose.
It is always required for `sparse-rows`, including full-canvas sparse cels. Tiles
also require it for partial edge padding. All other full-canvas storage forbids
it. No missing content inherits from a frame or layer. Cropping is placement,
not another encoding. Full-canvas `sparse-rows` with `rows: []` represents an
all-default image.

### Hybrid tokens

`hybrid-rle` rows, sparse span `pixels`, row definitions and tile rows use the
same format: a literal integer represents one pixel, and `[index,count]`
represents a positive number of consecutive pixels. For example `[1,[2,3],0]`
decodes to `[1,2,2,2,0]`. Order is always **index then count**. Count one is
accepted. Negative/fractional values, zero counts, nested tokens and arrays
whose length differs from two are errors. Runs stay inside one row. Every
explicit row or definition must expand to its exact width; short/overflowing
rows never imply trailing fill. Default-coloured pixels use ordinary tokens.

Canonical writers emit a run only when its minified JSON, including internal
commas, is strictly shorter than the equivalent literals. Ties use literals:
three zeros remain `0,0,0`; four become `[0,4]`. Readers accept all valid sequences.

### Exact payloads

Each cel below is a complete version-2 definition. These examples assume a
4×2 canvas unless a different canvas is stated. Optional placement and default
fields obey the shared rules above. Unknown or encoding-inappropriate fields
are errors.

**`grid`** — explicit rows, useful for easy inspection and small/unrepetitive art.
Exactly region-height rows of region-width unsigned literal indices; no runs.
This small positioned grid requires default fill outside its 2×1 region:

```json
{"id":"grid","encoding":"grid","offset":[1,1],"size":[2,1],"default_index":1,"pixels":[[0,2]]}
```

**`hybrid-rle`** — horizontal literals/runs, often useful with cropped clustered
sprites. Exactly region-height rows, each expanding to region-width. All region
pixels are explicit; default fill is only outside a cropped region.

```json
{"id":"hybrid","encoding":"hybrid-rle","rows":[[[2,4]],[0,1,1,0]]}
```

**`sparse-rows`** — spans for scattered content. Initialize the full canvas to
`default_index`, then apply region-relative spans. Rows must have unique,
strictly increasing `y`; each supplied row has at least one span. Spans have
strictly increasing `x`, cannot overlap, and must expand to at least one pixel
within the region. Adjacent spans and explicit default pixels are valid.
Span width comes from tokens, without a separate length field. Omitted rows and
gaps retain the default; these are **not frame deltas**. Canonical writers omit
default-only areas and trim default pixels from span edges.

```json
{"id":"sparse","encoding":"sparse-rows","default_index":1,"rows":[{"y":1,"spans":[{"x":0,"pixels":[2]},{"x":3,"pixels":[0]}]}]}
```

**`row-table`** — repeated complete rows. Every `row_definitions` entry is a
hybrid row of exactly region-width. `rows` has exactly region-height local
zero-based definition references. Definitions are local to this cel; no
recursion or dictionary references inside them. Readers accept valid duplicate
and unused definitions within budgets. Canonical writers deduplicate by exact
indices, omit unused definitions and order definitions by first top-to-bottom
occurrence. Example for a 4×3 canvas:

```json
{"id":"striped","encoding":"row-table","row_definitions":[[[1,4]],[1,2,2,1]],"rows":[0,1,0]}
```

**`tiles`** — repeated rectangular blocks. `tile_size` is a positive integer
`[width,height]` within image limits. Every definition in `tiles` has exactly
tile-height hybrid rows, each expanding to tile-width. The dense `map` has
`ceil(regionHeight/tileHeight)` rows and `ceil(regionWidth/tileWidth)` columns
of local tile-definition references. No transformations, sparse entries,
external tilesets, nested dictionaries or recursion. Readers accept valid
duplicate/unused tiles within budgets. Canonical writers deduplicate by exact
indices, omit unused tiles and use first occurrence in row-major map order.

```json
{"id":"pattern","encoding":"tiles","tile_size":[2,2],"tiles":[[[1,2],[2,1]],[[2,2],[1,1]]],"map":[[0,1]]}
```

Boundary tiles may extend beyond a nondivisible region. `default_index` is
required, and **every out-of-region pixel of each placed boundary tile must
match it**. Only validated padding is discarded. A dictionary tile used in
several positions must satisfy each boundary placement. Unused tiles still
must have valid dimensions/tokens/indices. Example for a 3×1 canvas:

```json
{"id":"edge","encoding":"tiles","tile_size":[2,1],"default_index":1,"tiles":[[[0,2]],[[2,1]]],"map":[[0,1]]}
```

Storage reuse of rows, tiles or cel definitions never establishes an editing
link. Only explicit layer-local timeline `link` values do that.

### Rust authoring APIs and budgets

`schema::read_document` validates typed version-aware storage;
`schema::decode_indices` returns exact full-canvas indices without RGBA conversion.
`encoding::encode` takes the full index image, canvas, palette length, selected
`Kind`, and `Placement`. It never chooses an encoding or crop automatically and
rejects crops that would lose non-default pixels. `output::write_document`
validates and writes the authoring document. `schema::compile`/`parse` produce
the existing storage-independent RGBA rendering/native model.

Before typed array allocation, a streaming preflight enforces document array
and cumulative dictionary-definition, token and reference limits and a structural limit of 256 Mi JSON values. Before pixel expansion,
validation checks all cel storage and cumulative budgets: at most 32768 dictionary
definitions, 64 Mi expanded dictionary pixels (including unused definitions),
64 Mi tokens and 64 Mi references per document. Canvas, aggregate decoded-cel,
frame/layer/slot, JSON byte and native/export limits below also apply. Untrusted
run counts never determine allocation before row-width validation.

Native PXO/Aseprite workflows store RGBA rasters: unpack generates a version-2
`grid` document from those bytes and cannot recover original duplicate palette
indices or the original encoding/placement. Preserve the authoring JSON for
exact index identity. Rendering/native equivalence is checked separately from
exact-index round trips. No optimizer or compressed `.pixloom` container is
implemented in this phase.

## Layers and explicit timelines

```json
{
  "id": "body",
  "name": "Sprite body",
  "visible": true,
  "opacity": 1.0,
  "locked": false,
  "new_cels_linked": false,
  "timeline": [
    {"cel": "idle", "link": "held-idle", "opacity": 1.0},
    {"cel": "idle"},
    {"cel": "idle", "link": "held-idle"},
    {"cel": null}
  ]
}
```

`id`, nonempty `name`, and `timeline` are required. Visibility defaults to true;
opacity defaults to 1; lock/new-cel-link flags default to false. Opacity must be
finite and within [0,1]. A layer's timeline contains exactly as many entries as
`frames`. Entry position is its frame; arrays use ordinary zero-based JSON
indexing, while CLI frame numbers and tag ranges start at 1.

Every entry **must include `cel`**, either a known definition ID or JSON `null`.
Missing entries are errors, not holds. Null means a full-canvas transparent
empty raster cel. Entry opacity defaults to 1. There is no cel-visibility flag;
use opacity zero or an explicit empty cel where appropriate. Alpha is multiplied
by layer opacity and entry opacity; raw cel bytes remain unchanged.

`link` is optional. Entries with the same link name **within one layer** share
pixel edits in supported native projects. Each group must reference the same definition ID (or
all null). A group may contain a single entry. Distinct names remain separate
groups even when their pixel grids are identical. Reusing `"idle"` without
`link` produces independent cels; the middle frame above can be edited alone.
Opacity/annotations remain per entry even in a linked group. Link names on
different layers never connect those layers.

`new_cels_linked` controls Pixelorama's default for frames added later. It does
not link any of the supplied timeline entries automatically. The existing
timeline is governed entirely by explicit `link` values.

## Timing, tags and repetition

```json
{
  "fps": 8,
  "frames": [{"duration_ms": 125}, {"duration_ms": 250}],
  "tags": [{"name": "idle", "from": 1, "to": 2, "color": "#6699CCFF"}]
}
```

Each frame's duration applies to all layers. Pixelorama stores
`duration = duration_ms / 1000 * fps`. Thus these frames use native multipliers
1 and 2 at 8 FPS. FPS does not round a frame to a whole tick. Millisecond values
may be fractional, and their native representation must remain finite/positive.
Aseprite builds require integer milliseconds in 1..65535 and reject other values;
layer/cel opacity rounds to the nearest native byte. Rendered exports apply
their documented format timing precision using cumulative endpoint rounding.

Tags require unique nonempty names and integer `from`/`to` satisfying
`1 <= from <= to <= frame count`. Tag ranges may overlap. Colour defaults to
`#6699CCFF`; optional `user_data` defaults to an empty string. There are no
ping-pong, playback direction, loop-count or repeat fields in version 1. To
repeat a sequence, write the repeated frame descriptors and timeline entries
explicitly, including any intentional edit links. Tags only label ranges.

## Annotations and native conversion

Projects, layers, frames and timeline entries accept
`"annotations": {"metadata": {"source": "hand-authored"}, "user_data": "notes"}`.
Both fields are optional and default to empty. Metadata is a JSON object;
keys beginning `_spritec` are reserved for internal provenance. Annotations are
stored as Pixelorama metadata/user-data and returned by supported conversions.
The reserved spelling remains unchanged for compatibility after the pixloom
rename. Aseprite preserves supported annotations through standard user-data JSON
envelopes; they may be visible as text in editors.

`pixelorama` optionally carries native `palettes` (an array of maps from palette
name to serialized native palette JSON), `project_current_palette_name`,
`license`, `author_display_name`, `author_real_name`, `author_contact`, and
`author_company`. Strings default to empty, palettes to `[]`. `unpack` preserves
these values so saved swatches and credits survive. They do not define the
numeric authoring palette or switch the project into indexed mode. New artwork
normally omits this section. Native palette objects are retained as data; Rust
does not evaluate Godot expressions or implement Pixelorama's palette UI.

When unpacking a native PXO, numeric palettes and grids are generated from exact
RGBA bytes. Identical grids may reuse a definition to reduce JSON size, without
creating editing links. Native link sets determine actual link relationships.
Layer IDs are retained where possible via provenance metadata; native duplicates
receive unique IDs. Independent native edits split reused definitions as needed.
Stale provenance never invents links. A linked set with inconsistent saved
pixel buffers is rejected rather than discarding any pixels.

## Limits and compatibility

Canvas: 1..4096 per side, at most 4 Mi pixels. Frames: 1..4096. Layers: 1..256.
At most 32768 layer/frame slots, 64 Mi pixels across definitions, 256 MiB decoded
definition bytes, and 256 MiB JSON. Output ZIP entry/size budgets can further
limit a build. Invalid grids, missing references, identifiers, links, ranges,
opacities, non-finite/zero durations, arithmetic overflow and unsupported fields
are errors. No allocations are made for repeated expanded timeline grids.

The supported subset preserves raw cel pixels and editing semantics. Normal
source-over rendering rounds once at output and can differ slightly from
Pixelorama's intermediate RGBA8 quantization. Nonzero z-index, groups, effects,
clipping and non-normal blends are rejected. Conversion does not preserve all
editor/UI/export settings or arbitrary archive entries. It is not an archival
round-trip guarantee.

## Storage optimization

Author convenient valid encodings, then run `pixloom-cli optimize INPUT --in-place`
or `pixloom-cli optimize INPUT --output OUTPUT`. Generated separate output names end
in `.pixloom.json`. The optimizer measures complete serialized cels across a
bounded search; do not manually predict the smallest encoding. Exact indices,
palette order, metadata and editing links are preserved. Encoding savings and
JSON formatting/normalization savings are reported separately.

## Container storage

The external JSON schema stays self-contained. `.pixloom` packages the same
Document fields into `document.json`, replacing each cel with an explicit
`{ "id": "body", "payload": "cels/body.json" }` reference. Each payload uses
that cel's existing encoding schema without `id`. Frames/timelines stay together;
row/tile dictionaries remain local. All definitions, including unused ones, are
retained. See [exact container schema](../../../workspace/docs/container-format.md).

Pack without optimization to retain encoding choices. `pack --optimize` invokes
the lossless JSON optimizer before packaging; JSON size does not predict smallest
ZIP. Validate/build/export accept either format. Use unpack to reconstruct embedded
payloads in `.pixloom.json` before standalone optimization.
